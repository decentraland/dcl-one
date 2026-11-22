//! The land-manager subgraph reader (`LAND_SUBGRAPH_URL`): a parcel's
//! subject and operator legs, and the parcels an update operator holds. The
//! old `/lambdas/parcels/{x}/{y}/operators` and
//! `/lambdas/users/{address}/lands-permissions` read it whenever no local
//! land index was present; v2 answers both from the land-authz index once it
//! is published and from this reader before that (`land_authz::routes`).
//! Without the URL those routes keep the unavailable envelope. Answers are
//! memoized for [`TTL`], the old cache's budget.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::shared::cache::TtlMap;

use super::ChainIndex;
use super::graph::{FULL_WALK_PAGES, THE_GRAPH_PAGE_SIZE, graph_query};

const TTL: Duration = Duration::from_secs(60);
const MAX_ENTRIES: usize = 50_000;
/// Pages walked per lookup (1000 rows each): lamb2 walks every page, so
/// this is only the runaway guard.
const MAX_PAGES: usize = FULL_WALK_PAGES;

pub struct Caches {
    operators: TtlMap<(i64, i64), Arc<Option<ParcelOperators>>>,
    by_update_operator: TtlMap<String, Arc<Vec<Value>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            operators: TtlMap::bounded(TTL, MAX_ENTRIES),
            by_update_operator: TtlMap::bounded(TTL, MAX_ENTRIES),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParcelOperators {
    pub owner: String,
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub update_managers: Vec<String>,
    pub approved_for_all: Vec<String>,
}

pub fn configured(state: &ChainIndex) -> bool {
    state.settings.land_subgraph_url.is_some()
}

/// The LAND and estate registries of the node's network (mainnet unless
/// `ETH_NETWORK` names sepolia), what an authorization's `tokenAddress` is.
fn registries(state: &ChainIndex) -> (String, String) {
    let net = super::registries::land_network(&state.settings.eth_network)
        .or_else(|| super::registries::land_network("mainnet"))
        .expect("mainnet is known");
    (
        net.land_registry.to_string(),
        net.estate_registry.to_string(),
    )
}

const QUERY_OPERATORS_PARCEL: &str = r#"
query fetchOperatorsOfParcel($x: Int, $y: Int){
  parcels(where: { and: [ { x: $x } { y: $y } ] }) {
    x
    y
    owner { address }
    operator
    updateOperator
  }
  estates(where: { parcels_: { x: $x, y: $y } }) {
    owner { address }
    operator
    updateOperator
  }
}"#;

const QUERY_AUTHORIZATIONS: &str = r#"
query fetchAuthorizations($address: String, $tokenAddress: String, $timestampFrom: BigInt) {
  authorizations(first: 1000, where: { owner_: { address: $address }, tokenAddress: $tokenAddress, timestamp_gt: $timestampFrom }, orderBy: timestamp, orderDirection: asc) {
    type
    operator
    isApproved
    timestamp
  }
}"#;

const QUERY_PARCELS_BY_UPDATE_OPERATOR: &str = r#"
query fetchParcelsByUpdateOperator($updateOperator: String, $first: Int, $skip: Int) {
  parcels(where: { updateOperator: $updateOperator } first: $first skip: $skip orderBy: id orderDirection: asc) {
    id
    x
    y
    owner { id }
    updateOperator
  }
}"#;

#[derive(Debug, Clone, PartialEq)]
struct ResolvedOperators {
    owner: String,
    operator: Option<String>,
    update_operator: Option<String>,
    belongs_to_estate: bool,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn owner_address(v: &Value) -> String {
    v.pointer("/owner/address")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// An estate row wins the subject (the parcel's owner is then the estate
/// registry); the parcel's own update operator, when set, overrides the
/// estate's.
fn resolve_operators(estates: &[Value], parcels: &[Value]) -> Option<ResolvedOperators> {
    let first_parcel = parcels.first();
    match estates.first() {
        Some(e) => Some(ResolvedOperators {
            owner: owner_address(e),
            operator: str_field(e, "operator"),
            update_operator: first_parcel
                .and_then(|p| str_field(p, "updateOperator"))
                .or_else(|| str_field(e, "updateOperator")),
            belongs_to_estate: true,
        }),
        None => first_parcel.map(|p| ResolvedOperators {
            owner: owner_address(p),
            operator: str_field(p, "operator"),
            update_operator: str_field(p, "updateOperator"),
            belongs_to_estate: false,
        }),
    }
}

/// Folds the owner's authorization log on one registry, in time order: an
/// approval adds the operator to its list, a revocation removes it.
fn fold_authorizations(
    update_managers: &mut Vec<String>,
    approved_for_all: &mut Vec<String>,
    auths: &[Value],
) {
    for a in auths {
        let operator = a.get("operator").and_then(Value::as_str).unwrap_or("");
        if operator.is_empty() {
            continue;
        }
        let target = match a.get("type").and_then(Value::as_str).unwrap_or("") {
            "UpdateManager" => &mut *update_managers,
            "Operator" => &mut *approved_for_all,
            _ => continue,
        };
        if a.get("isApproved")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            if !target.iter().any(|o| o == operator) {
                target.push(operator.to_string());
            }
        } else {
            target.retain(|o| o != operator);
        }
    }
}

/// Walk the composite timestamp (block seconds * 1,000,000 + log index).
/// Invalid or non-advancing pages cannot establish a complete grant set.
async fn walk_authorizations<F, Fut>(
    max_pages: usize,
    mut page: F,
) -> Result<(Vec<String>, Vec<String>), String>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    let mut update_managers = Vec::new();
    let mut approved_for_all = Vec::new();
    let mut timestamp_from = 0_u128;
    for _ in 0..max_pages {
        let data = page(timestamp_from.to_string()).await?;
        let auths = data
            .get("authorizations")
            .and_then(Value::as_array)
            .ok_or("subgraph response without authorizations")?;
        if auths.len() > THE_GRAPH_PAGE_SIZE as usize {
            return Err("subgraph returned more authorizations than requested".into());
        }
        let mut previous = timestamp_from;
        for row in auths {
            let timestamp = match row.get("timestamp") {
                Some(Value::String(value)) => value.parse::<u128>().ok(),
                Some(Value::Number(value)) => value.as_u64().map(u128::from),
                _ => None,
            }
            .ok_or("authorization has no unsigned timestamp")?;
            if timestamp <= previous {
                return Err("authorization timestamps do not advance".into());
            }
            previous = timestamp;
        }
        fold_authorizations(&mut update_managers, &mut approved_for_all, auths);
        if auths.len() < THE_GRAPH_PAGE_SIZE as usize {
            return Ok((update_managers, approved_for_all));
        }
        timestamp_from = previous;
    }
    Err(format!(
        "more than {max_pages} authorization pages of {THE_GRAPH_PAGE_SIZE}"
    ))
}

async fn update_managers_and_approved_for_all(
    state: &ChainIndex,
    url: &str,
    address: &str,
    token_address: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    walk_authorizations(MAX_PAGES, |timestamp_from| async move {
        graph_query(
            state,
            url,
            QUERY_AUTHORIZATIONS,
            json!({ "address": address, "tokenAddress": token_address, "timestampFrom": timestamp_from }),
        ).await
    }).await
}

/// The subject and the four legs of the parcel at `(x, y)`; `Ok(None)` when
/// the subgraph knows no parcel or estate there; `Err` without a URL or when
/// the subgraph did not answer.
pub async fn parcel_operators(
    state: &ChainIndex,
    x: i64,
    y: i64,
) -> Result<Option<ParcelOperators>, String> {
    let Some(url) = state.settings.land_subgraph_url.as_deref() else {
        return Err("no LAND subgraph is configured (LAND_SUBGRAPH_URL)".to_string());
    };
    state
        .land_graph
        .operators
        .get_or_fetch((x, y), || async move {
            let data = graph_query(
                state,
                url,
                QUERY_OPERATORS_PARCEL,
                json!({ "x": x, "y": y }),
            )
            .await?;
            let estates = data
                .get("estates")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let parcels = data
                .get("parcels")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let Some(resolved) = resolve_operators(&estates, &parcels) else {
                return Ok::<_, String>(Arc::new(None));
            };
            let (land, estate) = registries(state);
            let token_address = if resolved.belongs_to_estate {
                estate
            } else {
                land
            };
            let (update_managers, approved_for_all) =
                update_managers_and_approved_for_all(state, url, &resolved.owner, &token_address)
                    .await?;
            Ok(Arc::new(Some(ParcelOperators {
                owner: resolved.owner,
                operator: resolved.operator,
                update_operator: resolved.update_operator,
                update_managers,
                approved_for_all,
            })))
        })
        .await
        .map(|a| (*a).clone())
}

fn parcel_element(p: &Value) -> Value {
    let s = |key: &str| p.get(key).and_then(Value::as_str).unwrap_or("");
    json!({
        "id": s("id"),
        "x": s("x"),
        "y": s("y"),
        "owner": p.pointer("/owner/id").and_then(Value::as_str).unwrap_or(""),
        "updateOperator": s("updateOperator"),
    })
}

/// Every parcel whose update operator is `update_operator`, in subgraph id
/// order, as the old lands-permissions elements.
pub async fn parcels_by_update_operator(
    state: &ChainIndex,
    update_operator: &str,
) -> Result<Arc<Vec<Value>>, String> {
    let Some(url) = state.settings.land_subgraph_url.as_deref() else {
        return Err("no LAND subgraph is configured (LAND_SUBGRAPH_URL)".to_string());
    };
    let operator = update_operator.to_lowercase();
    state
        .land_graph
        .by_update_operator
        .get_or_fetch(operator.clone(), || async move {
            let mut elements = Vec::new();
            let mut skip = 0i64;
            for _ in 0..MAX_PAGES {
                let data = graph_query(
                    state,
                    url,
                    QUERY_PARCELS_BY_UPDATE_OPERATOR,
                    json!({ "updateOperator": operator, "first": THE_GRAPH_PAGE_SIZE, "skip": skip }),
                )
                .await?;
                let parcels = data.get("parcels").and_then(Value::as_array).cloned().unwrap_or_default();
                elements.extend(parcels.iter().map(parcel_element));
                if (parcels.len() as i64) != THE_GRAPH_PAGE_SIZE {
                    break;
                }
                skip += THE_GRAPH_PAGE_SIZE;
            }
            Ok::<_, String>(Arc::new(elements))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(owner: &str, operator: Option<&str>, update_operator: Option<&str>) -> Value {
        let mut v = json!({ "owner": { "address": owner } });
        if let Some(op) = operator {
            v["operator"] = json!(op);
        }
        if let Some(uo) = update_operator {
            v["updateOperator"] = json!(uo);
        }
        v
    }

    #[test]
    fn resolve_operators_parcel_update_operator_takes_precedence_over_estate() {
        let estates = vec![row("0xestateowner", None, Some("0xestateupdate"))];
        let parcels = vec![row(
            "0xparcelowner",
            Some("0xparcelop"),
            Some("0xparcelupdate"),
        )];
        let r = resolve_operators(&estates, &parcels).unwrap();
        assert!(r.belongs_to_estate);
        assert_eq!(r.owner, "0xestateowner");
        assert_eq!(r.operator, None);
        assert_eq!(r.update_operator.as_deref(), Some("0xparcelupdate"));
    }

    #[test]
    fn resolve_operators_estate_update_operator_used_when_parcel_has_none() {
        let estates = vec![row("0xestateowner", None, Some("0xestateupdate"))];
        let r = resolve_operators(&estates, &[row("0xparcelowner", None, None)]).unwrap();
        assert!(r.belongs_to_estate);
        assert_eq!(r.update_operator.as_deref(), Some("0xestateupdate"));
    }

    #[test]
    fn resolve_operators_estate_without_a_parcel_row() {
        let estates = vec![row(
            "0xestateowner",
            Some("0xestateop"),
            Some("0xestateupdate"),
        )];
        let r = resolve_operators(&estates, &[]).unwrap();
        assert_eq!(
            (
                r.owner.as_str(),
                r.operator.as_deref(),
                r.update_operator.as_deref()
            ),
            ("0xestateowner", Some("0xestateop"), Some("0xestateupdate"))
        );
    }

    #[test]
    fn resolve_operators_plain_parcel_and_nothing() {
        let r =
            resolve_operators(&[], &[row("0xparcelowner", None, Some("0xparcelupdate"))]).unwrap();
        assert!(!r.belongs_to_estate);
        assert_eq!(
            (r.owner.as_str(), r.update_operator.as_deref()),
            ("0xparcelowner", Some("0xparcelupdate"))
        );
        assert_eq!(resolve_operators(&[], &[]), None);
    }

    #[test]
    fn authorizations_fold_in_order_and_revocations_remove() {
        let mut managers = Vec::new();
        let mut approved = Vec::new();
        let auths = [
            json!({ "type": "UpdateManager", "operator": "0xa", "isApproved": true }),
            json!({ "type": "Operator", "operator": "0xb", "isApproved": true }),
            json!({ "type": "UpdateManager", "operator": "0xa", "isApproved": true }),
            json!({ "type": "Operator", "operator": "0xb", "isApproved": false }),
            json!({ "type": "Other", "operator": "0xc", "isApproved": true }),
            json!({ "type": "UpdateManager", "operator": "", "isApproved": true }),
        ];
        fold_authorizations(&mut managers, &mut approved, &auths);
        assert_eq!(managers, ["0xa"]);
        assert!(approved.is_empty());
    }

    fn authorization(timestamp: u128, operator: &str, approved: bool) -> Value {
        json!({ "timestamp": timestamp.to_string(), "type": "Operator", "operator": operator, "isApproved": approved })
    }

    #[tokio::test]
    async fn authorization_walk_reads_every_page_and_keeps_composite_timestamp_precision() {
        let first_timestamp = 9_007_199_254_740_993_u128;
        let mut asked = Vec::new();
        let (managers, approved) = walk_authorizations(3, |from| {
            let first = asked.is_empty();
            asked.push(from);
            let rows: Vec<Value> = if first {
                (0..1000)
                    .map(|i| authorization(first_timestamp + i, "operator", true))
                    .collect()
            } else {
                vec![authorization(first_timestamp + 1000, "operator", false)]
            };
            std::future::ready(Ok(json!({ "authorizations": rows })))
        })
        .await
        .unwrap();
        assert!(managers.is_empty());
        assert!(approved.is_empty(), "the later-page revocation is folded");
        assert_eq!(asked, ["0".into(), (first_timestamp + 999).to_string()]);
    }

    #[tokio::test]
    async fn authorization_walk_refuses_cap_malformed_and_nonadvancing_results() {
        let full: Vec<Value> = (1..=1000)
            .map(|i| authorization(i, "operator", true))
            .collect();
        let response = json!({ "authorizations": full });
        let err = walk_authorizations(1, |_| std::future::ready(Ok(response.clone())))
            .await
            .unwrap_err();
        assert!(err.contains("more than 1 authorization pages"));
        let err = walk_authorizations(2, |_| std::future::ready(Ok(response.clone())))
            .await
            .unwrap_err();
        assert!(err.contains("do not advance"));
        for response in [
            json!({}),
            json!({ "authorizations": null }),
            json!({ "authorizations": [{}] }),
            json!({ "authorizations": [{ "timestamp": "invalid" }] }),
            json!({ "authorizations": [{ "timestamp": -1 }] }),
            json!({ "authorizations": [authorization(10, "operator", true), authorization(10, "operator", false)] }),
            json!({ "authorizations": [authorization(10, "operator", true), authorization(9, "operator", false)] }),
        ] {
            assert!(
                walk_authorizations(2, |_| std::future::ready(Ok(response.clone())))
                    .await
                    .is_err()
            );
        }
        assert_eq!(
            walk_authorizations(2, |_| async { Err("upstream failed".into()) })
                .await
                .unwrap_err(),
            "upstream failed"
        );
    }

    #[tokio::test]
    async fn authorization_query_preserves_timestamp_traversal_through_the_last_page() {
        use std::sync::{Arc, Mutex};
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let captured = requests.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let captured = captured.clone();
                async move {
                    let rows: Vec<Value> = if body["variables"]["timestampFrom"] == "0" {
                        (1..=1000)
                            .map(|i| authorization(i, "operator", true))
                            .collect()
                    } else {
                        vec![authorization(1001, "operator", false)]
                    };
                    captured.lock().unwrap().push(body);
                    axum::Json(json!({ "data": { "authorizations": rows } }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let index = ChainIndex::new(
            crate::http::Outbound::for_tests(),
            crate::IndexSettings::default(),
        );
        index.allow_loopback_for_tests();
        let (_, approved) = update_managers_and_approved_for_all(&index, &url, "owner", "token")
            .await
            .unwrap();
        assert!(approved.is_empty());
        let calls = requests.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1]["variables"],
            json!({ "address": "owner", "tokenAddress": "token", "timestampFrom": "1000" })
        );
        let query = calls[0]["query"].as_str().unwrap();
        assert!(query.contains("timestamp_gt: $timestampFrom"));
        assert!(query.contains("orderBy: timestamp"));
        assert!(!query.contains("id_gt"));
        server.abort();
    }

    #[test]
    fn parcel_elements_carry_the_old_five_keys() {
        let p = json!({ "id": "7", "x": "1", "y": "-2", "owner": { "id": "0xowner" }, "updateOperator": "0xop" });
        assert_eq!(
            parcel_element(&p),
            json!({ "id": "7", "x": "1", "y": "-2", "owner": "0xowner", "updateOperator": "0xop" })
        );
        assert_eq!(parcel_element(&json!({}))["owner"], "");
    }
}

//! The land-manager subgraph reader (`LAND_SUBGRAPH_URL`): a parcel's
//! subject and operator legs, and the parcels an update operator holds. The
//! old `/lambdas/parcels/{x}/{y}/operators` and
//! `/lambdas/users/{address}/lands-permissions` read it whenever no local
//! land index was present; v2 answers both from the land-authz index once it
//! is published and from this reader before that (`land_authz::routes`).
//! Without the URL those routes keep the unavailable envelope. Answers are
//! memoized briefly; parcel permissions use a shorter TTL than reverse lookups.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::shared::cache::TtlMap;

use super::ChainIndex;
use super::graph::{FULL_WALK_PAGES, THE_GRAPH_PAGE_SIZE, graph_query};

const PARCEL_TTL: Duration = Duration::from_secs(5);
const TTL: Duration = Duration::from_secs(60);
const MAX_ENTRIES: usize = 50_000;
/// Pages walked per lookup (1000 rows each): lamb2 walks every page, so
/// this is only the runaway guard.
const MAX_PAGES: usize = FULL_WALK_PAGES;

type AuthorizationLists = (Vec<String>, Vec<String>);

pub struct Caches {
    subjects: TtlMap<(i64, i64), Arc<Option<ResolvedOperators>>>,
    authorizations: TtlMap<(String, String), Arc<AuthorizationLists>>,
    by_update_operator: TtlMap<String, Arc<Vec<Value>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            subjects: TtlMap::bounded(PARCEL_TTL, MAX_ENTRIES),
            authorizations: TtlMap::bounded(PARCEL_TTL, MAX_ENTRIES),
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
query fetchAuthorizations($address: String, $tokenAddress: String, $timestampFrom: BigInt, $first: Int!) {
  authorizations(first: $first, where: { owner_: { address: $address }, tokenAddress: $tokenAddress, timestamp_gt: $timestampFrom }, orderBy: timestamp, orderDirection: asc) {
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

fn owner_address(v: &Value) -> Result<String, String> {
    v.pointer("/owner/address")
        .and_then(Value::as_str)
        .filter(|address| !address.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "subgraph response without owner address".into())
}

/// An estate row wins the subject (the parcel's owner is then the estate
/// registry); the parcel's own update operator, when set, overrides the
/// estate's.
fn resolve_operators(
    estates: &[Value],
    parcels: &[Value],
) -> Result<Option<ResolvedOperators>, String> {
    let estate = estates.first();
    let parcel = parcels.first();
    let Some(subject) = estate.or(parcel) else {
        return Ok(None);
    };
    Ok(Some(ResolvedOperators {
        owner: owner_address(subject)?,
        operator: str_field(subject, "operator"),
        update_operator: parcel
            .and_then(|p| str_field(p, "updateOperator"))
            .or_else(|| estate.and_then(|e| str_field(e, "updateOperator"))),
        belongs_to_estate: estate.is_some(),
    }))
}

/// Folds the owner's authorization log on one registry, in time order: an
/// approval adds the operator to its list, a revocation removes it. Unknown
/// types, empty operators and non-boolean approvals invalidate the result.
fn fold_authorizations(
    update_managers: &mut Vec<String>,
    approved_for_all: &mut Vec<String>,
    auths: &[Value],
) -> Result<(), String> {
    for a in auths {
        let operator = a
            .get("operator")
            .and_then(Value::as_str)
            .filter(|operator| !operator.is_empty())
            .ok_or("authorization has no nonempty operator")?;
        let approved = a
            .get("isApproved")
            .and_then(Value::as_bool)
            .ok_or("authorization has no boolean approval")?;
        let target = match a.get("type").and_then(Value::as_str).unwrap_or("") {
            "UpdateManager" => &mut *update_managers,
            "Operator" => &mut *approved_for_all,
            _ => return Err("authorization has no recognized type".into()),
        };
        if approved {
            if !target.iter().any(|o| o == operator) {
                target.push(operator.to_string());
            }
        } else {
            target.retain(|o| o != operator);
        }
    }
    Ok(())
}

/// Walk the composite timestamp (block seconds * 1,000,000 + log index).
/// See decentraland/LAND-permissions-graph@51333777:src/utils/authorization.ts:79-83.
/// Invalid or non-advancing pages cannot establish a complete grant set.
async fn walk_authorizations<F, Fut>(
    max_pages: usize,
    mut page: F,
) -> Result<AuthorizationLists, String>
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
        fold_authorizations(&mut update_managers, &mut approved_for_all, auths)?;
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
) -> Result<AuthorizationLists, String> {
    walk_authorizations(MAX_PAGES, |timestamp_from| async move {
        graph_query(
            state,
            url,
            QUERY_AUTHORIZATIONS,
            json!({ "address": address, "tokenAddress": token_address, "first": THE_GRAPH_PAGE_SIZE, "timestampFrom": timestamp_from }),
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
    let subject = state
        .land_graph
        .subjects
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
                .ok_or("subgraph response without estates")?;
            let parcels = data
                .get("parcels")
                .and_then(Value::as_array)
                .ok_or("subgraph response without parcels")?;
            Ok::<_, String>(Arc::new(resolve_operators(estates, parcels)?))
        })
        .await?;
    let Some(resolved) = subject.as_ref() else {
        return Ok(None);
    };

    // Cache the parcels and the shared authorization scope independently so a
    // parcel whose subject is refreshed cannot extend the lifetime of a grant.
    let (land, estate) = registries(state);
    let registry = if resolved.belongs_to_estate {
        estate
    } else {
        land
    };
    let owner = resolved.owner.to_lowercase();
    let authorizations = state
        .land_graph
        .authorizations
        .get_or_fetch((owner.clone(), registry.clone()), || async {
            update_managers_and_approved_for_all(state, url, &owner, &registry)
                .await
                .map(Arc::new)
        })
        .await?;
    Ok(Some(ParcelOperators {
        owner: resolved.owner.clone(),
        operator: resolved.operator.clone(),
        update_operator: resolved.update_operator.clone(),
        update_managers: authorizations.0.clone(),
        approved_for_all: authorizations.1.clone(),
    }))
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
    update_operator_pages(state, update_operator, MAX_PAGES).await
}

async fn update_operator_pages(
    state: &ChainIndex,
    update_operator: &str,
    max_pages: usize,
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
            for _ in 0..max_pages {
                let data = graph_query(
                    state,
                    url,
                    QUERY_PARCELS_BY_UPDATE_OPERATOR,
                    json!({ "updateOperator": operator, "first": THE_GRAPH_PAGE_SIZE, "skip": skip }),
                )
                .await?;
                let parcels = data.get("parcels")
                    .and_then(Value::as_array)
                    .ok_or("subgraph response without parcels")?;
                if parcels.len() > THE_GRAPH_PAGE_SIZE as usize {
                    return Err("subgraph returned more parcels than requested".into());
                }
                elements.extend(parcels.iter().map(parcel_element));
                if (parcels.len() as i64) != THE_GRAPH_PAGE_SIZE {
                    return Ok(Arc::new(elements));
                }
                skip += THE_GRAPH_PAGE_SIZE;
            }
            Err(format!("no terminal parcel page after {max_pages} pages"))
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
        let r = resolve_operators(&estates, &parcels).unwrap().unwrap();
        assert!(r.belongs_to_estate);
        assert_eq!(r.owner, "0xestateowner");
        assert_eq!(r.operator, None);
        assert_eq!(r.update_operator.as_deref(), Some("0xparcelupdate"));
    }

    #[test]
    fn resolve_operators_estate_update_operator_used_when_parcel_has_none() {
        let estates = vec![row("0xestateowner", None, Some("0xestateupdate"))];
        let r = resolve_operators(&estates, &[row("0xparcelowner", None, None)])
            .unwrap()
            .unwrap();
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
        let r = resolve_operators(&estates, &[]).unwrap().unwrap();
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
        let r = resolve_operators(&[], &[row("0xparcelowner", None, Some("0xparcelupdate"))])
            .unwrap()
            .unwrap();
        assert!(!r.belongs_to_estate);
        assert_eq!(
            (r.owner.as_str(), r.update_operator.as_deref()),
            ("0xparcelowner", Some("0xparcelupdate"))
        );
        assert_eq!(resolve_operators(&[], &[]).unwrap(), None);
        assert!(resolve_operators(&[], &[json!({"owner": null})]).is_err());
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
        ];
        fold_authorizations(&mut managers, &mut approved, &auths).unwrap();
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
                    .map(|i| {
                        let mut row = authorization(first_timestamp + i, "operator", true);
                        if i == 999 {
                            row["timestamp"] = json!((first_timestamp + i) as u64);
                        }
                        row
                    })
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
        for (response, error) in [
            (json!({}), "subgraph response without authorizations"),
            (
                json!({"authorizations": null}),
                "subgraph response without authorizations",
            ),
            (
                json!({"authorizations": vec![Value::Null; THE_GRAPH_PAGE_SIZE as usize + 1]}),
                "subgraph returned more authorizations than requested",
            ),
            (
                json!({"authorizations": [{}]}),
                "authorization has no unsigned timestamp",
            ),
            (
                json!({"authorizations": [{"timestamp": "invalid"}]}),
                "authorization has no unsigned timestamp",
            ),
            (
                json!({"authorizations": [{"timestamp": -1}]}),
                "authorization has no unsigned timestamp",
            ),
            (
                json!({"authorizations": [authorization(10, "operator", true), authorization(10, "operator", false)]}),
                "authorization timestamps do not advance",
            ),
            (
                json!({"authorizations": [authorization(10, "operator", true), authorization(9, "operator", false)]}),
                "authorization timestamps do not advance",
            ),
        ] {
            assert_eq!(
                walk_authorizations(2, |_| std::future::ready(Ok(response.clone())))
                    .await
                    .unwrap_err(),
                error
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
    async fn authorization_walk_rejects_malformed_revocations_on_same_or_later_page() {
        for later_page in [false, true] {
            for (field, values) in [
                (
                    "type",
                    vec![
                        Value::Null,
                        json!(17),
                        json!(""),
                        json!("Other"),
                        json!("Owner"),
                        json!("UpdateOperator"),
                    ],
                ),
                ("operator", vec![Value::Null, json!(17), json!("")]),
                ("isApproved", vec![Value::Null, json!("false"), json!(0)]),
            ] {
                for value in std::iter::once(None).chain(values.into_iter().map(Some)) {
                    let mut revoked =
                        authorization(if later_page { 1001 } else { 2 }, "operator", false);
                    match value {
                        Some(value) => {
                            revoked[field] = value;
                        }
                        None => {
                            revoked.as_object_mut().unwrap().remove(field);
                        }
                    }
                    let mut calls = 0;
                    let result = walk_authorizations(3, |_| {
                        calls += 1;
                        let rows = if later_page && calls == 1 {
                            (1..=1000)
                                .map(|i| authorization(i, "operator", true))
                                .collect()
                        } else if later_page {
                            vec![revoked.clone()]
                        } else {
                            vec![authorization(1, "operator", true), revoked.clone()]
                        };
                        std::future::ready(Ok(json!({ "authorizations": rows })))
                    })
                    .await;
                    assert!(
                        result.is_err(),
                        "malformed {field} must not preserve an earlier grant: {revoked}"
                    );
                    assert_eq!(calls, if later_page { 2 } else { 1 });
                }
            }
        }
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
            json!({ "address": "owner", "tokenAddress": "token", "first": 1000, "timestampFrom": "1000" })
        );
        let query = calls[0]["query"].as_str().unwrap();
        assert!(query.contains("first: $first"));
        assert!(query.contains("timestamp_gt: $timestampFrom"));
        assert!(query.contains("orderBy: timestamp"));
        assert!(!query.contains("id_gt"));
        server.abort();
    }

    #[tokio::test]
    async fn parcel_subjects_and_shared_authorizations_expire_independently() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let parcels = Arc::new(AtomicUsize::new(0));
        let authorizations = Arc::new(AtomicUsize::new(0));
        let parcel_queries = parcels.clone();
        let auth_queries = authorizations.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(request): axum::Json<Value>| {
                let query = request["query"].as_str().unwrap();
                let data = if query.contains("fetchOperatorsOfParcel") {
                    parcel_queries.fetch_add(1, Ordering::SeqCst);
                    json!({"estates": [], "parcels": [row("owner", None, None)]})
                } else {
                    assert!(query.contains("fetchAuthorizations"));
                    let count = auth_queries.fetch_add(1, Ordering::SeqCst);
                    if count == 0 {
                        json!({"authorizations": [{
                            "operator": "manager", "type": "UpdateManager", "timestamp": "1", "isApproved": true
                        }]})
                    } else {
                        json!({"authorizations": []})
                    }
                };
                async move { axum::Json(json!({"data": data})) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let state = ChainIndex::new(
            crate::http::Outbound::for_tests(),
            crate::IndexSettings {
                land_subgraph_url: Some(url),
                ..Default::default()
            },
        );
        state.allow_loopback_for_tests();
        async fn advance_two_seconds() {
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(2)).await;
            tokio::time::resume();
        }

        assert_eq!(
            parcel_operators(&state, 1, 2)
                .await
                .unwrap()
                .unwrap()
                .update_managers,
            ["manager"]
        );
        advance_two_seconds().await;
        advance_two_seconds().await;
        assert_eq!(
            parcel_operators(&state, 3, 4)
                .await
                .unwrap()
                .unwrap()
                .update_managers,
            ["manager"]
        );
        assert_eq!(parcels.load(Ordering::SeqCst), 2);
        assert_eq!(authorizations.load(Ordering::SeqCst), 1);

        advance_two_seconds().await;
        assert!(
            parcel_operators(&state, 3, 4)
                .await
                .unwrap()
                .unwrap()
                .update_managers
                .is_empty()
        );
        assert_eq!(parcels.load(Ordering::SeqCst), 2);
        assert_eq!(authorizations.load(Ordering::SeqCst), 2);
        assert!(
            parcel_operators(&state, 1, 2)
                .await
                .unwrap()
                .unwrap()
                .update_managers
                .is_empty()
        );
        assert_eq!(parcels.load(Ordering::SeqCst), 3);
        assert_eq!(authorizations.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn incomplete_land_responses_are_retried_before_caching_an_empty_answer() {
        use std::collections::VecDeque;
        use std::sync::Mutex;

        let replies = Arc::new(Mutex::new(VecDeque::from([
            json!({"parcels": [row("registry", None, None)]}),
            json!({"estates": []}),
            json!({"estates": [], "parcels": [{"owner": null}]}),
            json!({"estates": [], "parcels": []}),
            json!({}),
            json!({"parcels": vec![Value::Null; THE_GRAPH_PAGE_SIZE as usize + 1]}),
            json!({"parcels": vec![Value::Null; THE_GRAPH_PAGE_SIZE as usize]}),
            json!({"parcels": []}),
        ])));
        let captured = replies.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move || {
                let next = captured.lock().unwrap().pop_front().unwrap();
                async move { axum::Json(json!({"data": next})) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let state = ChainIndex::new(
            crate::http::Outbound::for_tests(),
            crate::IndexSettings {
                land_subgraph_url: Some(url),
                ..Default::default()
            },
        );
        state.allow_loopback_for_tests();
        for error in [
            "subgraph response without estates",
            "subgraph response without parcels",
            "subgraph response without owner address",
        ] {
            assert_eq!(parcel_operators(&state, 1, 2).await.unwrap_err(), error);
        }
        assert_eq!(parcel_operators(&state, 1, 2).await.unwrap(), None);
        assert_eq!(parcel_operators(&state, 1, 2).await.unwrap(), None);
        assert_eq!(
            parcels_by_update_operator(&state, "operator")
                .await
                .unwrap_err(),
            "subgraph response without parcels"
        );
        assert_eq!(
            update_operator_pages(&state, "operator", 1)
                .await
                .unwrap_err(),
            "subgraph returned more parcels than requested"
        );
        assert_eq!(
            update_operator_pages(&state, "operator", 1)
                .await
                .unwrap_err(),
            "no terminal parcel page after 1 pages"
        );
        assert!(
            parcels_by_update_operator(&state, "operator")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(replies.lock().unwrap().is_empty());
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

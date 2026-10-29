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
use super::graph::{THE_GRAPH_PAGE_SIZE, graph_query};

const TTL: Duration = Duration::from_secs(60);
const MAX_ENTRIES: usize = 50_000;
/// Pages walked per lookup (1000 rows each) before the answer is cut off.
const MAX_PAGES: usize = 20;

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

async fn update_managers_and_approved_for_all(
    state: &ChainIndex,
    url: &str,
    address: &str,
    token_address: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut update_managers = Vec::new();
    let mut approved_for_all = Vec::new();
    let mut timestamp_from = "0".to_string();
    for _ in 0..MAX_PAGES {
        let data = graph_query(
            state,
            url,
            QUERY_AUTHORIZATIONS,
            json!({ "address": address, "tokenAddress": token_address, "timestampFrom": timestamp_from }),
        )
        .await?;
        let auths = data
            .get("authorizations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        fold_authorizations(&mut update_managers, &mut approved_for_all, &auths);
        if (auths.len() as i64) != THE_GRAPH_PAGE_SIZE {
            break;
        }
        timestamp_from = match auths.last().and_then(|a| a.get("timestamp")) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => break,
        };
    }
    Ok((update_managers, approved_for_all))
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

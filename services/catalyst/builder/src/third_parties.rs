//! Third parties: the registry's providers, read live through
//! [`crate::chain_index::third_parties`], and the virtual third parties
//! (`builder.virtual_third_parties`) that stand for a provider the registry
//! doesn't have yet, as builder-server keeps them.
//!
//! A decision takes only a registry read that started after the decision
//! began ([`ThirdParties::registry`], [`crate::kept::Kept::get_since`]): one
//! read runs at a time and every request waiting when it started shares it,
//! so a manager the registry no longer lists loses access as soon as the
//! subgraph has indexed the change. A node with no registry subgraph, or a
//! read that fails, can't tell: 503, kept [`crate::kept::FAILURE_TTL`], and
//! never an answer from an older read. No read holds a database connection.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use sqlx::PgConnection;
use tokio::time::Instant;

use crate::chain_index::ChainIndex;
use crate::chain_index::third_parties::{LinkedContract, Provider};
use crate::http::ApiError;
use crate::kept::{FETCH_TIMEOUT, Kept};

/// Every provider of the registry, sorted by id.
pub type Registry = Arc<Vec<Provider>>;

/// The registry read and its single-flight cache.
pub struct ThirdParties {
    kept: Kept<Registry>,
}

impl Default for ThirdParties {
    fn default() -> Self {
        Self {
            kept: Kept::new(Duration::ZERO),
        }
    }
}

impl ThirdParties {
    /// The registry as a read that started at or after `since` answered it;
    /// 503 when the node can't tell.
    pub async fn registry(
        &self,
        market: Option<&Arc<ChainIndex>>,
        since: Instant,
    ) -> Result<Registry, ApiError> {
        self.kept
            .get_since(since, || fetch(market.cloned()))
            .await
            .ok_or_else(unavailable)
    }
}

fn unavailable() -> ApiError {
    ApiError::service_unavailable("The third party registry is not available. Try again later.")
}

async fn fetch(market: Option<Arc<ChainIndex>>) -> Option<Registry> {
    let market = market?;
    let error = match tokio::time::timeout(
        FETCH_TIMEOUT,
        crate::chain_index::third_parties::registry(&market),
    )
    .await
    {
        Ok(Ok(providers)) => return Some(Arc::new(providers)),
        Ok(Err(error)) => error,
        Err(_) => "timed out".to_string(),
    };
    tracing::warn!(error = %error, "builder third-party registry read failed; third-party decisions answer 503");
    None
}

/// The registry's record of `id`, compared exactly.
pub fn provider<'a>(registry: &'a [Provider], id: &str) -> Option<&'a Provider> {
    registry
        .binary_search_by(|p| p.id.as_str().cmp(id))
        .ok()
        .map(|i| &registry[i])
}

/// Whether `address` is in `managers`, compared case-insensitively.
pub fn has_manager(managers: &[String], address: &str) -> bool {
    !address.is_empty() && managers.iter().any(|m| m.eq_ignore_ascii_case(address))
}

/// A row of `builder.virtual_third_parties`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Virtual {
    pub id: String,
    pub managers: Vec<String>,
    pub raw_metadata: String,
    pub is_programmatic: bool,
}

const VIRTUAL: &str =
    "SELECT id, managers, raw_metadata, is_programmatic FROM builder.virtual_third_parties";

/// The virtual third party `id`.
pub async fn virtual_by_id(conn: &mut PgConnection, id: &str) -> Result<Option<Virtual>, ApiError> {
    Ok(
        sqlx::query_as(sqlx::AssertSqlSafe(format!("{VIRTUAL} WHERE id = $1")))
            .bind(id)
            .fetch_optional(conn)
            .await?,
    )
}

/// The name, description and linked contracts a virtual third party's
/// `raw_metadata` holds (`tp:1:<name>:<description>[:<network>-<address>;...]`),
/// read as builder-server reads it.
pub fn parse_raw_metadata(raw: &str) -> (String, String, Vec<LinkedContract>) {
    let parts: Vec<&str> = raw.split(':').collect();
    let name = parts.get(2).copied().unwrap_or_default().to_string();
    let mut rest: Vec<&str> = parts.iter().skip(3).copied().collect();
    let contracts = if rest.len() >= 2 {
        rest.pop()
            .unwrap_or_default()
            .split(';')
            .filter_map(|c| match c.split('-').collect::<Vec<_>>().as_slice() {
                [network, address] => Some(LinkedContract {
                    network: network.to_string(),
                    address: address.to_string(),
                }),
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    (name, rest.join(":"), contracts)
}

/// `raw_metadata` for a name, description and linked contracts.
pub fn raw_metadata(name: &str, description: &str, contracts: &[LinkedContract]) -> String {
    let contracts = contracts
        .iter()
        .map(|c| format!("{}-{}", c.network, c.address))
        .collect::<Vec<_>>()
        .join(";");
    if contracts.is_empty() {
        format!("tp:1:{name}:{description}")
    } else {
        format!("tp:1:{name}:{description}:{contracts}")
    }
}

fn contracts_json(contracts: &[LinkedContract]) -> Value {
    Value::Array(
        contracts
            .iter()
            .map(|c| json!({ "address": c.address, "network": c.network }))
            .collect(),
    )
}

/// A registry record as `GET /v1/thirdParties` answers it.
pub fn provider_view(p: &Provider) -> Value {
    json!({
        "id": p.id,
        "root": p.root,
        "managers": p.managers,
        "maxItems": p.max_items,
        "isApproved": p.is_approved,
        "isProgrammatic": p.is_programmatic,
        "name": p.name,
        "description": p.description,
        "contracts": contracts_json(&p.contracts),
        "published": true,
    })
}

/// A virtual third party as `GET /v1/thirdParties` answers it.
pub fn virtual_view(v: &Virtual) -> Value {
    let (name, description, contracts) = parse_raw_metadata(&v.raw_metadata);
    json!({
        "id": v.id,
        "managers": v.managers,
        "name": name,
        "description": description,
        "contracts": contracts_json(&contracts),
        "maxItems": "0",
        "isApproved": false,
        "published": false,
        "isProgrammatic": v.is_programmatic,
        "root": "",
    })
}

/// A provider `GET /v1/thirdParties` lists to a manager: its id, its view,
/// whether it is programmatic, and its virtual third party's managers.
#[derive(Debug, Clone)]
pub struct Listed {
    pub id: String,
    pub view: Value,
    pub programmatic: bool,
    pub virtual_managers: Option<Vec<String>>,
}

/// builder-server's `getThirdParties(manager)`: the registry's providers
/// that list `manager`, less an unapproved one whose virtual third party
/// doesn't list it, then the virtual third parties that list it whose
/// provider the registry doesn't hold, least recently written first. So a
/// provider is listed exactly to the wallets that manage it.
pub async fn managed_by(
    conn: &mut PgConnection,
    registry: &[Provider],
    manager: &str,
) -> Result<Vec<Listed>, ApiError> {
    let listed: Vec<&Provider> = registry.iter().filter(|p| p.managed_by(manager)).collect();
    let ids: Vec<&str> = listed.iter().map(|p| p.id.as_str()).collect();
    let virtuals: Vec<Virtual> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{VIRTUAL} WHERE id = ANY($1) OR $2 = ANY(managers) ORDER BY updated_at, id"
    )))
    .bind(&ids)
    .bind(manager.to_ascii_lowercase())
    .fetch_all(conn)
    .await?;
    let by_id: HashMap<&str, &Virtual> = virtuals.iter().map(|v| (v.id.as_str(), v)).collect();
    let mut out: Vec<Listed> = listed
        .into_iter()
        .filter(|p| {
            p.is_approved
                || by_id
                    .get(p.id.as_str())
                    .is_none_or(|v| has_manager(&v.managers, manager))
        })
        .map(|p| Listed {
            id: p.id.clone(),
            view: provider_view(p),
            programmatic: p.is_programmatic,
            virtual_managers: by_id.get(p.id.as_str()).map(|v| v.managers.clone()),
        })
        .collect();
    for v in &virtuals {
        if has_manager(&v.managers, manager)
            && provider(registry, &v.id).is_none()
            && !out.iter().any(|l| l.id == v.id)
        {
            out.push(Listed {
                id: v.id.clone(),
                view: virtual_view(v),
                programmatic: v.is_programmatic,
                virtual_managers: Some(v.managers.clone()),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(network: &str, address: &str) -> LinkedContract {
        LinkedContract {
            network: network.into(),
            address: address.into(),
        }
    }

    #[test]
    fn raw_metadata_reads_and_writes_as_builder_server_does() {
        assert_eq!(
            parse_raw_metadata("tp:1:a name:a description:amoy-0x0"),
            (
                "a name".into(),
                "a description".into(),
                vec![contract("amoy", "0x0")]
            )
        );
        assert_eq!(
            parse_raw_metadata("tp:1:a name:a: description:amoy-0x0"),
            (
                "a name".into(),
                "a: description".into(),
                vec![contract("amoy", "0x0")]
            )
        );
        assert_eq!(
            parse_raw_metadata("tp:1:a name:a description:"),
            ("a name".into(), "a description".into(), vec![])
        );
        assert_eq!(
            parse_raw_metadata("tp:1:"),
            (String::new(), String::new(), vec![])
        );
        assert_eq!(
            parse_raw_metadata("tp:1:n:d:amoy-0x1;bad;matic-0x2"),
            (
                "n".into(),
                "d".into(),
                vec![contract("amoy", "0x1"), contract("matic", "0x2")]
            )
        );
        assert_eq!(
            raw_metadata("a name", "a description", &[contract("amoy", "0x0")]),
            "tp:1:a name:a description:amoy-0x0"
        );
        assert_eq!(
            raw_metadata("a name", "a description", &[]),
            "tp:1:a name:a description"
        );
    }

    #[test]
    fn a_manager_is_compared_without_case_and_nobody_is_the_empty_address() {
        let managers = vec!["0xabc0000000000000000000000000000000000001".to_string()];
        assert!(has_manager(
            &managers,
            "0xABC0000000000000000000000000000000000001"
        ));
        assert!(!has_manager(
            &managers,
            "0xabc0000000000000000000000000000000000002"
        ));
        assert!(!has_manager(&[String::new()], ""));
    }
}

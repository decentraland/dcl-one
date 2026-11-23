//! The curation committee's current members, as builder-server reads them
//! for `isCommitteeMember` and `GET /v1/committee`: the accounts the Polygon
//! collections subgraph ([`crate::State::collections_subgraph`]) marks
//! `isCommitteeMember`, paged as builder-server pages them ([`QUERY`], pages
//! of `THE_GRAPH_PAGE_SIZE` while a page is full), through the shared
//! subgraph client (`chain_index::graph::graph_query`: the SSRF guard, the
//! body cap and the retry ladder) under `kept::FETCH_TIMEOUT`.
//!
//! A write's decision takes only a read that started after it began, a
//! read-only one also a read that started up to `kept::READ_WINDOW` before
//! it ([`crate::kept`]'s single-flight cache, [`Since`]): a wallet the
//! subgraph stops listing loses its writes as soon as the subgraph has
//! indexed the change, and its reads within the window. A node with no
//! collections subgraph, a read that fails, or an answer with a malformed
//! account or more than [`MAX_MEMBERS`] members can't tell:
//! [`Committee::members`] and [`Committee::is_member`] answer 503, never an
//! empty committee, and [`Committee::known_member`] answers false. The node
//! can't tell which chain a subgraph indexes; the deployment pairs the
//! subgraph with the node's network.

use std::sync::Arc;
use std::time::Duration;

use dcl_auth::is_eth_address;
use serde::Serialize;
use serde_json::{Value, json};

use crate::State;
use crate::chain_index::ChainIndex;
use crate::chain_index::graph::{THE_GRAPH_PAGE_SIZE, graph_query};
use crate::http::ApiError;
use crate::kept::{FETCH_TIMEOUT, Kept, Since};

/// The most members a read accepts: one page of The Graph.
pub const MAX_MEMBERS: usize = THE_GRAPH_PAGE_SIZE as usize;

const QUERY: &str = "query getCommitteeAccounts($first: Int!, $skip: Int!) {\
  accounts(first: $first, skip: $skip, where: { isCommitteeMember: true }) { id address }\
}";

/// One member as `GET /v1/committee` answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    pub id: String,
    pub address: String,
}

/// The current members, in the subgraph's order.
pub type Members = Arc<Vec<Member>>;

/// The committee read and its single-flight cache.
pub struct Committee {
    kept: Kept<Members>,
}

impl Default for Committee {
    fn default() -> Self {
        Self {
            kept: Kept::new(Duration::ZERO),
        }
    }
}

impl Committee {
    /// The members as a read that started at or after `since` answered
    /// them; 503 when the subgraph can't tell.
    pub async fn members(&self, state: &State, since: Since) -> Result<Members, ApiError> {
        self.kept
            .get_since(since, || {
                fetch(state.market.clone(), state.collections_subgraph())
            })
            .await
            .ok_or_else(unavailable)
    }

    /// Whether `address` is a member; 503 when the subgraph can't tell.
    pub async fn is_member(
        &self,
        state: &State,
        since: Since,
        address: &str,
    ) -> Result<bool, ApiError> {
        Ok(contains(&self.members(state, since).await?, address))
    }

    /// Whether the subgraph shows `address` on the committee; false when it
    /// can't tell, for a view only the committee widens.
    pub async fn known_member(&self, state: &State, since: Since, address: &str) -> bool {
        self.members(state, since)
            .await
            .is_ok_and(|members| contains(&members, address))
    }
}

#[cfg(test)]
impl Committee {
    /// Ages the kept read past every read-only decision's window.
    pub(crate) fn outdate(&self) {
        self.kept.backdate(crate::kept::READ_WINDOW);
    }
}

fn contains(members: &[Member], address: &str) -> bool {
    members
        .iter()
        .any(|m| m.address.eq_ignore_ascii_case(address))
}

fn unavailable() -> ApiError {
    ApiError::service_unavailable("The curation committee is not available. Try again later.")
}

async fn fetch(market: Option<Arc<ChainIndex>>, url: Option<String>) -> Option<Members> {
    let error = match tokio::time::timeout(FETCH_TIMEOUT, read(market, url)).await {
        Ok(Ok(members)) => return Some(Arc::new(members)),
        Ok(Err(error)) => error,
        Err(_) => "timed out".to_string(),
    };
    tracing::warn!(error = %error, "builder committee read failed; committee decisions answer 503");
    None
}

async fn read(market: Option<Arc<ChainIndex>>, url: Option<String>) -> Result<Vec<Member>, String> {
    let market = market.ok_or("no chain index")?;
    let url = url.ok_or("no Polygon collections subgraph")?;
    let mut members = Vec::new();
    loop {
        let data = graph_query(
            &market,
            &url,
            QUERY,
            json!({ "first": THE_GRAPH_PAGE_SIZE, "skip": members.len() }),
        )
        .await?;
        let page = data
            .get("accounts")
            .and_then(Value::as_array)
            .ok_or("subgraph answered no accounts")?;
        for account in page {
            let (Some(id), Some(address)) = (
                account.get("id").and_then(Value::as_str),
                account.get("address").and_then(Value::as_str),
            ) else {
                return Err("subgraph answered an account without id or address".into());
            };
            if !is_eth_address(id) || !is_eth_address(address) {
                return Err("subgraph answered an account that is not an address".into());
            }
            members.push(Member {
                id: id.to_ascii_lowercase(),
                address: address.to_ascii_lowercase(),
            });
        }
        if members.len() > MAX_MEMBERS {
            return Err(format!("committee longer than {MAX_MEMBERS} members"));
        }
        if page.len() < THE_GRAPH_PAGE_SIZE as usize {
            return Ok(members);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMBER: &str = "0x00000000000000000000000000000000000000c2";

    async fn committee_route(
        fake: Option<&crate::tests::support::FakeCommittee>,
    ) -> (u16, serde_json::Value) {
        let market = match fake {
            Some(fake) => Some(crate::tests::support::committee_index(fake).await),
            None => None,
        };
        answer(crate::tests::support::state_without_db(market).await).await
    }

    async fn answer(state: crate::SharedState) -> (u16, serde_json::Value) {
        use tower::ServiceExt;
        let response = crate::router(state)
            .oneshot(
                axum::http::Request::get("/v1/committee")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status().as_u16();
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    fn many(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("0x{i:040x}")).collect()
    }

    #[tokio::test]
    async fn the_committee_is_the_collections_subgraph_s_committee_accounts() {
        use crate::tests::support::FakeCommittee;
        let upper = "0x00000000000000000000000000000000000000AB";
        let fake = FakeCommittee::with(&[MEMBER, upper]);
        assert_eq!(
            committee_route(Some(&fake)).await,
            (
                200,
                serde_json::json!({ "ok": true, "data": [
                    { "id": upper.to_lowercase(), "address": upper.to_lowercase() },
                    { "id": MEMBER, "address": MEMBER },
                ] })
            )
        );
        let unavailable = (
            503,
            serde_json::json!({ "ok": false, "data": {}, "error": "The curation committee is not available. Try again later." }),
        );
        assert_eq!(
            committee_route(Some(&FakeCommittee::down())).await,
            unavailable,
            "a subgraph that fails can't tell"
        );
        assert_eq!(
            committee_route(None).await,
            unavailable,
            "a node without the subgraph can't tell"
        );
        let malformed = FakeCommittee::with(&[MEMBER, "not-an-address"]);
        assert_eq!(
            committee_route(Some(&malformed)).await,
            unavailable,
            "a malformed account can't tell"
        );

        let full = many(MAX_MEMBERS);
        let full = FakeCommittee::with(&full.iter().map(String::as_str).collect::<Vec<_>>());
        let (status, body) = committee_route(Some(&full)).await;
        assert_eq!(
            (
                status,
                body["data"].as_array().map(Vec::len),
                full.committee_hits()
            ),
            (200, Some(MAX_MEMBERS), 2),
            "a full page asks for the next one, as builder-server pages"
        );
        let over = many(MAX_MEMBERS + 1);
        let over = FakeCommittee::with(&over.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(
            committee_route(Some(&over)).await,
            unavailable,
            "a committee past the bound can't tell"
        );
    }

    #[tokio::test]
    async fn the_committee_comes_from_the_subgraph_set_else_from_the_network_s() {
        use crate::tests::support::{FakeCommittee, committee_index, lazy_pool, state_over_with};
        const OTHER: &str = "0x00000000000000000000000000000000000000c3";
        let default = committee_index(&FakeCommittee::with(&[OTHER])).await;
        let network_s = || crate::Settings {
            default_collections_subgraph: default.settings.matic_collections_subgraph_url.clone(),
            ..crate::Settings::default()
        };
        let set = committee_index(&FakeCommittee::with(&[MEMBER])).await;
        let unset = crate::tests::support::market_without_db(
            crate::chain_index::IndexSettings::for_tests(),
        );
        unset.allow_loopback_for_tests();
        let only = |address: &str| serde_json::json!({ "ok": true, "data": [{ "id": address, "address": address }] });
        assert_eq!(
            answer(state_over_with(lazy_pool(), Some(set), network_s()).await).await,
            (200, only(MEMBER)),
            "a set subgraph wins"
        );
        assert_eq!(
            answer(state_over_with(lazy_pool(), Some(unset), network_s()).await).await,
            (200, only(OTHER)),
            "an unset one reads the network's"
        );
        let registry = |set: Option<&str>| {
            crate::tests::support::market_without_db(crate::chain_index::IndexSettings {
                third_party_registry_subgraph_url: set.map(str::to_string),
                ..crate::chain_index::IndexSettings::for_tests()
            })
        };
        let network_registry = || crate::Settings {
            default_registry_subgraph: Some("https://tpr.example/network".into()),
            ..crate::Settings::default()
        };
        for (set, read) in [
            (Some("https://tpr.example/set"), "https://tpr.example/set"),
            (None, "https://tpr.example/network"),
        ] {
            let state = state_over_with(lazy_pool(), Some(registry(set)), network_registry()).await;
            assert_eq!(state.registry_subgraph().as_deref(), Some(read));
        }
    }
}

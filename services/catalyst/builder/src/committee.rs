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
impl Committee {}

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
mod tests {}

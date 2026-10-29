//! Checkout: the settlement side of a Shop purchase that the node executes
//! itself, moved here from `src/economy` (SHOP.md, "One implementation"):
//! the Landiler broker (`broker`: a credits-paid buy the relayer settles
//! with treasury MANA and forwards to the escrow), the escrow reclaim and
//! release (`escrow`), NAME custody (`names`: the registrar mint and the
//! transfer out of custody), the pure calldata builders (`calls`), the
//! idempotency claims of the three ledgers (`claims`), the signed off-chain
//! trade decoding (`trade`) and the receipt reconciler (`reconcile`). The
//! meta-transaction relay, the signers, the chain readers and the contract
//! registry stay in `src/evm`, the shared service this module drives
//! through [`crate::evm::State`].
//!
//! Tables: `shop.broker_purchases`, `shop.escrow_actions`,
//! `shop.name_transfers` (schemas/30_shop.sql). Every route is behind the
//! economy bearer
//! (`CATALYRST_ECONOMY_ADMIN_TOKEN`) and the data-plane write gate
//! (`DATA_WRITES_ENABLED`); the credits fulfilment outbox calls the handlers
//! in process (`shop::credits::escrow::EconomyBroker`).
//!
//! The payment rule is applied before any claim or broadcast
//! (`shop::guards::broker`): a broker buy is a credit-funded one, so
//! `PurchaseMode::Secondary`, a `Trade` over an ERC-721 (the creator's own
//! minted token included) and `NameSecondary` are refused with the documented
//! envelope; a CollectionStore mint, a `COLLECTION_ITEM` trade and the
//! registration of an available NAME are the buys the ledger may fund
//! (`POST /v1/broker/buy`, `POST /v1/broker/names/buy`, rows of
//! [`crate::paths::CREDIT_PATHS`]).

pub mod broker;
pub mod calls;
pub mod claims;
pub mod escrow;
pub mod names;
pub mod reconcile;
pub mod trade;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::post;

use crate::evm::{SharedState, State};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::writes::DataWrites;

/// The lifecycle of a broker purchase row (`shop.broker_purchases.status`),
/// in the order the CHECK names them; the reconciler moves `Sent`/`Bought`/
/// `Forwarding` rows to a terminal state from receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PurchaseStatus {
    Pending,
    Sent,
    Reverted,
    Bought,
    Forwarding,
    Confirmed,
    Error,
}

impl PurchaseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Reverted => "reverted",
            Self::Bought => "bought",
            Self::Forwarding => "forwarding",
            Self::Confirmed => "confirmed",
            Self::Error => "error",
        }
    }

    /// The rows the reconciler polls receipts for (the partial index's predicate).
    pub fn is_unsettled(self) -> bool {
        matches!(self, Self::Sent | Self::Bought | Self::Forwarding)
    }

    /// Only a confirmed purchase answers 200 with its hash.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Reverted | Self::Confirmed | Self::Error)
    }
}

/// The lifecycle of an escrow action or a NAME transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Pending,
    Sent,
    Confirmed,
    Reverted,
    Error,
}

impl ActionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sent => "sent",
            Self::Confirmed => "confirmed",
            Self::Reverted => "reverted",
            Self::Error => "error",
        }
    }
}

/// The broker, escrow and NAME custody routes (the old economy `api_router`
/// entries), behind the write gate; `main.rs` merges the router at the root
/// and under `/transactions-api`, the same handlers at both mounts
/// (`/shop-api` carries the operator approvals, [`shop_router`]).
pub fn router(state: SharedState) -> Router {
    let enabled = state.settings.data_writes_enabled;
    let api = Router::new()
        .route("/v1/broker/buy", post(broker::buy))
        .route(
            "/v1/admin/collections/approve-mana",
            post(broker::approve_mana_collections),
        )
        .route("/v1/broker/names/buy", post(names::buy))
        .route("/v1/broker/names/transfer", post(names::transfer))
        .route("/v1/admin/names/approve-mana", post(names::approve_mana))
        .route("/v1/escrow/reclaim", post(escrow::reclaim))
        .route("/v1/escrow/release", post(escrow::release))
        .with_state(state);
    DataWrites::guard(api, enabled)
}

/// The canonical operator paths of the two MANA approvals (`/shop-api/admin/*`).
pub fn shop_router(state: SharedState) -> Router {
    let enabled = state.settings.data_writes_enabled;
    let api = Router::new()
        .route(
            "/admin/broker/approve-mana",
            post(broker::approve_mana_collections),
        )
        .route("/admin/names/approve-mana", post(names::approve_mana))
        .with_state(state);
    DataWrites::guard(api, enabled)
}

/// The broker reconciler, when a direct signer exists to read receipts with.
pub fn spawn_jobs(state: &Arc<State>, jobs: &Jobs) {
    if !state.transaction.has_direct_signer() && state.eth_signer.is_none() {
        tracing::info!(
            "checkout: broker reconciler not started (no direct JSON-RPC signer; nothing to poll receipts with)"
        );
        return;
    }
    let state = state.clone();
    jobs.spawn_periodic_leased(
        "broker-reconcile",
        PeriodicCfg::every(
            state
                .settings
                .broker_reconcile_interval
                .max(Duration::from_secs(1)),
        )
        .with_jitter(Duration::from_secs(10))
        .with_error_backoff(Duration::from_secs(600)),
        move |ctx| {
            let state = state.clone();
            async move { reconcile::run_pass(&state, &ctx).await }
        },
    );
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use crate::db::SCHEMA_SQL;

    #[test]
    fn the_schema_carries_every_checkout_table_this_module_names() {
        for table in [
            "broker_purchases",
            "escrow_actions",
            "name_transfers",
            "usage_grants",
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE TABLE shop.{table} (")),
                "{table}"
            );
        }
    }

    #[test]
    fn purchase_statuses_match_the_check_and_the_reconciler_predicate() {
        let all = [
            PurchaseStatus::Pending,
            PurchaseStatus::Sent,
            PurchaseStatus::Reverted,
            PurchaseStatus::Bought,
            PurchaseStatus::Forwarding,
            PurchaseStatus::Confirmed,
            PurchaseStatus::Error,
        ];
        let names: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
        let check = format!(
            "CHECK (status IN ({}))",
            names
                .iter()
                .map(|n| format!("'{n}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(SCHEMA_SQL.contains(&check), "{check}");
        let unsettled: Vec<&str> = all
            .iter()
            .filter(|s| s.is_unsettled())
            .map(|s| s.as_str())
            .collect();
        assert_eq!(unsettled, ["sent", "bought", "forwarding"]);
        assert!(SCHEMA_SQL.contains("WHERE status IN ('sent', 'bought', 'forwarding')"));
        assert!(all.iter().all(|s| !(s.is_unsettled() && s.is_terminal())));
        assert_eq!(ActionStatus::Confirmed.as_str(), "confirmed");
        for mode in [
            calls::PurchaseMode::Primary,
            calls::PurchaseMode::Secondary,
            calls::PurchaseMode::Trade,
            calls::PurchaseMode::NameMint,
            calls::PurchaseMode::NameSecondary,
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("'{}'", mode.as_str()))
                    || mode == calls::PurchaseMode::Primary,
                "mode {} must be in the CHECK",
                mode.as_str()
            );
        }
    }
}

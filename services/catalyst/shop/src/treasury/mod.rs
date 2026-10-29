//! Treasury: what the node's relayer account holds and receives, moved here
//! from `src/evm` (SHOP.md, "One implementation"): the MANA payment
//! readers and the receipt verifier of a buyer's MANA top-up (`payments`:
//! `/v1/payments/{config,nonce,balance,verify}`, the old
//! transactions-server routes, now also the canonical
//! `/shop-api/v1/credit-topups/mana/{config,nonce,balance}` reads). The
//! chain reader and the signers are `crate::evm`'s.
//!
//! Not here yet: shop-server's treasury deposits, settlements, withdrawals,
//! the refill and swap jobs and the drift checks
//! (`src/treasury::{refill,swap,reconcile,notifications}`), and the card
//! receivable versus USDC received distinction they keep.

pub mod payments;

use axum::Router;
use axum::routing::{get, post};

use crate::evm::SharedState;
use crate::shared::writes::DataWrites;

/// The payments routes behind the write gate; `POST /v1/payments/verify`
/// reads a receipt and stays open while writes are disabled
/// (`DataWrites::OPEN_WHILE_DISABLED`). `main.rs` merges the router at the
/// root and under `/transactions-api`.
pub fn router(state: SharedState) -> Router {
    let enabled = state.settings.data_writes_enabled;
    let api = Router::new()
        .route("/v1/payments/config", get(payments::config))
        .route("/v1/payments/nonce/{address}", get(payments::nonce))
        .route("/v1/payments/balance/{address}", get(payments::balance))
        .route("/v1/payments/verify", post(payments::verify))
        .with_state(state);
    DataWrites::guard(api, enabled)
}

/// The canonical Shop reads of the same handlers (`/shop-api` nest): the
/// MANA top-up guidance the UI renders (network, nonce, balance).
pub fn shop_router(state: SharedState) -> Router {
    Router::new()
        .route("/v1/credit-topups/mana/config", get(payments::config))
        .route(
            "/v1/credit-topups/mana/nonce/{address}",
            get(payments::nonce),
        )
        .route(
            "/v1/credit-topups/mana/balance/{address}",
            get(payments::balance),
        )
        .with_state(state)
}

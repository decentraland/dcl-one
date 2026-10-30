//! Economy: the meta-transaction relay, the Landiler broker/escrow/NAME
//! custody (the old `catalyrst-economy`) over the ONE pool
//! (`schemas/31_economy.sql`), the ONE client (`crate::http::Outbound` for
//! the OZ relayer, the upstream forward, the JSON-RPC endpoints and the
//! contract-addresses JSON, every call through the SSRF guard) and
//! `jobs::spawn_periodic` for the broker reconciler. The MANA spot archive
//! (`economy.price_snapshots`, the old `catalyrst-price`) shares the schema
//! but is not part of this module yet.
//!
//! Where each route reads:
//! - `POST /v1/transactions`: local, the old policy unchanged: the audited
//!   EIP-712 domain table, the selector allowlist, the contract allowlist
//!   (`CONTRACT_ADDRESSES_URL` JSON ∪ `shop.collections`, the mirror
//!   that replaces the crate's one `squid.collection` read; while the
//!   mirror has no complete pass an address outside the JSON is a 503, never
//!   "not a collection"), `MIN_SALE_VALUE_IN_WEI`,
//!   `MAX_GAS_PRICE_ALLOWED_IN_WEI`, the caller's simulation, the per-day
//!   quota reserved in `economy.transactions` under a per-user advisory lock
//!   with the request's clock bound. v2 never signs a user's transaction;
//!   the relayer key (`RELAYER_PRIVATE_KEY_FILE`), the OZ credentials and
//!   the admin bearer come from `*_FILE` or the unit's EnvironmentFile and
//!   are never logged or stored. Broadcast by the OZ relayer, the direct
//!   signer (the key signs an EIP-1559 transaction locally and the raw bytes
//!   go out as `eth_sendRawTransaction` through the shared client) or the
//!   upstream forward, in that order under the Auto preference.
//! - `GET /v1/transactions/{address}`: local.
//! - `GET /v1/contracts/{address}`: `shop.collections` ∪ the
//!   whitelist cache; the unavailable envelope (`marketplace::unavailable`,
//!   `Source::CollectionsMirror`) while the collections mirror has no
//!   complete pass on the relay's network.
//! - `/v1/payments/{config,nonce,balance,verify}`: config and JSON-RPC
//!   readers over `RPC_ENDPOINT_POLYGON` (no table); `verify` stays open
//!   under the write gate.
//! - `/v1/broker/*`, `/v1/escrow/*`, `/v1/admin/*`: local ledgers
//!   (`broker_purchases`, `escrow_actions`, `name_transfers`, claimed by
//!   idempotency key before any broadcast) + chain, behind the admin bearer
//!   (`CATALYRST_ECONOMY_ADMIN_TOKEN`; the old loopback exemption is gone
//!   with the per-service listener) and the write gate.
//! - `broker-reconcile` job: the three ledgers' unsettled rows -> receipts.
//!
//! Every write is local authority behind `DATA_WRITES_ENABLED`
//! (`crate::shared::writes::DataWrites`, the social switch's twin): off, every
//! non-GET route but `/v1/payments/verify` answers the 503 envelope.

pub mod abi;
pub mod admin;
pub mod chain;
pub mod contracts;
pub mod contracts_addrs;
pub mod eip712;
pub mod errors;
pub mod handlers;
pub mod meta_tx;
pub mod oracle;
pub mod relayer;
pub mod rpc;
pub mod signer;
pub mod transaction;
pub mod transfer_shortfall;
pub mod upstream;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, U256};
use axum::Router;
use axum::routing::{get, post};
use chrono::NaiveDateTime;
use sqlx::PgPool;

use crate::http::Outbound;
use crate::shared::writes::DataWrites;

use admin::RuntimeConfig;
use chain::{ChainComponent, DEFAULT_RPC_TIMEOUT};
use contracts::ContractsComponent;
use relayer::Relayer;
use signer::DirectSigner;
use transaction::TransactionComponent;
use upstream::UpstreamForwarder;

/// The schema every economy statement qualifies its tables with.
pub const SCHEMA: &str = "economy";

/// The relay's per-user quota window, as the old transactions-server counted it.
pub const QUOTA_WINDOW_SECS: i64 = 24 * 60 * 60;

/// The request clock every statement binds (UTC, `timestamp without time zone`).
pub fn now() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// The old crate's env table, filled by `main.rs` from `Config` (the secrets
/// through `<NAME>_FILE`). No `Debug`: three fields are credentials.
#[derive(Clone)]
pub struct Settings {
    /// `MIN_SALE_VALUE_IN_WEI` (1 MANA): the floor on the store/marketplace/bid sale price.
    pub min_sale_value_in_wei: String,
    /// `MAX_TRANSACTIONS_PER_DAY` (10): the per-address relay quota.
    pub max_transactions_per_day: i64,
    /// `CONTRACT_ADDRESSES_URL`: the dApps' contract-addresses JSON; unset, only the mirror admits.
    pub contract_addresses_url: Option<String>,
    /// `CONTRACT_ADDRESSES_CHAIN_KEY` (matic): the key inside that JSON.
    pub contract_addresses_chain_key: String,
    /// `COLLECTIONS_CHAIN_ID` (137): the collections chain the relay targets.
    pub collections_chain_id: u64,
    /// `COLLECTIONS_FETCH_INTERVAL_MS` (3600000): the whitelist cache TTL.
    pub collections_fetch_interval: Duration,
    /// `RPC_ENDPOINT_POLYGON`: the collections chain's JSON-RPC (the old `RPC_URL`).
    pub polygon_rpc_url: Option<String>,
    /// `MAX_GAS_PRICE_ALLOWED_IN_WEI`: the gas-price ceiling of the relay.
    pub max_gas_price_allowed_in_wei: Option<u128>,
    /// `MAX_GAS_LIMIT` (1500000): the direct signer's gas cap; 0 disables it.
    pub max_gas_limit: u64,
    /// `OZ_RELAYER_URL`, `OZ_RELAYER_ID`, `OZ_RELAYER_API_KEY` (`_FILE`): the OZ Defender relayer, active with all three.
    pub oz_relayer_url: Option<String>,
    pub oz_relayer_id: Option<String>,
    pub oz_relayer_api_key: Option<String>,
    /// `OZ_RELAYER_SPEED` (fast), `OZ_MAX_STATUS_CHECKS` (150), `OZ_SLEEP_TIME_BETWEEN_CHECKS_MS` (800).
    pub oz_relayer_speed: String,
    pub oz_max_status_checks: u32,
    pub oz_sleep_between_checks: Duration,
    /// `META_TX_BROADCAST_ENABLED` (false): arms the direct signers.
    pub meta_tx_broadcast_enabled: bool,
    /// `RELAYER_PRIVATE_KEY` via `RELAYER_PRIVATE_KEY_FILE`: the relayer EOA's key.
    pub relayer_private_key: Option<String>,
    /// `TRANSACTIONS_UPSTREAM_URL`, `TRANSACTIONS_UPSTREAM_TIMEOUT_MS` (30000): the forward fallback.
    pub transactions_upstream_url: Option<String>,
    pub transactions_upstream_timeout: Duration,
    /// `CATALYRST_ECONOMY_ADMIN_TOKEN` (`_FILE`): the bearer of the money routes; unset refuses them.
    pub admin_token: Option<String>,
    /// `LANDILER_ESCROW_ADDRESS`: the one escrow broker buys may deliver to.
    pub landiler_escrow_address: Option<String>,
    /// `NAMES_CHAIN_ID` (1), `ETH_RPC_URL` (`Config::eth_rpc_url()`), `NAMES_MAX_PRICE_WEI`.
    pub names_chain_id: u64,
    pub eth_rpc_url: Option<String>,
    pub names_max_price_wei: Option<U256>,
    /// `RECEIPT_POLL_INTERVAL_MS` (3000), `RECEIPT_TIMEOUT_MS` (180000), `BROKER_RECONCILE_INTERVAL_MS` (60000).
    pub receipt_poll_interval: Duration,
    pub receipt_timeout: Duration,
    pub broker_reconcile_interval: Duration,
    /// `MANA_USD_AGGREGATOR_ADDRESS`, `USD_PEGGED_ORACLE_MAX_AGE_SECS` (60), `USD_PEGGED_SLIPPAGE_BPS` (100).
    pub mana_usd_aggregator: Address,
    pub usd_pegged_oracle_max_age_secs: u64,
    pub usd_pegged_slippage_bps: u64,
    /// `DATA_WRITES_ENABLED` (false): the write gate.
    pub data_writes_enabled: bool,
}

impl Settings {
    /// The old crate's defaults; `main.rs` overrides from `Config`.
    pub fn defaults() -> Self {
        Self {
            min_sale_value_in_wei: "1000000000000000000".to_string(),
            max_transactions_per_day: 10,
            contract_addresses_url: None,
            contract_addresses_chain_key: "matic".to_string(),
            collections_chain_id: 137,
            collections_fetch_interval: Duration::from_millis(3_600_000),
            polygon_rpc_url: None,
            max_gas_price_allowed_in_wei: None,
            max_gas_limit: 1_500_000,
            oz_relayer_url: None,
            oz_relayer_id: None,
            oz_relayer_api_key: None,
            oz_relayer_speed: "fast".to_string(),
            oz_max_status_checks: 150,
            oz_sleep_between_checks: Duration::from_millis(800),
            meta_tx_broadcast_enabled: false,
            relayer_private_key: None,
            transactions_upstream_url: None,
            transactions_upstream_timeout: upstream::DEFAULT_UPSTREAM_TIMEOUT,
            admin_token: None,
            landiler_escrow_address: None,
            names_chain_id: 1,
            eth_rpc_url: None,
            names_max_price_wei: None,
            receipt_poll_interval: Duration::from_millis(3_000),
            receipt_timeout: Duration::from_millis(180_000),
            broker_reconcile_interval: Duration::from_millis(60_000),
            mana_usd_aggregator: oracle::MANA_USD_AGGREGATOR_POLYGON,
            usd_pegged_oracle_max_age_secs: 60,
            usd_pegged_slippage_bps: 100,
            data_writes_enabled: false,
        }
    }

    pub fn for_tests() -> Self {
        Self {
            receipt_poll_interval: Duration::from_millis(20),
            receipt_timeout: Duration::from_millis(500),
            oz_max_status_checks: 3,
            oz_sleep_between_checks: Duration::from_millis(10),
            mana_usd_aggregator: Address::ZERO,
            data_writes_enabled: true,
            ..Self::defaults()
        }
    }

    fn has(o: &Option<String>) -> bool {
        o.as_deref().is_some_and(|s| !s.trim().is_empty())
    }

    pub fn has_relayer(&self) -> bool {
        Self::has(&self.oz_relayer_url)
            && Self::has(&self.oz_relayer_id)
            && Self::has(&self.oz_relayer_api_key)
    }

    pub fn has_rpc(&self) -> bool {
        Self::has(&self.polygon_rpc_url)
    }

    pub fn has_direct_broadcast(&self) -> bool {
        self.meta_tx_broadcast_enabled && Self::has(&self.relayer_private_key) && self.has_rpc()
    }

    pub fn has_upstream_forward(&self) -> bool {
        Self::has(&self.transactions_upstream_url)
    }

    /// Locally-provisioned relayers win over the upstream forward: the
    /// upstream is the fallback broadcast path for nodes without real
    /// relayer creds.
    pub fn relay_mode(&self) -> &'static str {
        if self.has_relayer() {
            "oz"
        } else if self.has_direct_broadcast() {
            "direct"
        } else if self.has_upstream_forward() {
            "upstream"
        } else {
            "none"
        }
    }

    pub fn can_relay(&self) -> bool {
        self.relay_mode() != "none"
    }

    /// Every configured endpoint must be a publicly routable http(s) URL
    /// (the guard refuses anything else at send time too; this names the
    /// variable at startup instead of at the first relay). `ETH_RPC_URL`
    /// is checked by `Config::eth_rpc_url()` already. Values are never echoed.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (var, value) in [
            ("RPC_ENDPOINT_POLYGON", &self.polygon_rpc_url),
            ("CONTRACT_ADDRESSES_URL", &self.contract_addresses_url),
            ("OZ_RELAYER_URL", &self.oz_relayer_url),
            ("TRANSACTIONS_UPSTREAM_URL", &self.transactions_upstream_url),
        ] {
            if let Some(url) = value.as_deref().map(str::trim).filter(|u| !u.is_empty())
                && !crate::http::is_safe_http_url(url)
            {
                anyhow::bail!("{var} is not a publicly routable http(s) URL");
            }
        }
        if self.meta_tx_broadcast_enabled && Self::has(&self.relayer_private_key) && !self.has_rpc()
        {
            anyhow::bail!(
                "META_TX_BROADCAST_ENABLED=true and RELAYER_PRIVATE_KEY set, but RPC_ENDPOINT_POLYGON is empty"
            );
        }
        if let Some(raw) = self.landiler_escrow_address.as_deref()
            && raw.trim().parse::<Address>().is_err()
        {
            anyhow::bail!("LANDILER_ESCROW_ADDRESS is not a valid address");
        }
        if oracle::exceeds_onchain_stale_tolerance(self.usd_pegged_oracle_max_age_secs) {
            tracing::warn!(
                max_age_secs = self.usd_pegged_oracle_max_age_secs,
                onchain_tolerance_secs = oracle::ONCHAIN_MANA_USD_STALE_TOLERANCE_SECS,
                "USD_PEGGED_ORACLE_MAX_AGE_SECS is looser than the deployed marketplace manaUsdAggregatorTolerance; \
                 the chain refuses at execution what this bound accepts"
            );
        }
        Ok(())
    }
}

pub struct State {
    pub pool: PgPool,
    pub http: Outbound,
    pub settings: Settings,
    pub transaction: TransactionComponent,
    pub contracts: ContractsComponent,
    /// The NAMEs chain signer (Ethereum), when the key and `ETH_RPC_URL` are set.
    pub eth_signer: Option<Arc<DirectSigner>>,
    pub runtime: Arc<RuntimeConfig>,
}

pub type SharedState = Arc<State>;

impl State {
    /// Provisions the broadcast providers the settings describe (the old
    /// `build_state`, without the migrations: `schema.sql` owns the DDL).
    /// The log lines name the mode and the relayer address, never a URL or
    /// a key.
    pub fn new(pool: PgPool, http: Outbound, settings: Settings) -> anyhow::Result<Self> {
        let chain = settings
            .polygon_rpc_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|url| Arc::new(ChainComponent::new(http.clone(), url, DEFAULT_RPC_TIMEOUT)));
        let relayer = settings.has_relayer().then(|| {
            Relayer::new(
                http.clone(),
                settings.oz_relayer_url.as_deref().unwrap_or_default(),
                settings.oz_relayer_id.as_deref().unwrap_or_default(),
                settings.oz_relayer_api_key.as_deref().unwrap_or_default(),
                &settings.oz_relayer_speed,
                settings.oz_max_status_checks,
                settings.oz_sleep_between_checks,
            )
        });
        let key = settings
            .relayer_private_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty());
        let build_signer =
            |chain: &Arc<ChainComponent>, chain_id: u64| -> anyhow::Result<Arc<DirectSigner>> {
                Ok(Arc::new(
                    DirectSigner::new(
                        chain.clone(),
                        key.expect("a key gates the signer"),
                        chain_id,
                        settings.max_gas_limit,
                        settings.receipt_poll_interval,
                        settings.receipt_timeout,
                    )
                    .map_err(anyhow::Error::msg)?,
                ))
            };
        let signer = match (&chain, key, settings.meta_tx_broadcast_enabled) {
            (Some(chain), Some(_), true) => {
                Some(build_signer(chain, settings.collections_chain_id)?)
            }
            (None, Some(_), true) => {
                anyhow::bail!(
                    "META_TX_BROADCAST_ENABLED=true and RELAYER_PRIVATE_KEY set, but RPC_ENDPOINT_POLYGON is empty"
                )
            }
            _ => None,
        };
        let upstream = settings
            .transactions_upstream_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|url| {
                UpstreamForwarder::new(http.clone(), url, settings.transactions_upstream_timeout)
            });
        let eth_signer = match (
            settings
                .eth_rpc_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty()),
            key,
            settings.meta_tx_broadcast_enabled,
        ) {
            (Some(url), Some(_), true) => {
                let eth_chain =
                    Arc::new(ChainComponent::new(http.clone(), url, DEFAULT_RPC_TIMEOUT));
                Some(build_signer(&eth_chain, settings.names_chain_id)?)
            }
            _ => None,
        };
        match (&relayer, &signer, &upstream) {
            (Some(_), _, _) => tracing::info!(
                "economy: OZ relayer provisioned; meta-transaction broadcast via OZ Defender"
            ),
            (None, Some(s), _) => tracing::info!(
                relayer = %s.relayer_address(),
                chain_id = settings.collections_chain_id,
                "economy: direct JSON-RPC broadcast enabled; ensure the relayer address is funded for gas"
            ),
            (None, None, Some(_)) => tracing::info!(
                "economy: upstream forward enabled; POST /v1/transactions relays the validated body to the upstream"
            ),
            (None, None, None) => {
                tracing::warn!(
                    "economy: no broadcast provider provisioned; POST /v1/transactions validates + answers 503"
                )
            }
        }
        match &eth_signer {
            Some(s) => tracing::info!(
                relayer = %s.relayer_address(),
                chain_id = settings.names_chain_id,
                "economy: NAMEs chain signer enabled; ensure the relayer address holds ETH for gas and MANA on Ethereum"
            ),
            None => tracing::info!(
                "economy: NAMEs chain signer not provisioned (ETH_RPC_URL + the key enable /v1/broker/names)"
            ),
        }
        let contracts = ContractsComponent::new(
            pool.clone(),
            http.clone(),
            settings.contract_addresses_url.clone(),
            settings.contract_addresses_chain_key.clone(),
            settings.collections_chain_id,
            settings.collections_fetch_interval,
        )
        .with_chain(chain.clone());
        let runtime = RuntimeConfig::new();
        let transaction = TransactionComponent::new(
            pool.clone(),
            relayer,
            signer,
            upstream,
            chain,
            runtime.clone(),
        );
        Ok(Self {
            pool,
            http,
            settings,
            transaction,
            contracts,
            eth_signer,
            runtime,
        })
    }
}

/// The relay's `/v1` routes, behind the write gate: `main.rs` merges it at
/// the root and nests it under `/transactions-api`, beside the Shop's
/// checkout and treasury routers over the same state
/// (`shop::checkout::router`, `shop::treasury::router`), which hold the
/// broker, escrow, NAME custody and payments routes.
pub fn router(state: SharedState) -> Router {
    let enabled = state.settings.data_writes_enabled;
    let api = Router::new()
        .route(
            "/v1/transactions",
            post(handlers::transactions::send_transaction),
        )
        .route(
            "/v1/transactions/{user_address}",
            get(handlers::transactions::get_user_transactions),
        )
        .route(
            "/v1/contracts/{address}",
            get(handlers::contracts::contracts_address),
        )
        .route("/v1/admin/relayer", get(handlers::admin::relayer_status))
        .route(
            "/v1/admin/relayer/toggle",
            post(handlers::admin::relayer_toggle),
        )
        .route(
            "/v1/admin/relayer/signer",
            post(handlers::admin::relayer_signer),
        )
        .with_state(state);
    DataWrites::guard(api, enabled)
}

/// `GET /contracts/addresses.json`, the contract map the old main vhost
/// served from the node's own tree (root only).
pub fn contracts_document_router() -> Router {
    Router::new().route(
        "/contracts/addresses.json",
        get(contracts::addresses_document),
    )
}

/// The old service's `/health` and `/ping`, for the `/transactions-api`
/// nest only (the content server owns both at the root).
pub fn health_router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/ping", get(handlers::health::ping))
        .with_state(state)
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use crate::db::SCHEMA_SQL;

    #[test]
    fn the_schema_carries_every_economy_table_this_module_names() {
        for table in [
            "transactions",
            "price_snapshots",
            "price_overrides",
            "price_override_audit",
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE TABLE {SCHEMA}.{table} (")),
                "{table}"
            );
        }
        assert!(
            !SCHEMA_SQL.contains("FROM squid_marketplace")
                && !SCHEMA_SQL.contains("JOIN squid_marketplace"),
            "v2 has no squid database"
        );
    }

    #[test]
    fn relay_mode_prefers_local_providers_and_validate_names_the_variable() {
        let mut s = Settings::defaults();
        assert_eq!(s.relay_mode(), "none");
        assert!(!s.can_relay());
        s.transactions_upstream_url = Some("https://transactions-api.decentraland.org".into());
        assert_eq!(s.relay_mode(), "upstream");
        s.meta_tx_broadcast_enabled = true;
        s.relayer_private_key = Some("00".repeat(32));
        assert_eq!(
            s.relay_mode(),
            "upstream",
            "a key without an RPC is not a direct signer"
        );
        let err = s.validate().unwrap_err().to_string();
        assert!(err.contains("RPC_ENDPOINT_POLYGON"), "{err}");
        s.polygon_rpc_url = Some("https://polygon-rpc.example/v3/SECRET".into());
        assert_eq!(s.relay_mode(), "direct");
        assert!(s.validate().is_ok());
        s.oz_relayer_url = Some("https://api.defender.example".into());
        s.oz_relayer_id = Some("id".into());
        s.oz_relayer_api_key = Some("key".into());
        assert_eq!(s.relay_mode(), "oz");
        s.polygon_rpc_url = Some("http://127.0.0.1:8545".into());
        let err = s.validate().unwrap_err().to_string();
        assert!(
            err.contains("RPC_ENDPOINT_POLYGON") && !err.contains("127.0.0.1"),
            "{err}"
        );
    }
}

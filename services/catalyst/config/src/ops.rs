//! The operator plane's settings: telemetry, the RPC relay and the console.

use clap::Args;

/// The operator plane's switches and the telemetry console and sinks.
#[derive(Args, Clone, Debug)]
pub struct OpsConfig {
    // ops: the operator plane (src/ops, src/credits, src/explore/imposters).
    // The credits ledger, the telemetry console writes and the console's
    // mutating routes are local authority, so they sit behind the social
    // switch's twin; the telemetry sinks are not user state and have their
    // own switch (`ops::TELEMETRY_INGEST_ENABLED`).
    #[arg(long, env = "OPS_WRITES_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub ops_writes_enabled: bool,

    /// Accept Sentry envelopes and Segment batches into telemetry.events; off, the sinks answer 503.
    #[arg(long, env = "TELEMETRY_INGEST_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub telemetry_ingest_enabled: bool,

    // ops/telemetry (the old catalyrst-telemetry): the console bearer, the
    // event contract, the flags document, the nest prefix and retention.
    /// The console bearer (or `_FILE`); unset, every admin route answers 403.
    #[arg(long, env = "CATALYRST_TELEMETRY_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_telemetry_admin_token: Option<String>,

    /// telemetry-contract.json; unset, missing or unparseable disables validation (accept all).
    #[arg(long, env = "TELEMETRY_CONTRACT_PATH")]
    pub telemetry_contract_path: Option<String>,

    /// Comma-separated write keys the contract validates; unset = all.
    #[arg(long, env = "TELEMETRY_CONTRACT_PROJECTS")]
    pub telemetry_contract_projects: Option<String>,

    /// A public http(s) flags document for /dash/flags; unset reads the explore_api.feature_flags mirror.
    #[arg(long, env = "TELEMETRY_FLAGS_URL")]
    pub telemetry_flags_url: Option<String>,

    /// The prefix the telemetry router is nested under (the pages and the login cookie carry it).
    #[arg(long, env = "TELEMETRY_BASE_PATH", default_value = "/telemetry")]
    pub telemetry_base_path: String,

    #[arg(long, env = "TELEMETRY_RETENTION_DAYS", default_value_t = 90)]
    pub telemetry_retention_days: u32,

    /// 5,000-row purge batches per daily pass.
    #[arg(long, env = "TELEMETRY_PURGE_MAX_BATCHES", default_value_t = 100)]
    pub telemetry_purge_max_batches: u32,

    /// Rows per UTC day for a project with no telemetry.project_quotas row (0 refuses unknown projects).
    #[arg(long, env = "TELEMETRY_DEFAULT_DAILY_LIMIT", default_value_t = 50_000)]
    pub telemetry_default_daily_limit: i64,

    /// The process-wide ingest ceiling in rows per minute (300 = 432,000 a day, under one purge pass).
    #[arg(long, env = "TELEMETRY_MAX_ROWS_PER_MINUTE", default_value_t = 300)]
    pub telemetry_max_rows_per_minute: u32,

    /// The process-wide ingest ceiling in decoded request bytes per minute.
    #[arg(long, env = "TELEMETRY_MAX_BYTES_PER_MINUTE", default_value_t = 4 * 1024 * 1024)]
    pub telemetry_max_bytes_per_minute: u64,

    /// The DSN POST /telemetry/dash/sql logs in with (or `_FILE`): a role holding USAGE + SELECT on schema telemetry and nothing else (deploy/nix/postgresql.nix); unset, the route answers 503.
    #[arg(long, env = "TELEMETRY_SQL_DATABASE_URL", hide_env_values = true)]
    pub telemetry_sql_database_url: Option<String>,
}

/// The read-only EVM relay.
#[derive(Args, Clone, Debug)]
pub struct RpcConfig {
    // ops/rpc (the old catalyrst-rpc): the read-only EVM relay. Gateway keys
    // sit in the upstream URLs, so every one is hidden and readable from `_FILE`.
    /// The relay's admin bearer (or `_FILE`); unset, /rpc/admin/rpc/* answers 403.
    #[arg(long, env = "CATALYRST_RPC_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_rpc_admin_token: Option<String>,

    /// Admit self-hosted upstreams by internal name (the host's own stack stays refused).
    #[arg(long, env = "CATALYRST_RPC_ALLOW_PRIVATE_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub catalyrst_rpc_allow_private_upstream: bool,

    #[arg(long, env = "RPC_UPSTREAM_MAINNET", hide_env_values = true)]
    pub rpc_upstream_mainnet: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_ETHEREUM", hide_env_values = true)]
    pub rpc_upstream_ethereum: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_SEPOLIA", hide_env_values = true)]
    pub rpc_upstream_sepolia: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_POLYGON", hide_env_values = true)]
    pub rpc_upstream_polygon: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_MATIC", hide_env_values = true)]
    pub rpc_upstream_matic: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_AMOY", hide_env_values = true)]
    pub rpc_upstream_amoy: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_MUMBAI", hide_env_values = true)]
    pub rpc_upstream_mumbai: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_ARBITRUM", hide_env_values = true)]
    pub rpc_upstream_arbitrum: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_OPTIMISM", hide_env_values = true)]
    pub rpc_upstream_optimism: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_AVALANCHE", hide_env_values = true)]
    pub rpc_upstream_avalanche: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_BINANCE", hide_env_values = true)]
    pub rpc_upstream_binance: Option<String>,
    #[arg(long, env = "RPC_UPSTREAM_FANTOM", hide_env_values = true)]
    pub rpc_upstream_fantom: Option<String>,
}

/// The operator console's sign-in and its abgen admin link.
#[derive(Args, Clone, Debug)]
pub struct AdminConfig {
    // ops/admin (the old catalyrst-live console): SIWE sign-in into an HMAC cookie.
    /// The console's cookie and nonce HMAC key (or `_FILE`); unset disables sign-in.
    #[arg(long, env = "SESSION_SECRET", hide_env_values = true)]
    pub session_secret: Option<String>,

    /// Comma-separated wallets allowed to sign in; empty disables the console.
    #[arg(long, env = "ADMIN_ADDRESSES", default_value = "")]
    pub admin_addresses: String,

    #[arg(long, env = "ADMIN_SESSION_TTL_SECS", default_value_t = 43_200)]
    pub admin_session_ttl_secs: i64,

    /// Drop the cookie's Secure flag (plain-http labs only).
    #[arg(long, env = "ADMIN_COOKIE_INSECURE", default_value_t = false, action = clap::ArgAction::Set)]
    pub admin_cookie_insecure: bool,

    /// The abgen runtime's admin endpoint the console's `/admin/api/create/*`
    /// calls (registry re-ingest, cache flush, queues, denylist): a loopback
    /// URL only (abgen binds `127.0.0.1:5147`); unset, those routes answer the
    /// unconfigured-service envelope.
    #[arg(long, env = "ABGEN_ADMIN_URL")]
    pub abgen_admin_url: Option<String>,

    /// abgen's admin bearer (or `_FILE`) for its queue and denylist controls.
    #[arg(long, env = "AB_REGISTRY_ADMIN_TOKEN", hide_env_values = true)]
    pub ab_registry_admin_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    fn parse(args: &[&str]) -> Result<Config, clap::Error> {
        Config::parse_for_tests(args)
    }

    #[test]
    fn ops_defaults_are_the_old_binaries_values_and_writes_are_off() {
        let c = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        assert!(
            !c.ops.ops_writes_enabled,
            "operator-plane writes are opt-in; a restore must be demonstrated first"
        );
        assert!(
            !c.ops.telemetry_ingest_enabled,
            "the sinks have their own switch, off by default"
        );
        assert!(
            c.ops.catalyrst_telemetry_admin_token.is_none()
                && c.ops.telemetry_contract_path.is_none()
                && c.ops.telemetry_contract_projects.is_none()
                && c.ops.telemetry_flags_url.is_none()
        );
        assert_eq!(
            (
                c.ops.telemetry_base_path.as_str(),
                c.ops.telemetry_retention_days,
                c.ops.telemetry_purge_max_batches
            ),
            ("/telemetry", 90, 100)
        );
        assert_eq!(
            (
                c.ops.telemetry_default_daily_limit,
                c.ops.telemetry_max_rows_per_minute,
                c.ops.telemetry_max_bytes_per_minute
            ),
            (50_000, 300, 4 * 1024 * 1024),
            "the ingest bounds apply without an operator row"
        );
        assert!(
            c.ops.telemetry_sql_database_url.is_none(),
            "the SQL console has no login until one is provisioned; it never falls back to the runtime pool"
        );
        assert!(
            c.rpc.catalyrst_rpc_admin_token.is_none()
                && !c.rpc.catalyrst_rpc_allow_private_upstream
        );
        assert!(
            c.rpc.rpc_upstream_mainnet.is_none()
                && c.rpc.rpc_upstream_polygon.is_none()
                && c.rpc.rpc_upstream_fantom.is_none(),
            "no default upstream: an unset network is unsupported"
        );
        assert!(
            c.admin.session_secret.is_none()
                && c.admin.admin_addresses.is_empty()
                && !c.admin.admin_cookie_insecure
        );
        assert_eq!(c.admin.admin_session_ttl_secs, 43_200);
        assert!(c.admin.abgen_admin_url.is_none() && c.admin.ab_registry_admin_token.is_none());

        let c = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--ops-writes-enabled",
            "true",
            "--telemetry-ingest-enabled",
            "true",
            "--credits-mock-card",
            "true",
            "--ab-cdn-enabled",
            "false",
            "--admin-addresses",
            "0xAbC, 0xdef",
            "--rpc-upstream-polygon",
            "https://rpc.example/polygon",
        ])
        .unwrap();
        assert_eq!(
            (
                c.admin.admin_addresses.as_str(),
                c.rpc.rpc_upstream_polygon.as_deref()
            ),
            ("0xAbC, 0xdef", Some("https://rpc.example/polygon"))
        );
    }
}

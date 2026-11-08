//! Ops: the operator plane (schema `ops`, `schemas/60_ops.sql`).
//!
//! Declarations for the last stage-5 slice; the modules mount as they are
//! ported (stage 5). What lives here:
//!
//! - `console`: the old server's admin console and API (SIWE-style nonce/
//!   verify over `ADMIN_ADDRESSES` + `SESSION_SECRET`, HMAC cookie, 12 h;
//!   `/admin/api/content/sync/{pause,resume,force}`, `/flush-cache`, the
//!   failed-deployments retry/clear, the job report from `jobs::Jobs::report`,
//!   the cache report, `/admin/api/audit`). Sources:
//!   catalyrst/crates/catalyrst-server/src/admin, bin/live.
//! - `rpc`: the method-filtered EVM relay (`/{network}` POST + WebSocket,
//!   the read-only method allowlist, the per-network upstreams, the 1-2 s
//!   memo, `/admin/rpc/*` behind `CATALYRST_RPC_ADMIN_TOKEN`). No table: the
//!   allowlist and upstream edits are process state, as they were. Sources:
//!   catalyrst/crates/catalyrst-rpc/src.
//! - `audit`: the ONE `ops.admin_audit` writer every operator mutation in
//!   the tree records into (the old credits and content audit tables folded
//!   into one relation; the `telemetry` domain value stays for the rows the
//!   retired sink console wrote).
//!
//! Switches: [`OPS_WRITES_ENABLED`] gates every authoritative write of the
//! slice (credits ledger/checkouts/authorizations, registry writes, the console's
//! mutating routes; default false, 503 with `Retry-After`, built like
//! `social::writes`). Secrets (`CATALYRST_RPC_ADMIN_TOKEN`, `SESSION_SECRET`)
//! arrive as `<NAME>` or `<NAME>_FILE` through `Config::resolve_secret_files`
//! and are never logged.
//!
//! Compatibility reference (`docs/compatibility.md`): the asset-bundle registry
//! write side runs as an external runtime (github.com/decentraland/abgen, the
//! release the SDK's `abgen-release.lock` pins), while
//! `explore::api::registry` keeps the cached upstream reader and
//! `explore::api::ab_cdn` serves the CDN objects as a bounded read-through
//! cache; the sites SSR, metabase and NATS are external runtimes. The
//! bvimposters zip cache lives in `explore::imposters` over
//! `explore::filecache` (keyed by tile + crc, its own bounded directory beside
//! the ONE store). Client telemetry goes to Segment from the sites and the
//! explorer; this node is not a sink.

pub mod admin;

pub mod generated_secrets;
pub mod rpc;

pub use crate::shared::writes::{OPS_WRITES_ENABLED, WRITES_DISABLED_MESSAGE};

pub use crate::shared::audit::AUDIT_DOMAINS;

#[cfg(test)]
mod tests {
    use crate::db::SCHEMA_SQL;

    /// The rendering carries the operator slice's two schemas and the
    /// tables the module declarations above name.
    #[test]
    fn the_schema_renders_the_ops_slice() {
        assert!(SCHEMA_SQL.contains("-- ════ schemas/60_ops.sql ════"));
        for schema in ["credits", "ops"] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE SCHEMA {schema};")),
                "{schema}"
            );
        }
        assert!(
            !SCHEMA_SQL.contains("CREATE SCHEMA telemetry;"),
            "the telemetry sink is retired (schemas/releases/telemetry-retired.sql)"
        );
        for table in [
            "credits.wallets",
            "credits.ledger",
            "credits.idempotency",
            "credits.checkouts",
            "credits.fulfillment_outbox",
            "credits.authorizations",
            "ops.admin_audit",
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE TABLE {table} (")),
                "{table}"
            );
        }
        for domain in super::AUDIT_DOMAINS {
            assert!(
                SCHEMA_SQL.contains(&format!("'{domain}'")),
                "{domain} missing from the admin_audit CHECK"
            );
        }
        assert!(
            !SCHEMA_SQL.contains("pg_trgm"),
            "the trigram index was dead under the OR'd search predicate"
        );
    }
}

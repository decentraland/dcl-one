//! Ops: the operator plane (schema `ops`, `schemas/60_ops.sql`).
//!
//! Declarations for the last stage-5 slice; the modules mount as they are
//! ported (stage 5). What lives here:
//!
//! - `telemetry`: the Sentry (`/api/{project}/envelope`, `/store`) and
//!   Segment (`/v1/{batch,import,track,identify,page,screen,group,alias}`)
//!   sinks writing `telemetry.events` through one bounded batch writer, the
//!   `/dash/*` console (issues, events, metrics, health, funnel, breakdown,
//!   session, story, experiments, flags, groups, areas) behind the admin
//!   bearer or its login cookie, the operator routes (`/dash/admin/{purge,
//!   ingest,quota,bulk-delete,export,audit,regroup,release}`) and the
//!   retention job (`telemetry-purge`). Sources:
//!   catalyrst/crates/catalyrst-telemetry/src.
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
//!   the tree records into (the old telemetry, credits and content audit
//!   tables folded into one relation).
//!
//! Switches: [`OPS_WRITES_ENABLED`] gates every authoritative write of the
//! slice (credits ledger/checkouts/authorizations, telemetry console writes, registry writes, the console's
//! mutating routes; default false, 503 with `Retry-After`, built like
//! `social::writes`); [`TELEMETRY_INGEST_ENABLED`] gates the sinks alone,
//! which are not user-authoritative state (default false; with it on, the
//! operator's `telemetry.ingest_settings.enabled = false` makes the sinks
//! answer success and store nothing, the old runtime kill switch).
//! Secrets (`CATALYRST_TELEMETRY_ADMIN_TOKEN`, `CATALYRST_RPC_ADMIN_TOKEN`,
//! `SESSION_SECRET`) arrive as `<NAME>` or `<NAME>_FILE` through
//! `Config::resolve_secret_files` and are never logged.
//!
//! Compatibility reference (`docs/compatibility.md`): the asset-bundle registry
//! write side runs as an external runtime (github.com/decentraland/abgen, the
//! release the SDK's `abgen-release.lock` pins), while
//! `explore::api::registry` keeps the cached upstream reader and
//! `explore::api::ab_cdn` serves the CDN objects as a bounded read-through
//! cache; `/dash/sql` runs only on the telemetry reader's own login and pool
//! (`ops::telemetry::sql`, `TELEMETRY_SQL_DATABASE_URL`); the sites SSR,
//! metabase and NATS are external runtimes. The bvimposters zip cache lives in
//! `explore::imposters` over `explore::filecache` (keyed by tile + crc, its
//! own bounded directory beside the ONE store).

pub mod admin;

pub mod generated_secrets;
pub mod rpc;
pub mod telemetry;

/// The telemetry sinks' own switch: an ingest sink is not user authority,
/// so a node in catch-up may accept telemetry before it accepts writes.
pub const TELEMETRY_INGEST_ENABLED: &str = "TELEMETRY_INGEST_ENABLED";

pub use crate::shared::writes::{OPS_WRITES_ENABLED, WRITES_DISABLED_MESSAGE};

pub const INGEST_DISABLED_MESSAGE: &str =
    "telemetry ingest is disabled on this node (TELEMETRY_INGEST_ENABLED=false)";

pub use crate::shared::audit::AUDIT_DOMAINS;

#[cfg(test)]
mod tests {
    use crate::db::SCHEMA_SQL;

    /// The rendering carries the operator slice's three schemas and the
    /// tables the module declarations above name.
    #[test]
    fn the_schema_renders_the_ops_slice() {
        assert!(SCHEMA_SQL.contains("-- ════ schemas/60_ops.sql ════"));
        for schema in ["telemetry", "credits", "ops"] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE SCHEMA {schema};")),
                "{schema}"
            );
        }
        for table in [
            "telemetry.events",
            "telemetry.issue_state",
            "telemetry.ingest_settings",
            "telemetry.project_quotas",
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

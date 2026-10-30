//! `POST /dash/sql`, the console's ad-hoc query: one `SELECT`/`WITH`
//! statement wrapped as `SELECT to_jsonb(r) FROM (<sql>) r LIMIT 1000`, run
//! in a `READ ONLY` transaction under a 15 s `statement_timeout` on a pool
//! of its own that logs in as the telemetry reader
//! (`TELEMETRY_SQL_DATABASE_URL`, or `_FILE`): a login role holding CONNECT
//! on the database, USAGE on schema `telemetry` and SELECT on its tables,
//! and nothing else (NOINHERIT, a member of no role, CREATE nowhere,
//! `statement_timeout` and `default_transaction_read_only` set on the role;
//! `deploy/nix/postgresql.nix` provisions it with a generated password).
//!
//! Why a login and not a role switch: the old telemetry DSN reached one
//! database that held telemetry alone, while the ONE runtime role reads
//! every schema (the credits ledger, social, the economy keys). A
//! `SET LOCAL ROLE <reader>` on the runtime pool was undone from inside a
//! single permitted `SELECT` (`set_config('role', 'none', true)`, or a
//! function that runs `RESET ROLE`), both of which return to the session
//! user. Here the session user *is* the reader, so there is no wider role
//! to return to: `SET ROLE` to anything else needs a membership it does not
//! have, `SET SESSION AUTHORIZATION` needs a superuser session, and a read
//! of another schema, `pg_authid` or a server file is refused by the
//! database. The route never touches the runtime pool: unset, it answers
//! 503 naming the variable.
//!
//! What the route itself adds, since a misprovisioned role would silently
//! widen the bearer's reach again: before the statement it checks, in the
//! same transaction, that the login holds nothing beyond that
//! ([`POSTURE_SQL`]: no superuser/INHERIT/CREATEROLE/CREATEDB/REPLICATION/
//! BYPASSRLS, no membership, USAGE on no schema but `telemetry` and the
//! system ones, CREATE on no schema or the database, no readable relation
//! in `public`, EXECUTE on neither `pg_notify` nor `pg_logical_emit_message`),
//! and answers 503 listing what it found otherwise (a DSN pointed at the
//! runtime role, say). Those two built-ins write from inside a `READ ONLY`
//! transaction: a forged `NOTIFY new_deployment` (the channel the content
//! trigger wakes an entity cache on) and WAL records; the schema oneshot
//! revokes both from PUBLIC in this database and gives `pg_notify` back to
//! the runtime role alone, whose deployment trigger notifies. Every connection runs `DISCARD ALL`
//! when it returns to the pool, so what a statement leaves on its session
//! (a session-level advisory lock, which survives the rollback) ends with
//! the request; within the request, such a lock or a `pg_sleep` lasts at
//! most the statement timeout, which `set_config` cannot disarm once the
//! statement runs. The catalog stays readable as it is to every role:
//! names of other schemas' tables and columns, function sources, the role
//! list with masked passwords.
//!
//! The sites web tier's two direct readers of `telemetry.events`
//! (`TELEMETRY_DATABASE_URL`: the creator hub's scene errors, the operator
//! metrics) read nothing else, so they use this same login;
//! `deploy/nix/postgresql.nix` writes its DSN for them too.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use super::{SharedState, db_err};

/// The variable (or `<ENV>_FILE`) naming the reader's DSN.
pub const ENV: &str = "TELEMETRY_SQL_DATABASE_URL";
pub const STATEMENT_TIMEOUT_MS: u32 = 15_000;
pub const MAX_ROWS: usize = 1_000;
/// The console is one operator's page; two connections keep a slow
/// statement from blocking a second tab without letting the bearer fan out.
pub const MAX_CONNECTIONS: u32 = 2;
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

pub const NOT_CONFIGURED: &str = "POST /dash/sql runs on its own login role and never on the runtime pool: set TELEMETRY_SQL_DATABASE_URL (or TELEMETRY_SQL_DATABASE_URL_FILE) to a role with USAGE + SELECT on schema telemetry and nothing else (deploy/nix/postgresql.nix)";

/// What the reader holds beyond read access to `telemetry`, as one text
/// array (empty: the posture the route requires). Runs as the reader, over
/// catalogs every role can read.
pub const POSTURE_SQL: &str = "\
SELECT array_remove(ARRAY[ \
         CASE WHEN r.rolsuper THEN 'SUPERUSER' END, \
         CASE WHEN r.rolinherit THEN 'INHERIT' END, \
         CASE WHEN r.rolcreaterole THEN 'CREATEROLE' END, \
         CASE WHEN r.rolcreatedb THEN 'CREATEDB' END, \
         CASE WHEN r.rolreplication THEN 'REPLICATION' END, \
         CASE WHEN r.rolbypassrls THEN 'BYPASSRLS' END, \
         CASE WHEN current_user <> session_user THEN 'a role switched from ' || session_user END, \
         CASE WHEN has_database_privilege(current_database(), 'CREATE') THEN 'CREATE on the database' END \
       ], NULL) \
       || ARRAY(SELECT 'member of ' || g.rolname FROM pg_catalog.pg_auth_members m JOIN pg_catalog.pg_roles g ON g.oid = m.roleid \
                 WHERE m.member = r.oid ORDER BY 1) \
       || ARRAY(SELECT 'USAGE on schema ' || n.nspname FROM pg_catalog.pg_namespace n \
                 WHERE n.nspname NOT IN ('telemetry', 'pg_catalog', 'information_schema', 'public') AND n.nspname !~ '^pg_' \
                   AND has_schema_privilege(n.oid, 'USAGE') ORDER BY 1) \
       || ARRAY(SELECT 'CREATE on schema ' || n.nspname FROM pg_catalog.pg_namespace n \
                 WHERE n.nspname !~ '^pg_(temp|toast)' AND has_schema_privilege(n.oid, 'CREATE') ORDER BY 1) \
       || ARRAY(SELECT 'SELECT on public.' || c.relname FROM pg_catalog.pg_class c \
                 WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'v', 'm', 'p', 'f') \
                   AND has_table_privilege(c.oid, 'SELECT') ORDER BY 1) \
       || ARRAY(SELECT 'EXECUTE on ' || p.oid::regprocedure::text FROM pg_catalog.pg_proc p \
                 WHERE p.pronamespace = 'pg_catalog'::regnamespace AND p.proname IN ('pg_notify', 'pg_logical_emit_message') \
                   AND has_function_privilege(p.oid, 'EXECUTE') ORDER BY 1) \
  FROM pg_catalog.pg_roles r \
 WHERE r.rolname = current_user";

/// The reader's connection options. The DSN carries the role's password,
/// so neither it nor anything parsed from it is ever printed.
#[derive(Clone)]
pub struct ReaderDsn(PgConnectOptions);

impl std::fmt::Debug for ReaderDsn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReaderDsn(<redacted>)")
    }
}

impl ReaderDsn {
    /// A refusal names the variable, never the value.
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        raw.trim()
            .parse::<PgConnectOptions>()
            .map(Self)
            .map_err(|_| anyhow::anyhow!("{ENV} is not a postgres:// connection string"))
    }

    /// The reader's own pool, opened lazily (a login the database refuses
    /// is the route's 503, not a failed boot). The session options repeat
    /// the role's settings, so a role provisioned without them is still
    /// read-only, bounded and resolves the console's unqualified names in
    /// `telemetry`; the statement cache is off because `DISCARD ALL`, run
    /// on every release, deallocates what it would hold.
    pub fn pool(&self) -> PgPool {
        let statement_timeout = STATEMENT_TIMEOUT_MS.to_string();
        let options = self
            .0
            .clone()
            .application_name("catalyst-telemetry-sql")
            .statement_cache_capacity(0)
            .options([
                ("search_path", "telemetry,pg_catalog"),
                ("TimeZone", "UTC"),
                ("statement_timeout", statement_timeout.as_str()),
                ("default_transaction_read_only", "on"),
                ("idle_in_transaction_session_timeout", "30000"),
                ("client_connection_check_interval", "10000"),
            ]);
        PgPoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .min_connections(0)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .idle_timeout(IDLE_TIMEOUT)
            .after_release(|conn, _| {
                Box::pin(
                    async move { Ok(sqlx::raw_sql("DISCARD ALL").execute(conn).await.is_ok()) },
                )
            })
            .connect_lazy_with(options)
    }
}

#[derive(Deserialize)]
pub struct SqlBody {
    sql: String,
}

pub async fn sql_query(
    State(st): State<SharedState>,
    Json(b): Json<SqlBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let raw = b.sql.trim().trim_end_matches(';').trim();
    let low = raw.to_lowercase();
    if !(low.starts_with("select") || low.starts_with("with")) {
        return Err((
            StatusCode::BAD_REQUEST,
            "only SELECT / WITH queries are allowed".into(),
        ));
    }
    if raw.contains(';') {
        return Err((
            StatusCode::BAD_REQUEST,
            "one statement only (no ';')".into(),
        ));
    }
    let Some(pool) = st.sql_pool.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, NOT_CONFIGURED.into()));
    };

    let begin = format!(
        "BEGIN READ ONLY; SET LOCAL statement_timeout = {STATEMENT_TIMEOUT_MS}; SET LOCAL search_path = telemetry, pg_catalog"
    );
    let mut tx = pool
        .begin_with(sqlx::AssertSqlSafe(begin))
        .await
        .map_err(unreachable)?;
    let excess: Vec<String> = match sqlx::query_scalar(POSTURE_SQL).fetch_one(&mut *tx).await {
        Ok(excess) => excess,
        Err(e) => {
            let _ = tx.rollback().await;
            return Err(db_err("telemetry /dash/sql posture", e));
        }
    };
    if !excess.is_empty() {
        let _ = tx.rollback().await;
        tracing::error!(
            ?excess,
            "telemetry /dash/sql: the {ENV} login holds more than read access to schema telemetry; refusing to run statements on it"
        );
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "POST /dash/sql refuses to run: the role {ENV} logs in as holds more than read access to schema telemetry ({}); provision it as deploy/nix/postgresql.nix does",
                excess.join(", ")
            ),
        ));
    }

    // The row alias is a name no console statement uses: `to_jsonb(<alias>)`
    // resolves to a column of that name before the whole row (the old
    // wrapper's `t` turned a statement with a column `t` into that column).
    let wrapped = format!(
        "SELECT to_jsonb(dash_sql_row) AS row FROM ( {raw} ) dash_sql_row LIMIT {MAX_ROWS}"
    );
    let run = sqlx::query_scalar::<_, Value>(sqlx::AssertSqlSafe(wrapped))
        .fetch_all(&mut *tx)
        .await;
    let _ = tx.rollback().await;
    let rows = run.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let truncated = rows.len() >= MAX_ROWS;

    let cols: Vec<String> = rows
        .first()
        .and_then(|r| r.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    Ok(Json(
        json!({ "columns": cols, "rows": rows, "truncated": truncated }),
    ))
}

/// The reader's pool could not open a transaction: a refused login (wrong
/// password, the role or database missing), the connection limit or the
/// database down. Named and answered 503; the log line carries the
/// database's reason, which never includes the password.
fn unreachable(e: sqlx::Error) -> (StatusCode, String) {
    tracing::error!(error = %e, "telemetry /dash/sql: the {ENV} login could not open a transaction");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!(
            "POST /dash/sql could not connect as the role {ENV} names (refused login, connection limit or database down); see the node log"
        ),
    )
}

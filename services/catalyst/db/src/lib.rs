pub mod schema_migrate;

use std::collections::HashMap;
use std::sync::Mutex;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use thiserror::Error;

pub const SCHEMA_IDENTITY: &str = "catalyst";
/// 2 (2026-09-26): schema `marketplace` is `shop` and the four checkout ledgers
/// left `economy` (DATABASE.md, "Bootstrap, schema versions and local writes"); 1: the closure layout.
pub const SCHEMA_VERSION: i32 = 2;
/// `schema/*.sql` rendered in name order by build.rs; `catalyst schema print`
/// writes the same text.
pub const SCHEMA_SQL: &str = include_str!(concat!(env!("OUT_DIR"), "/schema.sql"));
/// `schemas/migrations/*.sql` rendered in name order by build.rs: the additive
/// steps `catalyst schema migrate` applies to a database stamped before them
/// (`schema_migrate`); `catalyst schema print --migrations` writes the same text.
pub const MIGRATIONS_SQL: &str = include_str!(concat!(env!("OUT_DIR"), "/migrations.sql"));

/// Every connection runs with this path and clock; application SQL is
/// schema-qualified anyway, and no request ever changes either.
const SESSION_OPTIONS: &[(&str, &str)] = &[("search_path", "content,meta"), ("TimeZone", "UTC")];

#[derive(Debug, Clone)]
pub struct PoolSettings {
    pub max_connections: u32,
    pub idle_timeout_secs: u64,
    pub acquire_timeout_secs: Option<u64>,
    pub statement_timeout_ms: u32,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self {
            max_connections: 20,
            idle_timeout_secs: 60,
            acquire_timeout_secs: Some(10),
            statement_timeout_ms: 60_000,
        }
    }
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("invalid postgres connection string: {0}")]
    InvalidUrl(sqlx::Error),

    #[error(transparent)]
    Connect(sqlx::Error),
}

/// The schema identity `bootstrap` verified, per pool (keyed by
/// host:port/database). Every connection the pool opens afterwards checks
/// `meta.schema` against it: a database dropped and recreated underneath a
/// running node (seen on the shared lab cluster) otherwise reconnects
/// silently and serves the empty replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SchemaStamp {
    identity: String,
    version: i32,
    applied_at: chrono::NaiveDateTime,
}

static VERIFIED: Mutex<Option<HashMap<String, SchemaStamp>>> = Mutex::new(None);

fn pool_key(opts: &PgConnectOptions) -> String {
    format!(
        "{}:{}/{}",
        opts.get_host(),
        opts.get_port(),
        opts.get_database().unwrap_or("")
    )
}

fn verified_stamp(key: &str) -> Option<SchemaStamp> {
    VERIFIED
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(key).cloned())
}

fn remember_stamp(key: String, stamp: SchemaStamp) {
    VERIFIED
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(key, stamp);
}

async fn read_stamp<'e, E>(executor: E) -> sqlx::Result<Option<SchemaStamp>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row: Option<(String, i32, chrono::NaiveDateTime)> =
        sqlx::query_as("SELECT identity, version, applied_at FROM meta.schema WHERE id = 1")
            .fetch_optional(executor)
            .await?;
    Ok(row.map(|(identity, version, applied_at)| SchemaStamp {
        identity,
        version,
        applied_at,
    }))
}

/// The shared pool: per-session GUCs are set once at connect time (no
/// per-request SET LOCAL round trip), and once `bootstrap` has verified the
/// schema, every new connection re-reads `meta.schema` and is refused (the
/// acquire fails, loudly) when the identity, version or `applied_at` moved.
pub async fn connect_pool(url: &str, settings: &PoolSettings) -> Result<PgPool, PoolError> {
    let statement_timeout = settings.statement_timeout_ms.to_string();
    let mut options: Vec<(&str, &str)> = vec![
        ("statement_timeout", statement_timeout.as_str()),
        ("idle_in_transaction_session_timeout", "30000"),
        // A backend whose client vanished (kill -9 under a long statement)
        // notices within 10 s instead of holding its locks until the
        // statement ends: the 2026-09-25 market live run saw the killed
        // node's land_authz fold block the restarted node's fold for 60 s.
        ("client_connection_check_interval", "10000"),
    ];
    options.extend(SESSION_OPTIONS.iter().copied());
    let connect_opts: PgConnectOptions = url
        .parse::<PgConnectOptions>()
        .map_err(PoolError::InvalidUrl)?
        .options(options);

    let mut pool_opts = PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .idle_timeout(std::time::Duration::from_secs(settings.idle_timeout_secs));
    if let Some(secs) = settings.acquire_timeout_secs {
        pool_opts = pool_opts.acquire_timeout(std::time::Duration::from_secs(secs));
    }
    let key = pool_key(&connect_opts);
    pool_opts = pool_opts.after_connect(move |conn, _meta| {
        let key = key.clone();
        Box::pin(async move {
            let Some(expected) = verified_stamp(&key) else {
                return Ok(());
            };
            let found = read_stamp(&mut *conn).await;
            match found {
                Ok(Some(stamp)) if stamp == expected => Ok(()),
                other => {
                    tracing::error!(
                        database = %key,
                        expected = ?expected,
                        found = ?other,
                        "meta.schema changed under the running node: the database was replaced or reset; refusing the connection"
                    );
                    Err(sqlx::Error::Configuration(
                        "meta.schema no longer matches the schema verified at startup (database replaced?)".into(),
                    ))
                }
            }
        })
    });

    pool_opts
        .connect_with(connect_opts)
        .await
        .map_err(PoolError::Connect)
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error(
        "meta.schema is missing: this database has no catalyst schema. Apply it once with \
         `catalyst schema apply` (runs schema.sql); startup never creates or resets data"
    )]
    SchemaMissing,

    #[error(
        "schema mismatch: database has identity {identity:?} version {version}, \
         this binary needs {SCHEMA_IDENTITY:?} version {SCHEMA_VERSION}"
    )]
    Mismatch { identity: String, version: i32 },

    #[error(
        "the database lacks part of this binary's schema: {0}. It was stamped by an earlier \
         binary; run `catalyst schema migrate` as the administrative role (the schema oneshot \
         does on every activation) to create them"
    )]
    Layout(String),

    #[error("reading the schema layout: {0}")]
    LayoutCheck(#[from] crate::schema_migrate::LayoutError),

    #[error("reading meta.schema: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// Verifies the schema identity/version before anything else touches the
/// database, and remembers the stamp so the pool refuses a connection to a
/// replaced database from then on. Read-only by design.
pub async fn bootstrap(pool: &PgPool) -> Result<(), BootstrapError> {
    let stamp = read_stamp(pool).await.map_err(|e| match &e {
        sqlx::Error::Database(db) if matches!(db.code().as_deref(), Some("42P01" | "3F000")) => {
            BootstrapError::SchemaMissing
        }
        _ => BootstrapError::Sqlx(e),
    })?;
    match stamp {
        Some(stamp) if stamp.identity == SCHEMA_IDENTITY && stamp.version == SCHEMA_VERSION => {
            tracing::info!(
                identity = %stamp.identity,
                version = stamp.version,
                applied_at = %stamp.applied_at,
                "schema verified"
            );
            let gaps = crate::schema_migrate::gaps(pool, SCHEMA_SQL).await?;
            let fatal = gaps.fatal();
            if !fatal.is_empty() {
                return Err(BootstrapError::Layout(fatal.join(", ")));
            }
            let indexes = gaps.indexes();
            if !indexes.is_empty() {
                tracing::warn!(missing = %indexes.join(", "), "indexes of this binary's schema are missing; `catalyst schema migrate` creates them");
            }
            // content lists sort their files under upstream's collation
            // (content queries.rs), a glibc locale the
            // cluster must know whatever the database's own collation is
            let collation: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_collation WHERE collname = 'en_US.utf8')",
            )
            .fetch_one(pool)
            .await?;
            if !collation {
                return Err(BootstrapError::Layout(
                    "collation en_US.utf8 is not defined on this cluster".into(),
                ));
            }
            remember_stamp(pool_key(&pool.connect_options()), stamp);
            Ok(())
        }
        Some(stamp) => Err(BootstrapError::Mismatch {
            identity: stamp.identity,
            version: stamp.version,
        }),
        None => Err(BootstrapError::SchemaMissing),
    }
}

#[derive(Debug, Error)]
pub enum ApplySchemaError {
    #[error("refusing to apply schema.sql: meta.schema already exists in this database")]
    AlreadyApplied,

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

/// Administrative only (`catalyst schema apply`): runs the rendered schema on
/// an empty database and refuses when a schema is already there. The steps
/// of `schemas/migrations/` are recorded as carried from birth (schema.sql
/// already holds their effects), so `schema migrate` never re-runs them.
pub async fn apply_schema(pool: &PgPool) -> Result<(), ApplySchemaError> {
    // One apply at a time per process: the DDL holds a lock per object it
    // creates until commit, and parallel applies (the test suites create a
    // database per module) exhaust the cluster's shared lock table
    // (max_locks_per_transaction * max_connections: "out of shared memory").
    static APPLYING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one_at_a_time = APPLYING.lock().await;
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('meta.schema') IS NOT NULL")
        .fetch_one(pool)
        .await?;
    if exists {
        return Err(ApplySchemaError::AlreadyApplied);
    }
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(SCHEMA_SQL).execute(&mut *tx).await?;
    let steps = crate::schema_migrate::parse_steps(MIGRATIONS_SQL);
    crate::schema_migrate::record_steps(&mut *tx, &steps, true).await?;
    tx.commit().await?;
    tracing::info!(
        identity = SCHEMA_IDENTITY,
        version = SCHEMA_VERSION,
        steps = steps.len(),
        "schema applied"
    );
    Ok(())
}

/// The lib crate's database-backed tests share one throwaway database per
/// test process, `catalyst_test_lib_<pid>_<rand>`, created on the cluster
/// `CATALYST_TEST_DATABASE_URL` names (the database in that URL is never
/// touched, so a live node's rows are out of reach) and seeded with
/// schema.sql; the tests seed uuid-scoped rows inside it. There is no
/// in-process teardown: the next test process drops every
/// `catalyst_test_lib_<pid>_*` whose pid is gone.
#[cfg(any(test, feature = "scratch"))]
pub mod scratch {
    use super::*;
    use tokio::sync::OnceCell;

    static URL: OnceCell<Option<String>> = OnceCell::const_new();

    /// The scratch database's URL; `None` (skip) without `CATALYST_TEST_DATABASE_URL`.
    pub async fn url() -> Option<String> {
        URL.get_or_init(create).await.clone()
    }

    /// A pool over the scratch database with the process's session options.
    pub async fn pool(max_connections: u32) -> Option<PgPool> {
        let url = url().await?;
        Some(
            connect_pool(
                &url,
                &PoolSettings {
                    max_connections,
                    ..Default::default()
                },
            )
            .await
            .expect("connecting to the scratch test database"),
        )
    }

    async fn create() -> Option<String> {
        let base = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let opts: PgConnectOptions = base
            .parse()
            .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap_or_else(|e| {
                panic!("connecting to the postgres database of the test cluster: {e}")
            });
        sweep_dead(&admin).await;
        let name = format!(
            "catalyst_test_lib_{}_{}",
            std::process::id(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap_or_else(|e| panic!("CREATE DATABASE {name}: {e}"));
        let mut url = url::Url::parse(&base).expect("CATALYST_TEST_DATABASE_URL must be a URL");
        url.set_path(&format!("/{name}"));
        let url = url.to_string();
        let pool = connect_pool(
            &url,
            &PoolSettings {
                max_connections: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|e| panic!("connecting to {name}: {e}"));
        apply_schema(&pool)
            .await
            .unwrap_or_else(|e| panic!("applying schema.sql to {name}: {e}"));
        pool.close().await;
        eprintln!("lib tests: scratch database {name}");
        Some(url)
    }

    /// Drops the scratch databases of test processes that no longer exist.
    async fn sweep_dead(admin: &PgPool) {
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT datname FROM pg_database WHERE datname LIKE 'catalyst_test_lib_%'",
        )
        .fetch_all(admin)
        .await
        .unwrap_or_default();
        for name in names {
            let pid = name
                .trim_start_matches("catalyst_test_lib_")
                .split('_')
                .next()
                .and_then(|p| p.parse::<u32>().ok());
            let alive = pid.is_some_and(|pid| {
                pid == std::process::id() || std::path::Path::new(&format!("/proc/{pid}")).exists()
            });
            if alive {
                continue;
            }
            let dropped = sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE \"{name}\" WITH (FORCE)"
            )))
            .execute(admin)
            .await;
            match dropped {
                Ok(_) => eprintln!("lib tests: dropped the stale scratch database {name}"),
                Err(e) => eprintln!("lib tests: could not drop {name}: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Once bootstrap verified a database, any connection opened to it
    /// afterwards (a reconnect after the backends were killed, here a second
    /// pool on the same URL) re-reads meta.schema and is refused when the
    /// stamp moved: the shape of a database dropped and recreated underneath
    /// a running node.
    #[tokio::test]
    async fn a_replaced_schema_stamp_refuses_new_connections() {
        let Some(url) = scratch::url().await else {
            return;
        };
        // A database of its own: the stamp is per host:port/database and the
        // shared scratch database must keep working for the other tests.
        let opts: PgConnectOptions = url.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("catalyst_test_lib_{}_stamp", std::process::id());
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await;
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut own = url::Url::parse(&url).unwrap();
        own.set_path(&format!("/{name}"));
        let settings = PoolSettings {
            max_connections: 2,
            ..Default::default()
        };
        let pool = connect_pool(own.as_str(), &settings).await.unwrap();
        apply_schema(&pool).await.unwrap();
        bootstrap(&pool).await.unwrap();
        let again = connect_pool(own.as_str(), &settings)
            .await
            .expect("the same stamp connects");
        again.close().await;

        sqlx::query(
            "UPDATE meta.schema SET applied_at = applied_at + interval '1 second' WHERE id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();
        // sqlx retries a failed after_connect until the acquire timeout, so
        // the caller sees the timeout and the log the refusal.
        let quick = PoolSettings {
            acquire_timeout_secs: Some(2),
            ..settings
        };
        let err = connect_pool(own.as_str(), &quick)
            .await
            .expect_err("a fresh connection must be refused");
        let text = err.to_string();
        assert!(
            text.contains("no longer matches") || text.contains("timed out"),
            "{text}"
        );

        pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await;
    }

    #[test]
    fn schema_sql_stamps_the_version_this_binary_expects() {
        assert!(SCHEMA_SQL.contains("CREATE SCHEMA content"));
        assert!(SCHEMA_SQL.contains("CREATE SCHEMA meta"));
        assert!(
            SCHEMA_SQL.contains("SET timezone = "),
            "every session reads UTC timestamps"
        );
        let stamp = format!("(1, '{SCHEMA_IDENTITY}', {SCHEMA_VERSION},");
        assert!(
            SCHEMA_SQL.contains(&stamp),
            "schema.sql must insert {stamp}"
        );
    }

    #[test]
    fn schema_sql_is_the_rendering_of_every_domain_file() {
        for domain in [
            "schemas/00_content.sql",
            "schemas/10_explore.sql",
            "schemas/20_worlds.sql",
            "schemas/30_shop.sql",
            "schemas/40_social.sql",
            "schemas/50_create.sql",
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("-- ════ {domain} ════")),
                "{domain} missing from the rendering"
            );
        }
    }

    /// The four copied indexes the 2026-09-26 review found without a
    /// statement stay dropped from the rendered schema
    /// (`broker_purchases_escrow_address_idx`,
    /// `escrow_actions_escrow_address_idx`, `name_transfers_token_id_idx`,
    /// `usage_grants_urn_idx`).
    #[test]
    fn copied_indexes_without_a_statement_stay_dropped() {
        for dropped in [
            "broker_purchases_escrow_address_idx",
            "escrow_actions_escrow_address_idx",
            "name_transfers_token_id_idx",
            "usage_grants_urn_idx",
        ] {
            assert!(
                !SCHEMA_SQL.contains(dropped),
                "{dropped} serves no statement (review 2026-09-26)"
            );
        }
    }

    /// The deploy validator's per-item probe (`content::validator::chain`,
    /// `granted`) is served by the grantee's partial index; its `urn = ANY
    /// OR urn LIKE ANY` clause is a filter no btree on `urn` answers, which
    /// is why `usage_grants_urn_idx` is not carried.
    #[tokio::test]
    async fn the_usage_grant_probe_plans_on_the_grantee_index() {
        let Some(pool) = scratch::pool(1).await else {
            return;
        };
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL enable_seqscan = off")
            .execute(&mut *tx)
            .await
            .unwrap();
        let plan: Vec<(String,)> = sqlx::query_as(
            "EXPLAIN SELECT urn, token_id FROM shop.usage_grants \
             WHERE grantee_address = lower($1) AND status = 'active' \
               AND (urn = ANY($2) OR urn LIKE ANY($3))",
        )
        .bind("0xAB")
        .bind(vec!["urn:a".to_string()])
        .bind(vec!["urn:b:%".to_string()])
        .fetch_all(&mut *tx)
        .await
        .unwrap();
        let plan: Vec<String> = plan.into_iter().map(|(l,)| l).collect();
        assert!(
            plan.iter()
                .any(|l| l.contains("Index Scan using usage_grants_grantee_idx")),
            "{plan:#?}"
        );
        assert!(
            plan.iter()
                .any(|l| l.contains("Filter:") && l.contains("urn")),
            "the urn clause is a filter on the grantee's rows: {plan:#?}"
        );
        tx.rollback().await.unwrap();
    }

    /// A database `schema apply` created carries every step of this binary
    /// from birth: `schema migrate` on it applies nothing, and `status` says
    /// up to date.
    #[tokio::test]
    async fn a_fresh_schema_records_every_step_as_carried_from_birth() {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        // This assertion covers the entire migration ledger. Other tests add
        // synthetic steps to the shared scratch database, so UUID-scoped rows
        // there cannot isolate a freshly applied schema.
        let opts: PgConnectOptions = base.parse().unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!("dcl_fresh_schema_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut own = url::Url::parse(&base).unwrap();
        own.set_path(&format!("/{name}"));
        let pool = connect_pool(
            own.as_str(),
            &PoolSettings {
                max_connections: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        apply_schema(&pool).await.unwrap();
        let steps = crate::schema_migrate::parse_steps(MIGRATIONS_SQL);
        let recorded: Vec<(String, String, bool)> =
            sqlx::query_as("SELECT step, sha256, created_fresh FROM meta.schema_steps WHERE step ~ '^[0-9]{4}_' ORDER BY step")
                .fetch_all(&pool)
                .await
                .unwrap();
        let want: Vec<(String, String, bool)> = steps
            .iter()
            .map(|s| (s.name.clone(), s.sha256.clone(), true))
            .collect();
        assert_eq!(recorded, want);
        let st = crate::schema_migrate::status(&pool, SCHEMA_SQL, MIGRATIONS_SQL)
            .await
            .unwrap();
        assert_eq!(st.exit_code(), 0, "{:?}", st.lines());
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }

    #[test]
    fn session_options_pin_path_and_clock() {
        assert!(SESSION_OPTIONS.contains(&("search_path", "content,meta")));
        assert!(SESSION_OPTIONS.contains(&("TimeZone", "UTC")));
    }
}

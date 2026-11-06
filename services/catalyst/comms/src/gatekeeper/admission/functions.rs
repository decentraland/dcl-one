//! Admission SQL functions. Names bind the binary to its SQL body; the schema
//! object pass installs new versions without replacing functions old binaries use.

use sqlx::PgPool;

const ADMISSION_DATABASE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(1);

struct SqlFunction {
    prefix: &'static str,
    args: &'static str,
    returns: &'static str,
    body: &'static str,
}

const STAMP: SqlFunction = SqlFunction {
    prefix: "world_authorization_started",
    args: "text",
    returns: "timestamp without time zone",
    body: "BEGIN
    PERFORM 1 FROM worlds.worlds WHERE name = $1 FOR SHARE;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    RETURN timezone('UTC', clock_timestamp());
END",
};

const REFUND: SqlFunction = SqlFunction {
    prefix: "world_secret_refund",
    args: "text, text, timestamp without time zone",
    returns: "void",
    body: "DECLARE
    charged integer;
BEGIN
    SELECT attempts INTO charged FROM social.world_secret_attempts
    WHERE world_name = $1 AND subject = $2 AND window_start = $3 FOR UPDATE;
    IF charged = 1 THEN
        DELETE FROM social.world_secret_attempts
        WHERE world_name = $1 AND subject = $2 AND window_start = $3;
    ELSIF charged > 1 THEN
        UPDATE social.world_secret_attempts SET attempts = attempts - 1
        WHERE world_name = $1 AND subject = $2 AND window_start = $3;
    END IF;
END",
};

const ALL: [&SqlFunction; 2] = [&STAMP, &REFUND];
// Function-local settings restore the caller's values, including explicit transactions.
const SETTINGS: &str = "SET lock_timeout = '500ms'";

impl SqlFunction {
    fn name(&self) -> String {
        use sha2::Digest;
        let digest = sha2::Sha256::digest(format!(
            "{}|{}|{}|{}",
            self.args, self.returns, SETTINGS, self.body
        ));
        format!("{}_{}", self.prefix, &hex::encode(digest)[..12])
    }

    fn signature(&self) -> String {
        format!("social.{}({})", self.name(), self.args)
    }

    fn sql(&self, replace: bool) -> String {
        format!(
            "CREATE {}FUNCTION social.{}({}) RETURNS {} LANGUAGE plpgsql {} AS $admission$ {} $admission$;\n",
            if replace { "OR REPLACE " } else { "" },
            self.name(),
            self.args,
            self.returns,
            SETTINGS,
            self.body
        )
    }
}

/// Prints the same objects that `catalyst schema migrate` installs.
pub fn social_functions_sql(replace: bool) -> String {
    ALL.map(|function| function.sql(replace)).join("\n")
}

/// Bound the caller's wait, including pool acquisition. Dropping the future
/// does not assert that a dispatched server statement was cancelled.
pub(crate) async fn bounded<T>(
    operation: impl std::future::Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, sqlx::Error> {
    tokio::time::timeout(ADMISSION_DATABASE_DEADLINE, operation)
        .await
        .map_err(|_| {
            sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "world admission database operation timed out",
            ))
        })?
}

pub(crate) struct Installed {
    pub stamp: String,
    pub refund: String,
}

static INSTALLED: tokio::sync::OnceCell<Installed> = tokio::sync::OnceCell::const_new();

pub(crate) async fn installed(pool: &PgPool) -> Result<&'static Installed, sqlx::Error> {
    INSTALLED.get_or_try_init(|| Installed::load(pool)).await
}

impl Installed {
    async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let signatures: Vec<String> = ALL.iter().map(|function| function.signature()).collect();
        let missing = crate::db::missing_functions(pool, &signatures).await?;
        if !missing.is_empty() {
            return Err(sqlx::Error::Configuration(format!(
                "admission SQL functions missing: {}; run `catalyst schema migrate` as the administrative role",
                missing.join(", ")
            ).into()));
        }
        Ok(Self {
            stamp: STAMP.name(),
            refund: REFUND.name(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_carries_the_admission_functions() {
        assert!(crate::db::SCHEMA_SQL.contains(&social_functions_sql(false)));
    }
    #[tokio::test]
    async fn installation_checks_preserve_query_errors() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();
        pool.close().await;
        assert!(matches!(
            Installed::load(&pool).await,
            Err(sqlx::Error::PoolClosed)
        ));
    }

    #[tokio::test]
    async fn old_non_key_writers_block_stamps_and_lock_timeout_is_restored() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let world = format!("legacy-writer-{}.dcl.eth", uuid::Uuid::new_v4());
        sqlx::query("INSERT INTO worlds.worlds(name) VALUES($1)")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let functions = installed(&pool).await.unwrap();
        let statement = format!("SELECT social.{}($1)", functions.stamp);
        let mut writer = pool.begin().await.unwrap();
        // Non-key update is the lock taken by access writers before this PR.
        sqlx::query("UPDATE worlds.worlds SET access_type='shared-secret',access_secret_hash='changed' WHERE name=$1")
            .bind(&world)
            .execute(&mut *writer)
            .await
            .unwrap();
        let mut caller = pool.acquire().await.unwrap();
        let before: String = sqlx::query_scalar("SHOW lock_timeout")
            .fetch_one(&mut *caller)
            .await
            .unwrap();
        let error = sqlx::query_scalar::<_, Option<chrono::NaiveDateTime>>(sqlx::AssertSqlSafe(
            statement.clone(),
        ))
        .bind(&world)
        .fetch_one(&mut *caller)
        .await
        .unwrap_err();
        assert_eq!(
            error.as_database_error().and_then(|e| e.code()).as_deref(),
            Some("55P03")
        );
        let restored: String = sqlx::query_scalar("SHOW lock_timeout")
            .fetch_one(&mut *caller)
            .await
            .unwrap();
        assert_eq!(restored, before);
        let cutoff: chrono::NaiveDateTime =
            sqlx::query_scalar("SELECT timezone('UTC',clock_timestamp())")
                .fetch_one(&mut *writer)
                .await
                .unwrap();
        writer.commit().await.unwrap();
        let stamped: Option<chrono::NaiveDateTime> =
            sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .bind(&world)
                .fetch_one(&mut *caller)
                .await
                .unwrap();
        assert!(stamped.unwrap() >= cutoff);
        let restored: String = sqlx::query_scalar("SHOW lock_timeout")
            .fetch_one(&mut *caller)
            .await
            .unwrap();
        assert_eq!(restored, before);
    }

    #[tokio::test]
    async fn the_schema_object_pass_installs_functions_without_changing_existing_rows() {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        // Only this UUID-named database loses functions. The configured database
        // and the shared scratch database remain untouched by the upgrade fixture.
        let options: sqlx::postgres::PgConnectOptions = base.parse().unwrap();
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_test_lib_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        use futures::FutureExt;
        let outcome = std::panic::AssertUnwindSafe(async {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(2)
                .connect_with(options.database(&name))
                .await
                .unwrap();
            crate::db::apply_schema(&pool).await.unwrap();
            let signatures: Vec<String> = ALL.iter().map(|function| function.signature()).collect();
            for signature in &signatures {
                sqlx::query(sqlx::AssertSqlSafe(format!("DROP FUNCTION {signature}")))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            assert_eq!(
                crate::db::missing_functions(&pool, &signatures)
                    .await
                    .unwrap(),
                signatures
            );
            let error = match Installed::load(&pool).await {
                Err(error) => error.to_string(),
                Ok(_) => panic!("missing production functions must refuse installation"),
            };
            for signature in &signatures {
                assert!(error.contains(signature), "{error}");
            }
            assert!(
                error.contains("catalyst schema migrate") && error.contains("administrative role"),
                "{error}"
            );
            let world = format!("upgrade-{}.dcl.eth", uuid::Uuid::new_v4());
            sqlx::query("INSERT INTO social.world_secret_attempts(world_name,subject,attempts,window_start) VALUES($1,'caller',2,timezone('UTC',now()))")
                .bind(&world).execute(&pool).await.unwrap();
            let before: Vec<(i32, chrono::NaiveDateTime)> = sqlx::query_as(
                "SELECT attempts,window_start FROM social.world_secret_attempts WHERE world_name=$1",
            )
            .bind(&world)
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(before.len(), 1);
            let done = crate::db::schema_migrate::migrate(
                &pool,
                crate::db::SCHEMA_SQL,
                crate::db::MIGRATIONS_SQL,
            )
            .await
            .unwrap();
            assert_eq!(done.created.len(), 2);
            assert!(
                crate::db::schema_migrate::migrate(
                    &pool,
                    crate::db::SCHEMA_SQL,
                    crate::db::MIGRATIONS_SQL
                )
                .await
                .unwrap()
                .created
                .is_empty()
            );
            assert!(
                crate::db::schema_migrate::gaps(&pool, crate::db::SCHEMA_SQL)
                    .await
                    .unwrap()
                    .fatal()
                    .is_empty()
            );
            let after: Vec<(i32, chrono::NaiveDateTime)> = sqlx::query_as(
                "SELECT attempts,window_start FROM social.world_secret_attempts WHERE world_name=$1",
            )
            .bind(&world)
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(after, before);
            assert!(
                crate::db::missing_functions(&pool, &signatures)
                    .await
                    .unwrap()
                    .is_empty()
            );
            pool.close().await;
        })
        .catch_unwind()
        .await;
        // Force only this private database's sessions closed, including a pool
        // dropped during a failed connection, schema setup or assertion.
        let cleanup = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{name}\" WITH (FORCE)"
        )))
        .execute(&admin)
        .await;
        admin.close().await;
        if let Err(panic) = outcome {
            if let Err(error) = &cleanup {
                eprintln!("cleaning up private upgrade database {name}: {error}");
            }
            std::panic::resume_unwind(panic);
        }
        cleanup.unwrap();
    }
}

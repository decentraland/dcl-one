//! Admission SQL functions. Names bind the binary to its SQL body; the schema
//! object pass installs new versions without replacing functions old binaries use.

use sqlx::PgPool;

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

pub(crate) struct Installed {
    pub stamp: String,
    pub refund: String,
}

static INSTALLED: tokio::sync::OnceCell<Installed> = tokio::sync::OnceCell::const_new();

pub(crate) async fn installed(pool: &PgPool) -> Result<&'static Installed, sqlx::Error> {
    INSTALLED.get_or_try_init(|| async {
        let signatures: Vec<String> = ALL.iter().map(|function| function.signature()).collect();
        let present: bool = sqlx::query_scalar("SELECT bool_and(to_regprocedure(signature) IS NOT NULL) FROM unnest($1::text[]) AS signature")
            .bind(signatures).fetch_one(pool).await?;
        if !present {
            return Err(sqlx::Error::Configuration("admission SQL functions missing; run catalyst schema migrate as the administrative role".into()));
        }
        Ok(Installed { stamp: STAMP.name(), refund: REFUND.name() })
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_carries_the_admission_functions() {
        assert!(crate::db::SCHEMA_SQL.contains(&social_functions_sql(false)));
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
        sqlx::query("UPDATE worlds.worlds SET access_secret_hash='changed' WHERE name=$1")
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
    async fn exported_functions_are_idempotent_on_existing_databases() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        // The operator's exported object SQL is idempotent and supports existing tables.
        for _ in 0..2 {
            sqlx::raw_sql(&social_functions_sql(true))
                .execute(&pool)
                .await
                .unwrap();
        }
        for function in ALL {
            let present: bool = sqlx::query_scalar("SELECT to_regprocedure($1) IS NOT NULL")
                .bind(function.signature())
                .fetch_one(&pool)
                .await
                .unwrap();
            assert!(present);
        }
    }
}

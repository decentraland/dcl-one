//! The explore schema's functions, each named by a hash of its definition as
//! the content write path's are (DATABASE.md): a stored generated column
//! keeps the values one definition computed, so a changed definition is a
//! new function and a new column, never a replaced one. `schemas/10_explore.sql`
//! carries [`functions_sql`] verbatim and a test pins it; the step
//! `0030_explore_read_columns.sql` is immutable once shipped (DATABASE.md), so
//! a test pins the three names it creates, and a changed definition means a
//! new step.

use std::sync::LazyLock;

use sha2::Digest;

use crate::places::query::RAW_TIMESTAMP_IS_CASTABLE_RE;

pub struct Function {
    /// `explore.<prefix>_<hash>`.
    pub name: String,
    /// The `CREATE FUNCTION` statement.
    pub sql: String,
}

/// `explore.<prefix>_<the first 12 hex digits of the SHA-256 of tail>`,
/// `tail` being everything after the name: the parameters, the attributes
/// and the body.
fn function(prefix: &str, tail: String) -> Function {
    let digest = hex::encode(sha2::Sha256::digest(&tail));
    let name = format!("explore.{prefix}_{}", &digest[..12]);
    Function {
        sql: format!("CREATE FUNCTION {name}{tail}"),
        name,
    }
}

/// An ISO 8601 date-time string as a UTC timestamp, NULL for anything else.
/// The guard admits the forms every DateStyle reads the same way, so the
/// function is immutable and a generated column may call it. A zone-less
/// string is read as UTC whatever the session's TimeZone: only a value with
/// a time field can carry a zone, so the zone test requires the minutes or
/// seconds digits before it and a date's `-DD` is never taken for an offset.
pub static UTC_TIMESTAMP: LazyLock<Function> = LazyLock::new(|| {
    let guard = format!("    RETURN CASE WHEN value ~ '{RAW_TIMESTAMP_IS_CASTABLE_RE}'");
    function(
        "utc_timestamp",
        [
            "(value text) RETURNS timestamp without time zone",
            "    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT",
            &guard,
            "        THEN CASE WHEN value ~ ':[0-9]{2}(\\.[0-9]{1,9})?(Z|z|[+-][0-9]{2}(:?[0-9]{2})?)$'",
            "                  THEN value::timestamptz AT TIME ZONE 'UTC'",
            "                  ELSE value::timestamp END",
            "        END;",
        ]
        .join("\n"),
    )
});

/// The recurrence instants of an event: each `raw.recurrent_dates` entry that
/// is an ISO 8601 date-time, ascending.
pub static RECURRENCE_DATES: LazyLock<Function> = LazyLock::new(|| {
    let parsed = format!(
        "    RETURN ARRAY(SELECT parsed.d FROM (SELECT {}(value) AS d",
        UTC_TIMESTAMP.name
    );
    function(
        "recurrence_dates",
        [
            "(raw jsonb) RETURNS timestamp without time zone[]",
            "    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT",
            &parsed,
            "                 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(raw->'recurrent_dates') = 'array'",
            "                                                     THEN raw->'recurrent_dates' ELSE '[]'::jsonb END)) parsed",
            "                 WHERE parsed.d IS NOT NULL ORDER BY parsed.d);",
        ]
        .join("\n"),
    )
});

/// The last instant an event is live under the lists' effective occurrence
/// (`events::EFF_NEXT_START_SQL` plus the duration), NULL when the row has no
/// instant: a live list reads an index range instead of expanding every
/// row's recurrence.
pub static EVENT_LIVE_UNTIL: LazyLock<Function> = LazyLock::new(|| {
    let dates = &RECURRENCE_DATES.name;
    let last = format!("        (SELECT max(d) FROM unnest({dates}(raw)) d)");
    let none = format!("        CASE WHEN next_start_at IS NULL AND cardinality({dates}(raw)) = 0");
    function(
        "event_live_until",
        [
            "(next_start_at timestamp without time zone, start_at timestamp without time zone, duration_ms bigint, raw jsonb)",
            "    RETURNS timestamp without time zone",
            "    LANGUAGE sql IMMUTABLE PARALLEL SAFE",
            "    RETURN GREATEST(",
            "        next_start_at + COALESCE(duration_ms, 0) * interval '1 millisecond',",
            &last,
            "            + COALESCE(duration_ms, 0) * interval '1 millisecond',",
            &none,
            "             THEN start_at + COALESCE(duration_ms, 0) * interval '1 millisecond' END);",
        ]
        .join("\n"),
    )
});

/// The statements as `schemas/10_explore.sql` carries them.
pub fn functions_sql() -> [&'static str; 3] {
    [
        &UTC_TIMESTAMP.sql,
        &RECURRENCE_DATES.sql,
        &EVENT_LIVE_UNTIL.sql,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = include_str!(concat!(env!("DCL_ONE_ROOT"), "/schemas/10_explore.sql"));
    const STEP: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0030_explore_read_columns.sql"
    ));

    /// The functions the shipped step created, by name. The step never
    /// changes, so neither does this list: a changed definition is a new
    /// name in a new step.
    const STEP_FUNCTIONS: [&str; 3] = [
        "explore.utc_timestamp_c37f040ae6c2",
        "explore.recurrence_dates_17e91951d5fe",
        "explore.event_live_until_8c77b09c74f5",
    ];

    /// `file` calls the functions by their hash-named names only and
    /// replaces none.
    fn calls_hashed_names_only(file: &str, text: &str) {
        for prefix in ["utc_timestamp", "recurrence_dates", "event_live_until"] {
            assert!(
                !text.contains(&format!("explore.{prefix}(")),
                "{file} calls {prefix} by its hash-named name"
            );
        }
        assert!(!text.contains("CREATE OR REPLACE FUNCTION"), "{file}");
    }

    /// The fresh schema carries each statement as this binary renders it.
    #[test]
    fn the_schema_carries_the_functions() {
        for sql in functions_sql() {
            assert!(
                SCHEMA.contains(sql),
                "schemas/10_explore.sql must carry this statement exactly:\n\n{sql}\n"
            );
        }
        calls_hashed_names_only("schemas/10_explore.sql", SCHEMA);
    }

    /// The shipped step creates the three functions it named, and where
    /// this binary still renders one of those names the definition is the
    /// one it hashes: nobody edits the step, and a changed definition goes
    /// in a new step under a new name.
    #[test]
    fn the_step_creates_the_functions_it_shipped() {
        const FILE: &str = "schemas/migrations/0030_explore_read_columns.sql";
        for name in STEP_FUNCTIONS {
            assert!(
                STEP.contains(&format!("CREATE FUNCTION {name}(")),
                "{FILE} creates {name}"
            );
        }
        for function in [&*UTC_TIMESTAMP, &*RECURRENCE_DATES, &*EVENT_LIVE_UNTIL] {
            if STEP_FUNCTIONS.contains(&function.name.as_str()) {
                assert!(
                    STEP.contains(&function.sql),
                    "{FILE} names {} with a definition other than this binary's:\n\n{}\n",
                    function.name,
                    function.sql
                );
            }
        }
        calls_hashed_names_only(FILE, STEP);
    }

    #[test]
    fn a_name_is_the_hash_of_its_definition_and_a_caller_follows_it() {
        let name = &UTC_TIMESTAMP.name;
        assert_eq!(name.len(), "explore.utc_timestamp_".len() + 12);
        assert!(name.starts_with("explore.utc_timestamp_"));
        assert!(
            UTC_TIMESTAMP
                .sql
                .starts_with(&format!("CREATE FUNCTION {name}(value text)"))
        );
        assert!(RECURRENCE_DATES.sql.contains(name.as_str()));
        assert!(
            EVENT_LIVE_UNTIL
                .sql
                .contains(RECURRENCE_DATES.name.as_str())
        );
        assert_ne!(
            function("f", "a".into()).name,
            function("f", "b".into()).name
        );
    }

    /// The guard admits only ISO 8601 forms, which every DateStyle reads the
    /// same way, and the timestamptz branch runs only with an explicit zone,
    /// so the session's TimeZone never reaches a result: under every
    /// DateStyle and TimeZone a session may set, the function reads each
    /// admitted form as the value `ISO, YMD` and `UTC` give and rejects the
    /// rest, which is what its `IMMUTABLE` attribute (read back from
    /// `pg_proc`, so a stored generated column may call it) claims.
    #[tokio::test]
    async fn utc_timestamp_reads_the_same_under_every_datestyle_and_timezone() {
        use chrono::NaiveDateTime;
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let name = &UTC_TIMESTAMP.name;
        let (volatility,): (String,) = sqlx::query_as(
            "SELECT provolatile::text FROM pg_proc WHERE oid = to_regprocedure($1)::oid",
        )
        .bind(format!("{name}(text)"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(volatility, "i", "{name} is IMMUTABLE");
        let inputs = [
            "2024-03-05T06:07:08",
            "2024-03-05",
            "2024-03-05T06:07:08Z",
            "2024-03-05T06:07:08+05:30",
            "2024-03-05T06:07:08.123456Z",
            "05/03/2024",
            "March 5 2024",
            "2024-13-01",
            "0000-01-01",
            "",
            " 2024-03-05T06:07:08",
        ];
        let expected: Vec<Option<NaiveDateTime>> = [
            Some("2024-03-05 06:07:08"),
            Some("2024-03-05 00:00:00"),
            Some("2024-03-05 06:07:08"),
            Some("2024-03-05 00:37:08"),
            Some("2024-03-05 06:07:08.123456"),
            None,
            None,
            None,
            None,
            None,
            None,
        ]
        .into_iter()
        .map(|s| s.map(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").unwrap()))
        .collect();
        let select = format!(
            "SELECT {name}(value) FROM unnest($1::text[]) WITH ORDINALITY AS t(value, i) ORDER BY i"
        );
        for style in [
            "ISO, YMD",
            "ISO, MDY",
            "ISO, DMY",
            "SQL, MDY",
            "SQL, DMY",
            "Postgres, MDY",
            "German, DMY",
        ] {
            for zone in ["UTC", "America/New_York", "Asia/Kolkata"] {
                let mut tx = pool.begin().await.unwrap();
                sqlx::query(
                    "SELECT set_config('DateStyle', $1, true), set_config('TimeZone', $2, true)",
                )
                .bind(style)
                .bind(zone)
                .execute(&mut *tx)
                .await
                .unwrap();
                let read: Vec<(Option<NaiveDateTime>,)> =
                    sqlx::query_as(sqlx::AssertSqlSafe(select.clone()))
                        .bind(&inputs[..])
                        .fetch_all(&mut *tx)
                        .await
                        .unwrap();
                let read: Vec<_> = read.into_iter().map(|(value,)| value).collect();
                assert_eq!(read, expected, "DateStyle {style}, TimeZone {zone}");
                tx.rollback().await.unwrap();
            }
        }
    }
}

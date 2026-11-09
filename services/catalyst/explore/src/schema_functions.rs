//! The explore schema's functions, each named by a hash of its definition as
//! the content write path's are (DATABASE.md): a stored generated column
//! keeps the values one definition computed, so a definition that changes a
//! value is a new function and a new column, never a replaced one.
//! `schemas/10_explore.sql` carries [`functions_sql`] verbatim and a test
//! pins it; a shipped step is immutable (DATABASE.md), so a test pins the
//! names `0030_explore_read_columns.sql` created. A body that computes the
//! same values at a lower cost is the one exception: the binary hashes it
//! under a new name for fresh databases, and a later step replaces the
//! shipped name's body with it (`CREATE OR REPLACE FUNCTION`, the step form
//! schema_migrate admits), so the columns of a migrated database take the
//! new cost without a rewrite; a test pins the replacement to the definition
//! this binary renders. `0033_explore_recurrence_one_pass.sql` does this for
//! the two recurrence functions.

use std::sync::LazyLock;

use sha2::Digest;

use crate::places::query::RAW_TIMESTAMP_IS_CASTABLE_RE;

pub struct Function {
    /// `explore.<prefix>_<hash>`.
    pub name: String,
    /// The `CREATE FUNCTION` statement.
    pub sql: String,
    /// Everything after the name: the parameters, the attributes and the
    /// body, which the name hashes.
    pub tail: String,
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
        tail,
    }
}

/// A time field followed by a zone: the forms of
/// [`RAW_TIMESTAMP_IS_CASTABLE_RE`] whose instant the string fixes itself.
const ZONED_RE: &str = r":[0-9]{2}(\.[0-9]{1,9})?(Z|z|[+-][0-9]{2}(:?[0-9]{2})?)$";

/// The guarded read of the text `value` as a UTC timestamp: NULL unless the
/// guard admits it; read as timestamptz and moved to UTC when it carries a
/// zone, as a plain timestamp otherwise. The lines of a nested `CASE`, the
/// first after `lead`, the rest indented under it, `tail` after the `END`.
/// The functions below carry this expression inline: a function call per
/// array entry cost more than the parse itself (the task report of step
/// 0033 holds the measurement).
fn guarded_utc_lines(lead: &str, tail: &str) -> [String; 5] {
    let rest = " ".repeat(lead.len() - 3);
    let inner = " ".repeat(lead.len() + 7);
    [
        format!("{lead}CASE WHEN value ~ '{RAW_TIMESTAMP_IS_CASTABLE_RE}'"),
        format!("{rest}THEN CASE WHEN value ~ '{ZONED_RE}'"),
        format!("{inner}THEN value::timestamptz AT TIME ZONE 'UTC'"),
        format!("{inner}ELSE value::timestamp END"),
        format!("{rest}END{tail}"),
    ]
}

/// An ISO 8601 date-time string as a UTC timestamp, NULL for anything else.
/// The guard admits the forms every DateStyle reads the same way, so the
/// function is immutable and a generated column may call it. A zone-less
/// string is read as UTC whatever the session's TimeZone: only a value with
/// a time field can carry a zone, so the zone test requires the minutes or
/// seconds digits before it and a date's `-DD` is never taken for an offset.
pub static UTC_TIMESTAMP: LazyLock<Function> = LazyLock::new(|| {
    let mut lines = vec![
        "(value text) RETURNS timestamp without time zone".to_string(),
        "    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT".to_string(),
    ];
    lines.extend(guarded_utc_lines("    RETURN ", ";"));
    function("utc_timestamp", lines.join("\n"))
});

/// The `recurrent_dates` entries of a raw document, an empty array when the
/// key is not an array.
const RECURRENT_DATES_SQL: &str =
    "jsonb_array_elements_text(CASE WHEN jsonb_typeof(raw->'recurrent_dates') = 'array'";

/// The recurrence instants of an event: each `raw.recurrent_dates` entry that
/// is an ISO 8601 date-time (the guard of [`UTC_TIMESTAMP`]), ascending.
/// The materialized parse runs the guard once per entry: pulled up, the
/// `WHERE` and the output would each evaluate it.
pub static RECURRENCE_DATES: LazyLock<Function> = LazyLock::new(|| {
    let mut lines = vec![
        "(raw jsonb) RETURNS timestamp without time zone[]".to_string(),
        "    LANGUAGE sql IMMUTABLE PARALLEL SAFE STRICT".to_string(),
        "    RETURN ARRAY(WITH parsed AS MATERIALIZED (".to_string(),
    ];
    lines.extend(guarded_utc_lines("                   SELECT ", " AS d"));
    lines.extend([
        format!("                   FROM {RECURRENT_DATES_SQL}"),
        "                                                      THEN raw->'recurrent_dates' ELSE '[]'::jsonb END))".to_string(),
        "                 SELECT d FROM parsed WHERE d IS NOT NULL ORDER BY d);".to_string(),
    ]);
    function("recurrence_dates", lines.join("\n"))
});

/// The last instant an event is live under the lists' effective occurrence
/// (`events::EFF_NEXT_START_SQL` plus the duration), NULL when the row has no
/// instant: a live list reads an index range instead of expanding every
/// row's recurrence. One pass over the entries, a bounded aggregate: the
/// last valid instant, and whether there is one.
pub static EVENT_LIVE_UNTIL: LazyLock<Function> = LazyLock::new(|| {
    let mut lines = vec![
        "(next_start_at timestamp without time zone, start_at timestamp without time zone, duration_ms bigint, raw jsonb)".to_string(),
        "    RETURNS timestamp without time zone".to_string(),
        "    LANGUAGE sql IMMUTABLE PARALLEL SAFE".to_string(),
        "    RETURN (SELECT GREATEST(".to_string(),
        "        next_start_at + COALESCE(duration_ms, 0) * interval '1 millisecond',".to_string(),
        "        last.d + COALESCE(duration_ms, 0) * interval '1 millisecond',".to_string(),
        "        CASE WHEN next_start_at IS NULL AND last.d IS NULL".to_string(),
        "             THEN start_at + COALESCE(duration_ms, 0) * interval '1 millisecond' END)".to_string(),
    ];
    lines.extend(guarded_utc_lines("      FROM (SELECT max(", ") AS d"));
    lines.extend([
        format!("            FROM {RECURRENT_DATES_SQL}"),
        "                                               THEN raw->'recurrent_dates' ELSE '[]'::jsonb END)) last);".to_string(),
    ]);
    function("event_live_until", lines.join("\n"))
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
    const REPLACING_STEP: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0033_explore_recurrence_one_pass.sql"
    ));

    /// The functions the shipped step created, by name. The step never
    /// changes, so neither does this list: a changed definition is a new
    /// name in a new step.
    const STEP_FUNCTIONS: [&str; 3] = [
        "explore.utc_timestamp_c37f040ae6c2",
        "explore.recurrence_dates_17e91951d5fe",
        "explore.event_live_until_8c77b09c74f5",
    ];

    /// The shipped names whose body step 0033 replaces with this binary's
    /// definition of the same values.
    const REPLACED: [(&str, &LazyLock<Function>); 2] = [
        ("explore.recurrence_dates_17e91951d5fe", &RECURRENCE_DATES),
        ("explore.event_live_until_8c77b09c74f5", &EVENT_LIVE_UNTIL),
    ];

    /// `file` calls the functions by their hash-named names only.
    fn calls_hashed_names_only(file: &str, text: &str) {
        for prefix in ["utc_timestamp", "recurrence_dates", "event_live_until"] {
            assert!(
                !text.contains(&format!("explore.{prefix}(")),
                "{file} calls {prefix} by its hash-named name"
            );
        }
    }

    /// The fresh schema carries each statement as this binary renders it and
    /// replaces none.
    #[test]
    fn the_schema_carries_the_functions() {
        for sql in functions_sql() {
            assert!(
                SCHEMA.contains(sql),
                "schemas/10_explore.sql must carry this statement exactly:\n\n{sql}\n"
            );
        }
        calls_hashed_names_only("schemas/10_explore.sql", SCHEMA);
        assert!(!SCHEMA.contains("CREATE OR REPLACE FUNCTION"));
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
        assert!(!STEP.contains("CREATE OR REPLACE FUNCTION"));
    }

    /// Step 0033 replaces each shipped body with the tail this binary hashes
    /// under the new name, so a migrated database's columns and a fresh
    /// one's compute the same values at the same cost.
    #[test]
    fn the_replacing_step_carries_this_binary_s_bodies() {
        const FILE: &str = "schemas/migrations/0033_explore_recurrence_one_pass.sql";
        for (name, function) in REPLACED {
            assert_ne!(function.name, name, "{name} is a superseded name");
            let expected = format!("CREATE OR REPLACE FUNCTION {name}{}", function.tail);
            assert!(
                REPLACING_STEP.contains(&expected),
                "{FILE} must replace {name} with this binary's body exactly:\n\n{expected}\n"
            );
        }
        assert!(
            !REPLACING_STEP.contains("CREATE FUNCTION "),
            "{FILE} creates nothing"
        );
        calls_hashed_names_only(FILE, REPLACING_STEP);
    }

    #[test]
    fn a_name_is_the_hash_of_its_definition_and_the_guard_is_shared() {
        let name = &UTC_TIMESTAMP.name;
        assert_eq!(name.len(), "explore.utc_timestamp_".len() + 12);
        assert!(name.starts_with("explore.utc_timestamp_"));
        assert!(
            UTC_TIMESTAMP
                .sql
                .starts_with(&format!("CREATE FUNCTION {name}(value text)"))
        );
        assert_eq!(
            UTC_TIMESTAMP.sql,
            format!("CREATE FUNCTION {name}{}", UTC_TIMESTAMP.tail)
        );
        for function in [&*RECURRENCE_DATES, &*EVENT_LIVE_UNTIL] {
            assert_eq!(
                function.sql.matches(RAW_TIMESTAMP_IS_CASTABLE_RE).count(),
                1
            );
            assert_eq!(function.sql.matches(ZONED_RE).count(), 1);
            assert!(!function.sql.contains(name.as_str()));
        }
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

    /// The statement of `STEP` that creates `name`, as shipped.
    fn shipped_statement(name: &str) -> &'static str {
        let start = STEP
            .find(&format!("CREATE FUNCTION {name}("))
            .unwrap_or_else(|| panic!("0030 creates {name}"));
        let end = STEP[start..].find(";\n").unwrap() + start + 1;
        &STEP[start..end]
    }

    /// The one-pass bodies compute what the shipped step's did for every
    /// shape of document: a migrated database replaces the shipped bodies in
    /// place, so its stored columns stay the values the new definition
    /// computes. The shipped functions are created under their names inside
    /// a transaction that is rolled back (a fresh database has only the
    /// current names), and both pairs read the same documents under a zone
    /// other than UTC.
    #[tokio::test]
    async fn the_one_pass_bodies_compute_what_the_shipped_ones_did() {
        let Some(pool) = crate::db::scratch::pool(1).await else {
            return;
        };
        let documents = [
            r#"{"recurrent_dates": ["2031-01-02T03:04:05.000Z", "2030-12-31T23:00:00Z", "2031-06-01T10:00:00+05:30"]}"#,
            r#"{"recurrent_dates": ["2031-01-02T03:04:05", "2031-01-02", "2031-01-02 03:04:05.123456789z"]}"#,
            r#"{"recurrent_dates": ["junk", "2031-13-01T00:00:00Z", "", "0000-01-01", "2031-02-29T00:00:00Z", "2032-02-29T00:00:00Z"]}"#,
            r#"{"recurrent_dates": ["2031-01-02T03:04:05Z", "2031-01-02T03:04:05Z", "2020-01-01T00:00:00Z"]}"#,
            r#"{"recurrent_dates": []}"#,
            r#"{"recurrent_dates": "2031-01-02T03:04:05Z"}"#,
            r#"{"recurrent_dates": null}"#,
            r#"{"recurrent_dates": [null, 1, true, {"a": "2031-01-02T03:04:05Z"}]}"#,
            r#"{}"#,
        ];
        let instants: [Option<&str>; 3] = [
            None,
            Some("2020-01-01T00:00:00"),
            Some("2031-03-01T00:00:00"),
        ];
        let durations: [Option<i64>; 3] = [None, Some(0), Some(3_600_000)];
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('TimeZone', 'America/New_York', true)")
            .execute(&mut *tx)
            .await
            .unwrap();
        for (name, _) in REPLACED {
            sqlx::raw_sql(sqlx::AssertSqlSafe(shipped_statement(name).to_string()))
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        let (old_dates, old_until) = (REPLACED[0].0, REPLACED[1].0);
        let (new_dates, new_until) = (&RECURRENCE_DATES.name, &EVENT_LIVE_UNTIL.name);
        let compare = format!(
            "SELECT {old_dates}(d.raw)::text, {new_dates}(d.raw)::text, \
                    {old_until}(d.next_start_at, d.start_at, d.duration_ms, d.raw)::text, \
                    {new_until}(d.next_start_at, d.start_at, d.duration_ms, d.raw)::text \
             FROM (SELECT $1::jsonb AS raw, $2::timestamp AS next_start_at, $3::timestamp AS start_at, $4::bigint AS duration_ms) d"
        );
        let mut compared = 0;
        for raw in documents {
            for next_start_at in instants {
                for start_at in instants {
                    for duration_ms in durations {
                        let row: (
                            Option<String>,
                            Option<String>,
                            Option<String>,
                            Option<String>,
                        ) = sqlx::query_as(sqlx::AssertSqlSafe(compare.clone()))
                            .bind(raw)
                            .bind(next_start_at)
                            .bind(start_at)
                            .bind(duration_ms)
                            .fetch_one(&mut *tx)
                            .await
                            .unwrap();
                        assert_eq!(row.0, row.1, "recurrence_dates of {raw}");
                        assert_eq!(
                            row.2, row.3,
                            "live_until of {raw}, next_start_at {next_start_at:?}, start_at {start_at:?}, duration_ms {duration_ms:?}"
                        );
                        compared += 1;
                    }
                }
            }
        }
        assert_eq!(compared, documents.len() * 27);
        // The one input the bodies answer apart, which no generated column
        // passes (`raw` is NOT NULL): a SQL NULL document, where the shipped
        // body's cardinality(NULL) left the start_at arm NULL.
        let (shipped, one_pass): (Option<String>, Option<String>) =
            sqlx::query_as(sqlx::AssertSqlSafe(format!(
                "SELECT {old_until}(NULL, '2031-03-01'::timestamp, 1000, NULL)::text, \
                        {new_until}(NULL, '2031-03-01'::timestamp, 1000, NULL)::text"
            )))
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            (shipped, one_pass.as_deref()),
            (None, Some("2031-03-01 00:00:01"))
        );
        tx.rollback().await.unwrap();
    }
}

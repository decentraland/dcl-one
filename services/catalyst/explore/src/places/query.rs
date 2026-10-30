//! WHERE/ORDER BY assembly for the place listings over `explore.place_indexed`,
//! and the guarded casts every raw jsonb read goes through.

use std::sync::LazyLock;

use super::content_quality::build_content_quality_condition;
use super::rows::{PlaceListFilters, PlaceOrderBy};
use crate::sanitize::MAX_SANITIZE_PASSES;

/// The read-side mirror of the `description_plain` generated column
/// (schemas/10_explore.sql): as many strip passes as the sanitizer, then every
/// angle bracket dropped. The two are pinned against drift by a test.
pub(super) fn description_plain_sql() -> &'static str {
    static SQL: LazyLock<String> = LazyLock::new(|| {
        let mut stripped = "coalesce(description, '')".to_string();
        for _ in 0..MAX_SANITIZE_PASSES {
            stripped = format!("regexp_replace({stripped}, '</?[a-zA-Z][^>]*>', '', 'g')");
        }
        format!(
            "CASE WHEN strpos(coalesce(description, ''), '<') = 0 \
             AND strpos(coalesce(description, ''), '>') = 0 \
             THEN coalesce(description, '') \
             ELSE regexp_replace({stripped}, '[<>]', '', 'g') END"
        )
    });
    &SQL
}

pub(super) const IS_PRIVATE_SQL: &str =
    "COALESCE(NULLIF(raw->'is_private', 'null'::jsonb) = 'true'::jsonb, false)";

pub(super) const SHOW_IN_PLACES_SQL: &str =
    "COALESCE(NULLIF(raw->'show_in_places', 'null'::jsonb) = 'true'::jsonb, true)";

pub(super) const SINGLE_PLAYER_SQL: &str =
    "COALESCE(NULLIF(raw->'single_player', 'null'::jsonb) = 'true'::jsonb, false)";

pub(super) const USER_FAVORITE_SQL: &str =
    "COALESCE(NULLIF(raw->'user_favorite', 'null'::jsonb) = 'true'::jsonb, false)";

pub(super) const USER_LIKE_SQL: &str =
    "COALESCE(NULLIF(raw->'user_like', 'null'::jsonb) = 'true'::jsonb, false)";

pub(super) const USER_DISLIKE_SQL: &str =
    "COALESCE(NULLIF(raw->'user_dislike', 'null'::jsonb) = 'true'::jsonb, false)";

pub(super) const SHOW_IN_PLACES_CLAUSE: &str = "(world IS FALSE OR COALESCE(NULLIF(raw->'show_in_places', 'null'::jsonb) = 'true'::jsonb, true) IS TRUE)";

pub(crate) const EXCLUDE_FROM_RANKING_SQL: &str =
    "COALESCE(NULLIF(raw->'exclude_from_ranking', 'null'::jsonb) = 'true'::jsonb, false)";

/// Bound magnitude, never precision: float8 errors both above 1.7976931348623157e308
/// and below the smallest subnormal, so every alternative here must keep the value
/// it admits inside both ends -- a digit run that can reach either is an abort, and a
/// digit bound tighter than the double it describes nulls a value the cast reads fine.
/// Keep every repetition count at or below 255 -- Postgres ARE rejects more.
pub(super) const RAW_FLOAT_IS_CASTABLE_RE: &str = concat!(
    r"^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})",
    r"(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?",
    r"|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])",
    r"|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]",
    r"|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$"
);

pub(super) const RAW_INT_IS_CASTABLE_RE: &str = r"^-?[0-9]{1,10}$";

/// Year zero is not a timestamptz and Postgres caps a timezone displacement at
/// +/-15:59:59, so neither year run may reach 0000 and the offset hours must stop
/// at 15 -- both abort the statement at the cast, not just the row.
pub(super) const RAW_TIMESTAMP_IS_CASTABLE_RE: &str = concat!(
    r"^((000[1-9]|00[1-9][0-9]|0[1-9][0-9]{2}|[1-9][0-9]{3})-",
    r"((0[13578]|1[02])-(0[1-9]|[12][0-9]|3[01])",
    r"|(0[469]|11)-(0[1-9]|[12][0-9]|30)",
    r"|02-(0[1-9]|1[0-9]|2[0-8]))",
    r"|([0-9]{2}(0[48]|[2468][048]|[13579][26])",
    r"|(0[48]|[2468][048]|[13579][26])00)-02-29)",
    r"([T ]([01][0-9]|2[0-3]):[0-5][0-9]",
    r"(:[0-5][0-9](\.[0-9]{1,9})?)?",
    r"(Z|z|[+-](0[0-9]|1[0-5])(:?[0-5][0-9])?)?)?$"
);

fn raw_cast_sql(key: &str, kinds: &str, pattern: &str, cast: &str) -> String {
    format!(
        "CASE WHEN jsonb_typeof(raw->'{key}') IN ({kinds}) \
         AND raw->>'{key}' ~ '{pattern}' \
         THEN (raw->>'{key}')::{cast} END"
    )
}

pub(crate) fn raw_float8_sql(key: &str) -> String {
    raw_cast_sql(
        key,
        "'number', 'string'",
        RAW_FLOAT_IS_CASTABLE_RE,
        "float8",
    )
}

/// Keep the range test in a CASE of its own: Postgres does not promise to
/// short-circuit an AND, so the `::bigint` must never share a WHEN with the
/// regex that protects it.
pub(super) fn raw_int_sql(key: &str) -> String {
    format!(
        "CASE WHEN jsonb_typeof(raw->'{key}') IN ('number', 'string') \
         THEN CASE WHEN raw->>'{key}' ~ '{RAW_INT_IS_CASTABLE_RE}' \
         THEN CASE WHEN (raw->>'{key}')::bigint BETWEEN -2147483648 AND 2147483647 \
         THEN (raw->>'{key}')::int END END END"
    )
}

pub(super) fn raw_timestamptz_sql(key: &str) -> String {
    raw_cast_sql(key, "'string'", RAW_TIMESTAMP_IS_CASTABLE_RE, "timestamptz")
}

#[derive(Debug)]
pub(super) enum Bind {
    Text(String),
    TextArray(Vec<String>),
    UuidArray(Vec<uuid::Uuid>),
    JsonbArray(Vec<serde_json::Value>),
    Int(i32),
}

/// The request's own-rows restriction (`copies::OwnRowsOnly`), if any.
pub(super) fn own_rows_sql() -> impl Iterator<Item = &'static str> {
    crate::copies::origin::place_rows_sql().into_iter()
}

#[cfg(test)]
pub(super) fn build_where(f: &PlaceListFilters, road_positions: bool) -> (String, Vec<Bind>) {
    build_where_from(f, road_positions, 1, false)
}

pub(super) fn build_where_from(
    f: &PlaceListFilters,
    road_positions: bool,
    start: usize,
    scene_projection: bool,
) -> (String, Vec<Bind>) {
    let mut clauses: Vec<String> = vec![if f.include_opted_out {
        "(disabled IS FALSE OR raw->>'disabled_reason' = 'opt_out')".to_string()
    } else {
        "disabled IS FALSE".to_string()
    }];
    clauses.extend(own_rows_sql().map(String::from));
    let mut binds: Vec<Bind> = Vec::new();
    let mut idx = start;

    if !f.ids.is_empty() {
        clauses.push(format!("id = ANY(${idx})"));
        binds.push(Bind::TextArray(f.ids.clone()));
        idx += 1;
    }
    if f.viewer_favorites_only {
        match &f.viewer {
            Some(viewer) => {
                clauses.push(format!(
                    r#"EXISTS (SELECT 1 FROM explore.user_favorites uf WHERE uf.entity_id = id AND lower(uf."user") = ${idx})"#
                ));
                binds.push(Bind::Text(viewer.to_lowercase()));
                idx += 1;
            }
            None => clauses.push("FALSE".to_string()),
        }
    }
    if f.only_worlds {
        clauses.push("world IS TRUE".to_string());
    } else if f.only_places {
        clauses.push(if scene_projection {
            "(world IS FALSE OR canonical_scene)".to_string()
        } else {
            "world IS FALSE".to_string()
        });
    }
    clauses.push(SHOW_IN_PLACES_CLAUSE.to_string());
    if f.only_categorized {
        clauses.push("array_length(categories, 1) > 0".to_string());
    }
    if f.only_highlighted {
        clauses.push("highlighted = TRUE".to_string());
    }
    if f.only_excluded_from_ranking {
        clauses.push(format!("{EXCLUDE_FROM_RANKING_SQL} IS TRUE"));
    }
    // An explicit pointer narrows a lookup independently of ownership. Operator
    // parcels are an alternative to the owner leg, never alternative pointers.
    let positions = f.positions.clone();
    let mut positions_clause = None;
    if !positions.is_empty() {
        positions_clause = Some(format!("raw->'positions' @> ANY(${idx}::jsonb[])"));
        binds.push(Bind::JsonbArray(
            positions
                .into_iter()
                .map(|p| serde_json::json!([p]))
                .collect(),
        ));
        idx += 1;
    }
    let mut names_clause = None;
    if !f.names.is_empty() {
        names_clause = Some(format!("lower(world_name) = ANY(${idx})"));
        binds.push(Bind::TextArray(
            f.names.iter().map(|n| n.to_lowercase()).collect(),
        ));
        idx += 1;
    }
    if !f.partial_names.is_empty() {
        let clause = format!("lower(world_name) LIKE ANY(${idx})");
        binds.push(Bind::TextArray(
            f.partial_names
                .iter()
                .map(|name| format!("%{}%", name.to_lowercase()))
                .collect(),
        ));
        idx += 1;
        names_clause = Some(match names_clause {
            Some(exact) => format!("({exact} AND {clause})"),
            None => clause,
        });
    }
    match (positions_clause, names_clause) {
        (Some(p), Some(n)) => clauses.push(format!("({p} OR {n})")),
        (Some(p), None) => clauses.push(p),
        (None, Some(n)) => clauses.push(n),
        (None, None) => {}
    }
    if let Some(owner) = &f.owner {
        let mut owner_clause = format!("LOWER(COALESCE(raw->>'owner', creator_address)) = ${idx}");
        binds.push(Bind::Text(owner.to_lowercase()));
        idx += 1;
        if !f.operated_positions.is_empty() {
            owner_clause = format!(
                "({owner_clause} OR (world IS FALSE AND raw->'positions' @> ANY(${idx}::jsonb[])))"
            );
            binds.push(Bind::JsonbArray(
                f.operated_positions
                    .iter()
                    .map(|p| serde_json::json!([p]))
                    .collect(),
            ));
            idx += 1;
        }
        clauses.push(owner_clause);
    } else if f.owner_filtered {
        // The caller requested an owner but supplied no usable identity.
        clauses.push("FALSE".to_string());
    }
    if !f.categories.is_empty() {
        clauses.push(format!("categories && ${idx}"));
        binds.push(Bind::TextArray(f.categories.clone()));
        idx += 1;
    }
    if let Some(addr) = &f.creator_address {
        clauses.push(format!("LOWER(creator_address) = ${idx}"));
        binds.push(Bind::Text(addr.to_lowercase()));
        idx += 1;
    }
    if let Some(sdk) = &f.sdk {
        let null_clause = if sdk == "6" {
            " OR raw->>'sdk' IS NULL"
        } else {
            ""
        };
        clauses.push(format!(
            "(raw->>'sdk' = ${0} OR raw->>'sdk' LIKE ${1}{2})",
            idx,
            idx + 1,
            null_clause
        ));
        binds.push(Bind::Text(sdk.clone()));
        binds.push(Bind::Text(format!("{sdk}.%")));
        idx += 2;
    }
    if let Some(s) = &f.search {
        clauses.push(format!(
            "(to_tsvector('english', coalesce(title,'') || ' ' || ({plain})) @@ plainto_tsquery('english', ${0}) \
             OR title ILIKE ${1} OR ({plain}) ILIKE ${1} OR world_name ILIKE ${1})",
            idx,
            idx + 1,
            plain = description_plain_sql(),
        ));
        binds.push(Bind::Text(s.clone()));
        binds.push(Bind::Text(format!("%{s}%")));
        idx += 2;
    }
    if let Some((clause, gate_binds)) = build_content_quality_condition(f, road_positions, idx) {
        clauses.push(clause);
        binds.extend(gate_binds);
    }
    (clauses.join(" AND "), binds)
}

pub(super) fn build_live_user_count_order(
    f: &PlaceListFilters,
    start_idx: usize,
) -> (String, Vec<Bind>) {
    if !matches!(f.order_by, PlaceOrderBy::MostActive) {
        return (String::new(), Vec::new());
    }
    if f.place_user_counts.is_empty() && f.world_user_counts.is_empty() {
        return (String::new(), Vec::new());
    }
    let mut binds: Vec<Bind> = Vec::new();
    let mut idx = start_idx;

    let places_case = if f.place_user_counts.is_empty() {
        "0".to_string()
    } else {
        let mut whens = String::new();
        for (pos, count) in &f.place_user_counts {
            whens.push_str(&format!(" WHEN ${} THEN ${}", idx, idx + 1));
            binds.push(Bind::Text(pos.clone()));
            binds.push(Bind::Int(*count));
            idx += 2;
        }
        format!("COALESCE(CASE base_position{whens} ELSE 0 END, 0)")
    };

    let worlds_case = if f.world_user_counts.is_empty() {
        "0".to_string()
    } else {
        let mut whens = String::new();
        for (name, count) in &f.world_user_counts {
            whens.push_str(&format!(" WHEN ${} THEN ${}", idx, idx + 1));
            binds.push(Bind::Text(name.to_lowercase()));
            binds.push(Bind::Int(*count));
            idx += 2;
        }
        format!("COALESCE(CASE lower(world_name){whens} ELSE 0 END, 0)")
    };

    let expr = format!("(CASE WHEN world THEN {worlds_case} ELSE {places_case} END)::int DESC, ");
    (expr, binds)
}

pub(super) fn destinations_highlighted_prefix(f: &PlaceListFilters) -> &'static str {
    if f.destinations_mode {
        "highlighted DESC, "
    } else {
        ""
    }
}

/// The featured shelf leads the destinations and the live count follows,
/// except under `most_active` (the only order with live counts), where
/// people outrank curation: a busy uncurated destination lists above an
/// empty featured one and `highlighted` only breaks ties among equal counts.
pub(super) fn destinations_lead_prefix(f: &PlaceListFilters, live_prefix: &str) -> String {
    let highlighted = destinations_highlighted_prefix(f);
    if matches!(f.order_by, PlaceOrderBy::MostActive) {
        format!("{live_prefix}{highlighted}")
    } else {
        format!("{highlighted}{live_prefix}")
    }
}

pub(super) fn destinations_ranking_prefix(f: &PlaceListFilters) -> &'static str {
    static SQL: LazyLock<String> =
        LazyLock::new(|| format!("COALESCE({}, 0) DESC, ", raw_float8_sql("ranking")));
    if f.destinations_mode { &SQL } else { "" }
}

pub(super) const PLACES_ORDER_TAIL: &str = "deployed_at DESC, id ASC";

/// Upstream ends a places-shaped listing on the deployment time and a
/// worlds-shaped or union one on the last update; a branch added here must pick
/// the tail of the upstream branch it mirrors. Upstream reads a NOT NULL column
/// on both legs while ours reads a raw jsonb key a locally published row never
/// carries, so the tail must fall back to the deployment time rather than
/// collapse that whole population onto the identifier.
pub(super) fn order_tail(f: &PlaceListFilters) -> &'static str {
    static WORLDS_TAIL: LazyLock<String> = LazyLock::new(|| {
        format!(
            "COALESCE({}, deployed_at) DESC, id ASC",
            raw_timestamptz_sql("updated_at")
        )
    });
    if f.only_worlds || (f.destinations_mode && !f.only_places) {
        &WORLDS_TAIL
    } else {
        PLACES_ORDER_TAIL
    }
}

pub(super) fn build_order_by(
    lead_prefix: &str,
    ranking_prefix: &str,
    rank_prefix: &str,
    order_column: &str,
    dir: &str,
    tail: &str,
) -> String {
    format!("{lead_prefix}{ranking_prefix}{rank_prefix}{order_column} {dir} NULLS LAST, {tail}")
}

pub(super) fn bind_param<'a>(
    q: sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments>,
    b: &'a Bind,
) -> sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match b {
        Bind::Text(s) => q.bind(s),
        Bind::TextArray(v) => q.bind(v),
        Bind::UuidArray(v) => q.bind(v),
        Bind::JsonbArray(v) => q.bind(v),
        Bind::Int(n) => q.bind(*n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = include_str!(concat!(env!("DCL_ONE_ROOT"), "/schemas/10_explore.sql"));

    fn squeezed(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn description_plain_sql_strips_once_per_sanitizer_pass_then_fails_closed() {
        let sql = description_plain_sql();
        assert_eq!(
            sql.matches("'</?[a-zA-Z][^>]*>'").count(),
            MAX_SANITIZE_PASSES,
            "one strip pass per sanitizer pass: {sql}"
        );
        assert!(sql.ends_with("'[<>]', '', 'g') END"), "{sql}");
    }

    #[test]
    fn description_plain_sql_is_mirrored_by_both_generated_columns() {
        let schema = squeezed(SCHEMA);
        let expr = squeezed(description_plain_sql());
        assert_eq!(
            schema.matches(expr.as_str()).count(),
            2,
            "schemas/10_explore.sql must generate place and place_world_local with {expr}"
        );
    }

    #[test]
    fn the_like_score_indexes_carry_the_exact_order_column() {
        let expr = PlaceOrderBy::LikeScore.column();
        assert_eq!(
            SCHEMA.matches(expr).count(),
            2,
            "one partial index per leg must index {expr} byte for byte"
        );
    }

    #[test]
    fn content_gate_binds_follow_the_search_binds() {
        let f = PlaceListFilters {
            search: Some("garden".to_string()),
            destinations_mode: true,
            require_content_places: true,
            require_content_worlds: true,
            ..Default::default()
        };
        let (clause, binds) = build_where(&f, true);
        assert_eq!(
            binds.len(),
            7,
            "search takes $1/$2, the gate $3..$7: {clause}"
        );
        assert!(clause.contains("LIKE ANY ($3::text[])"), "{clause}");
        assert!(clause.contains("title !~ $4"), "{clause}");
        assert!(clause.contains("<> ALL ($6::text[])"), "{clause}");
        assert!(clause.contains("<> $7)"), "{clause}");
        assert!(
            clause.contains("FROM explore.road_positions rp"),
            "{clause}"
        );
    }

    #[test]
    fn content_gate_is_absent_from_lookup_and_places_queries() {
        let f = PlaceListFilters {
            destinations_mode: true,
            ..Default::default()
        };
        let (clause, binds) = build_where(&f, true);
        assert!(!clause.contains("highlighted IS TRUE"), "{clause}");
        assert!(binds.is_empty());
        let (clause, _) = build_where(&PlaceListFilters::default(), true);
        assert!(!clause.contains("highlighted IS TRUE"), "{clause}");
    }

    #[test]
    fn every_list_hides_worlds_that_opted_out_of_the_directory() {
        for f in [
            PlaceListFilters::default(),
            PlaceListFilters {
                only_worlds: true,
                ..Default::default()
            },
            PlaceListFilters {
                ids: vec!["world-row".to_string()],
                destinations_mode: true,
                ..Default::default()
            },
        ] {
            let (clause, _) = build_where(&f, true);
            assert!(clause.contains(SHOW_IN_PLACES_CLAUSE), "{clause}");
        }
        assert!(SHOW_IN_PLACES_CLAUSE.starts_with("(world IS FALSE OR"));
    }

    #[test]
    fn the_map_lists_categorized_places_only() {
        let (clause, _) = build_where(
            &PlaceListFilters {
                only_categorized: true,
                ..Default::default()
            },
            true,
        );
        assert!(
            clause.contains("array_length(categories, 1) > 0"),
            "{clause}"
        );
        let (clause, _) = build_where(&PlaceListFilters::default(), true);
        assert!(!clause.contains("array_length(categories"), "{clause}");
    }

    #[test]
    fn every_raw_boolean_reads_through_a_jsonb_comparison() {
        let columns = super::super::rows::place_columns();
        for (key, sql, absent) in [
            ("is_private", IS_PRIVATE_SQL, "false"),
            ("show_in_places", SHOW_IN_PLACES_SQL, "true"),
            ("single_player", SINGLE_PLAYER_SQL, "false"),
            ("user_favorite", USER_FAVORITE_SQL, "false"),
            ("user_like", USER_LIKE_SQL, "false"),
            ("user_dislike", USER_DISLIKE_SQL, "false"),
            ("exclude_from_ranking", EXCLUDE_FROM_RANKING_SQL, "false"),
        ] {
            assert_eq!(
                sql,
                format!("COALESCE(NULLIF(raw->'{key}', 'null'::jsonb) = 'true'::jsonb, {absent})")
            );
            assert!(columns.contains(sql), "{key} must be selected as {sql}");
        }
        assert!(SHOW_IN_PLACES_CLAUSE.contains(SHOW_IN_PLACES_SQL));
    }

    fn scanned_sources() -> [(&'static str, &'static str); 5] {
        [
            ("rows.rs", include_str!("rows.rs")),
            ("mod.rs", include_str!("mod.rs")),
            ("content_quality.rs", include_str!("content_quality.rs")),
            ("query.rs", include_str!("query.rs")),
            ("mirror.rs", include_str!("mirror.rs")),
        ]
    }

    #[test]
    fn no_raw_key_is_cast_to_bool() {
        let needle = format!("'){}", "::bool");
        for (file, src) in scanned_sources() {
            assert!(
                !src.contains(&needle),
                "{file} casts a raw jsonb key to bool; one non-boolean value there \
                 aborts the whole listing query"
            );
        }
        let world_backfill = format!("(raw->>'world'){}", "::boolean");
        let place_ddl = SCHEMA
            .split("-- ── events")
            .next()
            .unwrap()
            .replace(world_backfill.as_str(), "");
        assert!(
            !place_ddl.contains(&needle),
            "schemas/10_explore.sql casts a raw jsonb key of a place row to bool"
        );
    }

    fn cast_scanner() -> regex::Regex {
        regex::Regex::new(concat!(
            r"raw\s*(?:->>|#>>)\s*'\{?([a-z_]+)\}?'\s*(?:,\s*'')?\)?::",
            r"(boolean|bool|bigint|integer|int4|int8|int|float8|numeric",
            r"|double precision|timestamptz|timestamp|date)"
        ))
        .unwrap()
    }

    #[test]
    fn no_raw_key_is_cast_without_a_type_guard() {
        let cast = cast_scanner();
        for (file, src) in scanned_sources() {
            for m in cast.captures_iter(src) {
                let whole = m.get(0).unwrap();
                let key = &m[1];
                let ty = &m[2];
                assert!(
                    ty != "bool" && ty != "boolean",
                    "{file} casts the raw jsonb key {key} to bool"
                );
                let window = &src[whole.start().saturating_sub(400)..whole.start()];
                assert!(
                    window.contains(&format!("jsonb_typeof(raw->'{key}')"))
                        || window.contains(&format!("jsonb_typeof(raw->'{{{key}}}')")),
                    "{file} casts the raw jsonb key {key} to {ty} with no jsonb_typeof guard"
                );
            }
        }
    }

    fn assert_generated_casts_are_guarded(label: &str, sql: &str) -> usize {
        let cast = cast_scanner();
        let mut seen = 0usize;
        for m in cast.captures_iter(sql) {
            seen += 1;
            let at = m.get(0).unwrap().start();
            let key = &m[1];
            let ty = &m[2];
            assert!(
                ty != "bool" && ty != "boolean",
                "{label} casts the raw jsonb key {key} to bool: {sql}"
            );
            let guard = format!("jsonb_typeof(raw->'{key}')");
            let Some(opened) = sql[..at].rfind(&guard) else {
                panic!("{label} casts the raw jsonb key {key} to {ty} unguarded: {sql}");
            };
            assert!(
                !sql[opened..at].contains(" END"),
                "{label} closes the CASE guarding {key} before the {ty} cast: {sql}"
            );
        }
        seen
    }

    #[test]
    fn every_generated_statement_guards_the_casts_it_emits() {
        let destinations = PlaceListFilters {
            destinations_mode: true,
            ..Default::default()
        };
        let columns = assert_generated_casts_are_guarded(
            "place_columns",
            super::super::rows::place_columns(),
        );
        assert!(
            columns >= 9,
            "the scanner must see every cast place_columns emits: {columns}"
        );
        assert_generated_casts_are_guarded(
            "destinations_ranking_prefix",
            destinations_ranking_prefix(&destinations),
        );
        assert_generated_casts_are_guarded("order_tail(destinations)", order_tail(&destinations));
        assert_generated_casts_are_guarded(
            "order_tail(places)",
            order_tail(&PlaceListFilters::default()),
        );
        for order_by in [
            PlaceOrderBy::LikeScore,
            PlaceOrderBy::UpdatedAt,
            PlaceOrderBy::CreatedAt,
            PlaceOrderBy::UserVisits,
            PlaceOrderBy::MostActive,
        ] {
            assert_generated_casts_are_guarded("PlaceOrderBy::column", order_by.column());
        }
        let place_ddl = SCHEMA.split("-- ── events").next().unwrap();
        assert_generated_casts_are_guarded("schemas/10_explore.sql (places)", place_ddl);
    }

    #[test]
    fn raw_float_castability_bounds_magnitude_not_precision() {
        let re = regex::Regex::new(RAW_FLOAT_IS_CASTABLE_RE).unwrap();
        for ok in [
            "0.30000000000000004",
            "0.0000000000000000000000000000000000000000000010",
            "0.85771024",
            "4",
            "1.5e-7",
            "-3.25",
            "1e-300",
            "999e305",
            "1e306",
            "1e307",
            "1.7e308",
        ] {
            assert!(re.is_match(ok), "a float8 reads {ok} back unchanged");
        }
        for bad in [
            "1e999", "1e309", "2e308", "1.8e308", "1e-400", "N/A", "true", "", "1.0.0",
        ] {
            assert!(
                !re.is_match(bad),
                "{bad} must fail soft to NULL, not reach the cast"
            );
        }
        assert!(re.is_match(&"9".repeat(308)));
        assert!(!re.is_match(&"9".repeat(309)));
        assert!(re.is_match(&format!("0.{}1", "0".repeat(322))));
        assert!(!re.is_match(&format!("0.{}1", "0".repeat(323))));
    }

    #[test]
    fn raw_timestamp_castability_refuses_every_instant_postgres_cannot_read() {
        let re = regex::Regex::new(RAW_TIMESTAMP_IS_CASTABLE_RE).unwrap();
        for ok in [
            "2024-02-29",
            "2000-02-29T00:00:00Z",
            "2023-02-28T23:59:59Z",
            "2023-10-17T16:31:57.766Z",
            "2024-01-01T00:00:00+14:00",
            "2024-01-01T00:00:00-15:59",
            "0001-01-01",
        ] {
            assert!(re.is_match(ok), "{ok} is a real instant");
        }
        for bad in [
            "2023-02-29T00:00:00Z",
            "1900-02-29",
            "2100-02-29",
            "2023-02-30T00:00:00Z",
            "2023-13-01T00:00:00Z",
            "0000-01-01T00:00:00Z",
            "0000-02-29",
            "2024-01-01T00:00:00+16:30",
            "2024-01-01T00:00:00+23:00",
            "yesterday",
        ] {
            assert!(
                !re.is_match(bad),
                "{bad} aborts the whole listing if it reaches the cast"
            );
        }
    }

    #[test]
    fn every_raw_cast_column_carries_its_own_guard() {
        let columns = super::super::rows::place_columns();
        assert!(!columns.contains("NULLIF(raw->>"), "{columns}");
        for key in [
            "ranking",
            "skybox_time",
            "like_rate",
            "like_score",
            "user_count",
            "user_visits",
            "disabled_at",
            "created_at",
            "updated_at",
        ] {
            assert!(
                columns.contains(&format!("jsonb_typeof(raw->'{key}')")),
                "{key} must be guarded"
            );
        }
    }

    #[test]
    fn a_guarded_cast_keeps_the_shape_the_value_must_have() {
        let ranking = raw_float8_sql("ranking");
        assert!(ranking.contains("jsonb_typeof(raw->'ranking') IN ('number', 'string')"));
        assert!(ranking.ends_with("THEN (raw->>'ranking')::float8 END"));
        let user_count = raw_int_sql("user_count");
        assert_eq!(user_count.matches("CASE WHEN").count(), 3, "{user_count}");
        assert!(
            user_count.starts_with(
                "CASE WHEN jsonb_typeof(raw->'user_count') IN ('number', 'string') THEN"
            )
        );
        assert!(user_count.contains(&format!("~ '{RAW_INT_IS_CASTABLE_RE}' THEN")));
        assert!(user_count.contains("BETWEEN -2147483648 AND 2147483647"));
        let created = raw_timestamptz_sql("created_at");
        assert!(created.contains("jsonb_typeof(raw->'created_at') IN ('string')"));
        assert!(created.contains(RAW_TIMESTAMP_IS_CASTABLE_RE));
    }

    #[test]
    fn both_search_legs_read_the_stripped_text() {
        let (clause, _) = build_where(
            &PlaceListFilters {
                search: Some("garden".to_string()),
                ..Default::default()
            },
            false,
        );
        assert_eq!(
            clause.matches(description_plain_sql()).count(),
            2,
            "{clause}"
        );
        assert!(!clause.contains("|| coalesce(description,'')"), "{clause}");
    }

    #[test]
    fn names_owner_sdk_and_creator_clauses() {
        let f = PlaceListFilters {
            names: vec!["Foo.DCL.eth".to_string()],
            ..Default::default()
        };
        let (where_clause, binds) = build_where(&f, false);
        assert!(where_clause.contains("lower(world_name) = ANY"));
        match binds.last().unwrap() {
            Bind::TextArray(v) => assert_eq!(v, &vec!["foo.dcl.eth".to_string()]),
            _ => panic!("expected names text array bind"),
        }

        let f = PlaceListFilters {
            operated_positions: vec!["10,20".to_string()],
            owner: Some("0xabc".to_string()),
            ..Default::default()
        };
        let (where_clause, binds) = build_where(&f, false);
        assert!(where_clause.contains("raw->'positions' @> ANY($2::jsonb[])"));
        match binds.last().unwrap() {
            Bind::JsonbArray(v) => assert_eq!(v, &vec![serde_json::json!(["10,20"])]),
            _ => panic!("expected positions jsonb array bind"),
        }

        let f = PlaceListFilters {
            owner_filtered: true,
            ..Default::default()
        };
        let (where_clause, _) = build_where(&f, false);
        assert!(where_clause.contains("AND FALSE"));
        assert!(!where_clause.contains("raw->'positions' @>"));
        assert!(
            !build_where(&PlaceListFilters::default(), false)
                .0
                .contains("AND FALSE")
        );

        let f = PlaceListFilters {
            sdk: Some("7".to_string()),
            ..Default::default()
        };
        let (where_clause, _) = build_where(&f, false);
        assert!(where_clause.contains("raw->>'sdk' ="));
        assert!(where_clause.contains("raw->>'sdk' LIKE"));
        assert!(!where_clause.contains("raw->>'sdk' IS NULL"));
        let f6 = PlaceListFilters {
            sdk: Some("6".to_string()),
            ..Default::default()
        };
        assert!(build_where(&f6, false).0.contains("raw->>'sdk' IS NULL"));

        let f = PlaceListFilters {
            creator_address: Some("0xABC".to_string()),
            ..Default::default()
        };
        let (where_clause, binds) = build_where(&f, false);
        assert!(where_clause.contains("LOWER(creator_address) ="));
        match binds.last().unwrap() {
            Bind::Text(s) => assert_eq!(s, "0xabc"),
            _ => panic!("expected creator text bind"),
        }
    }

    #[test]
    fn most_active_orders_by_live_counts_and_degrades_to_the_stored_order() {
        let f = PlaceListFilters {
            order_by: PlaceOrderBy::MostActive,
            place_user_counts: vec![("10,20".to_string(), 42), ("0,0".to_string(), 7)],
            world_user_counts: vec![("MyWorld.DCL.eth".to_string(), 99)],
            ..Default::default()
        };
        let (prefix, binds) = build_live_user_count_order(&f, 5);
        assert!(
            prefix.contains("CASE base_position WHEN $5 THEN $6"),
            "{prefix}"
        );
        assert!(
            prefix.contains("CASE lower(world_name) WHEN $9 THEN $10"),
            "{prefix}"
        );
        assert!(prefix.contains("CASE WHEN world THEN"), "{prefix}");
        assert!(prefix.trim_end().ends_with("DESC,"), "{prefix}");
        assert_eq!(binds.len(), 6);
        match &binds[4] {
            Bind::Text(s) => assert_eq!(s, "myworld.dcl.eth"),
            _ => panic!("expected lower-cased world name bind"),
        }

        let f = PlaceListFilters {
            order_by: PlaceOrderBy::MostActive,
            ..Default::default()
        };
        let (prefix, binds) = build_live_user_count_order(&f, 1);
        assert!(prefix.is_empty() && binds.is_empty());
        assert_eq!(
            f.order_by.column(),
            format!("COALESCE({}, 0)", raw_int_sql("user_count"))
        );

        for ob in [
            PlaceOrderBy::LikeScore,
            PlaceOrderBy::UpdatedAt,
            PlaceOrderBy::CreatedAt,
            PlaceOrderBy::UserVisits,
        ] {
            let f = PlaceListFilters {
                order_by: ob,
                place_user_counts: vec![("1,1".to_string(), 5)],
                ..Default::default()
            };
            let (prefix, binds) = build_live_user_count_order(&f, 1);
            assert!(prefix.is_empty() && binds.is_empty(), "{ob:?}");
        }
    }

    #[test]
    fn destinations_order_leads_above_ranking_above_rank() {
        let f = PlaceListFilters {
            destinations_mode: true,
            ..Default::default()
        };
        assert_eq!(destinations_highlighted_prefix(&f), "highlighted DESC, ");
        assert_eq!(
            destinations_ranking_prefix(&f),
            format!("COALESCE({}, 0) DESC, ", raw_float8_sql("ranking"))
        );
        assert_eq!(
            destinations_highlighted_prefix(&PlaceListFilters::default()),
            ""
        );
        assert_eq!(
            destinations_ranking_prefix(&PlaceListFilters::default()),
            ""
        );

        let clause = build_order_by(
            &destinations_lead_prefix(&f, "(CASE WHEN true THEN 1 ELSE 0 END)::int DESC, "),
            destinations_ranking_prefix(&f),
            "ts_rank_cd(x, y, 32) DESC, ",
            PlaceOrderBy::LikeScore.column(),
            "DESC",
            order_tail(&f),
        );
        let p_live = clause.find("::int DESC").unwrap();
        let p_hi = clause.find("highlighted DESC").unwrap();
        let p_rk = clause.find("ranking").unwrap();
        let p_rank = clause.find("ts_rank_cd").unwrap();
        let p_col = clause.find("like_score").unwrap();
        assert!(
            p_hi < p_live && p_live < p_rk && p_rk < p_rank && p_rank < p_col,
            "{clause}"
        );
        assert!(clause.trim_end().ends_with("id ASC"), "{clause}");

        let plain = build_order_by(
            "",
            "",
            "",
            PlaceOrderBy::LikeScore.column(),
            "DESC",
            order_tail(&PlaceListFilters::default()),
        );
        assert!(
            plain.starts_with(&format!(
                "{} DESC NULLS LAST",
                PlaceOrderBy::LikeScore.column()
            )),
            "{plain}"
        );
        assert!(plain.ends_with("deployed_at DESC, id ASC"), "{plain}");
    }

    #[test]
    fn most_active_ranks_the_live_count_above_the_featured_shelf() {
        let live = "(CASE WHEN world THEN 0 ELSE 0 END)::int DESC, ";
        let destinations = |order_by| PlaceListFilters {
            destinations_mode: true,
            order_by,
            ..Default::default()
        };
        let lead = destinations_lead_prefix(&destinations(PlaceOrderBy::MostActive), live);
        assert!(
            lead.find("::int DESC").unwrap() < lead.find("highlighted DESC").unwrap(),
            "{lead}"
        );
        for ob in [
            PlaceOrderBy::LikeScore,
            PlaceOrderBy::UpdatedAt,
            PlaceOrderBy::CreatedAt,
            PlaceOrderBy::UserVisits,
        ] {
            let lead = destinations_lead_prefix(&destinations(ob), live);
            assert!(
                lead.find("highlighted DESC").unwrap() < lead.find("::int DESC").unwrap(),
                "{ob:?}: {lead}"
            );
            assert_eq!(
                destinations_lead_prefix(&destinations(ob), ""),
                "highlighted DESC, ",
                "{ob:?}"
            );
        }
        let places = PlaceListFilters {
            order_by: PlaceOrderBy::MostActive,
            ..Default::default()
        };
        assert_eq!(destinations_lead_prefix(&places, live), live);
    }

    #[test]
    fn each_branch_ends_on_the_tail_of_the_upstream_branch_it_mirrors() {
        let updated_at = format!(
            "COALESCE({}, deployed_at) DESC, id ASC",
            raw_timestamptz_sql("updated_at")
        );
        let destinations = || PlaceListFilters {
            destinations_mode: true,
            ..Default::default()
        };
        assert_eq!(
            order_tail(&PlaceListFilters {
                only_places: true,
                ..destinations()
            }),
            "deployed_at DESC, id ASC"
        );
        assert_eq!(
            order_tail(&PlaceListFilters {
                only_worlds: true,
                ..destinations()
            }),
            updated_at
        );
        assert_eq!(order_tail(&destinations()), updated_at);
        assert_eq!(
            order_tail(&PlaceListFilters {
                only_worlds: true,
                ..Default::default()
            }),
            updated_at
        );
        assert_eq!(
            order_tail(&PlaceListFilters::default()),
            "deployed_at DESC, id ASC"
        );
    }
}

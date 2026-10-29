use axum::extract::{Request, State};
use axum::http::{StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;

use super::SharedState;
use super::deployments::normalize_query_string;
use super::errors::{AppError, AppResult};
use super::queries::{self, PointerChangesQueryOptions};
use super::query_params::{
    camel_to_snake, parse_entity_type, parse_query_string, qs_get_array, qs_get_bool,
    qs_get_number, qs_get_string,
};
use super::wire_types::{HistoryPagination, PointerChangeDelta, PointerChangesResponse};

const VALID_SORTING_FIELDS: &[&str] = &["local_timestamp", "entity_timestamp"];

const VALID_SORTING_ORDERS: &[&str] = &["ASC", "DESC"];

/// Peers poll this continuously with identical queries; the 5 s deployments
/// memo (cleared on every commit) serves them one keyset scan per window
/// instead of one per poll.
pub async fn get_pointer_changes(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<impl IntoResponse> {
    let query_string = request.uri().query().unwrap_or("").to_string();
    let cache_key = format!("pc:{}", normalize_query_string(&query_string));
    let state_in_fetch = state.clone();
    let body = state
        .deployments_cache
        .get_or_fetch(cache_key, move || async move {
            fetch_pointer_changes(&state_in_fetch, &query_string).await
        })
        .await?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .unwrap())
}

fn parse_pointer_changes_options(query_string: &str) -> AppResult<PointerChangesQueryOptions> {
    let params = parse_query_string(query_string);

    let mut entity_types: Vec<String> = Vec::new();
    for raw in qs_get_array(&params, "entityType") {
        match parse_entity_type(&raw) {
            Some(canonical) => entity_types.push(canonical.to_string()),
            None => return Err(AppError::invalid("Found an unrecognized entity type")),
        }
    }
    let from = qs_get_number(&params, "from");
    let to = qs_get_number(&params, "to");
    let offset = qs_get_number(&params, "offset");
    let limit = qs_get_number(&params, "limit");
    let last_id = qs_get_string(&params, "lastId").map(|s| s.to_lowercase());
    let include_auth_chain = qs_get_bool(&params, "includeAuthChain").unwrap_or(false);

    let sorting_field = match qs_get_string(&params, "sortingField") {
        Some(sf) => {
            let snake = camel_to_snake(&sf);
            if !VALID_SORTING_FIELDS.contains(&snake.as_str()) {
                return Err(AppError::invalid("Found an unrecognized sort field param"));
            }
            Some(snake)
        }
        None => None,
    };

    let sorting_order = match qs_get_string(&params, "sortingOrder") {
        Some(so) => {
            if !VALID_SORTING_ORDERS.contains(&so.as_str()) {
                return Err(AppError::invalid("Found an unrecognized sort order param"));
            }
            Some(so)
        }
        None => None,
    };

    if offset.is_some_and(|off| off > 5000) {
        return Err(AppError::invalid(
            "Offset can't be higher than 5000. Please use the 'next' property for pagination.",
        ));
    }

    Ok(PointerChangesQueryOptions {
        entity_types,
        from,
        to,
        include_auth_chain,
        sorting_field,
        sorting_order,
        offset,
        limit,
        last_id,
    })
}

async fn fetch_pointer_changes(state: &SharedState, query_string: &str) -> AppResult<Bytes> {
    let options = parse_pointer_changes_options(query_string)?;
    let result = queries::get_pointer_changes(&state.pool, &options).await?;

    let deltas: Vec<PointerChangeDelta> = result
        .deltas
        .into_iter()
        .filter(|delta| !state.denylist.contains(&delta.entity_id))
        .collect();

    let mut next: Option<String> = None;
    if result.pagination.more_data
        && let Some(last) = deltas.last()
    {
        let mut qs: Vec<String> = Vec::new();
        for et in &options.entity_types {
            qs.push(format!("entityType={et}"));
        }
        if let Some(from) = options.from {
            qs.push(format!("from={from}"));
        }
        qs.push(format!("to={}", last.local_timestamp));
        qs.push(format!("limit={}", result.pagination.limit));
        qs.push(format!("lastId={}", last.entity_id));
        next = Some(format!("?{}", qs.join("&")));
    }

    let pagination = HistoryPagination {
        offset: result.pagination.offset,
        limit: result.pagination.limit,
        more_data: result.pagination.more_data,
        next,
        last_id: None,
    };

    serde_json::to_vec(&PointerChangesResponse {
        deltas,
        filters: result.filters,
        pagination,
    })
    .map(Bytes::from)
    .map_err(|e| AppError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::require_test_db;

    #[test]
    fn invalid_params_are_rejected_before_any_query() {
        let err = parse_pointer_changes_options("entityType=nope").unwrap_err();
        assert_eq!(err.to_string(), "Found an unrecognized entity type");
        assert_eq!(
            parse_pointer_changes_options("offset=5001")
                .unwrap_err()
                .to_string(),
            "Offset can't be higher than 5000. Please use the 'next' property for pagination."
        );
    }

    #[test]
    fn options_follow_the_query_string() {
        let options = parse_pointer_changes_options(
            "entityType=scenes&from=1&to=2&limit=10&includeAuthChain=true&lastId=BafyX",
        )
        .unwrap();
        assert_eq!(options.entity_types, vec!["scene"]);
        assert_eq!(
            (options.from, options.to, options.limit),
            (Some(1), Some(2), Some(10))
        );
        assert!(options.include_auth_chain);
        assert_eq!(options.last_id.as_deref(), Some("bafyx"));
    }

    async fn seed(pool: &sqlx::PgPool, entity_id: &str, local_ms: i64) {
        seed_at(pool, entity_id, local_ms as f64).await;
    }

    async fn seed_at(pool: &sqlx::PgPool, entity_id: &str, local_ms: f64) {
        sqlx::query(
            "INSERT INTO content.deployments
                (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp,
                 entity_pointers, local_timestamp, auth_chain)
             VALUES ('0xDEPLOYER', 'v3', 'scene', $1, NULL, to_timestamp($2 / 1000.0), ARRAY['0,0'],
                     to_timestamp($2 / 1000.0), '[{\"type\":\"SIGNER\",\"payload\":\"0xdeployer\"}]'::json)",
        )
        .bind(entity_id)
        .bind(local_ms)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn get(router: axum::Router, uri: &str) -> serde_json::Value {
        use tower::ServiceExt;
        let resp = router
            .oneshot(Request::get(uri).body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{uri}");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// One ingest flush stamps a whole batch with one `local_timestamp`, so
    /// pages regularly end inside a tie group; the cursor is
    /// `(to, lastId)` and the tie-break is `LOWER(entity_id)`. Walking the
    /// `next` links as the sync client does must visit every row once, in
    /// order, and every delta carries `authChain` with the flag off
    /// (metrics/pointer-changes-auth-chain.md).
    #[tokio::test]
    async fn next_links_walk_an_equal_timestamp_group_once_with_auth_chains() {
        use crate::tests::support::TestNode;
        require_test_db!(Some(node) = TestNode::with_db("pcpage").await);
        let pool = node.state.pool.clone();
        sqlx::query("DELETE FROM content.deployments WHERE entity_id LIKE 'test-pcpage-%'")
            .execute(&pool)
            .await
            .unwrap();
        const TS: i64 = 1_701_234_567_000;
        for id in [
            "test-pcpage-B",
            "test-pcpage-a",
            "test-pcpage-D",
            "test-pcpage-c",
            "test-pcpage-E",
        ] {
            seed(&pool, id, TS).await;
        }
        seed(&pool, "test-pcpage-earlier", TS - 1).await;

        let mut uri = format!("/pointer-changes?from={}&to={TS}&limit=2", TS - 1);
        let mut seen: Vec<(i64, String)> = Vec::new();
        let mut pages = 0;
        loop {
            let body = get(node.router(), &uri).await;
            pages += 1;
            assert_eq!(body["filters"]["includeAuthChain"], false);
            for delta in body["deltas"].as_array().unwrap() {
                let id = delta["entityId"].as_str().unwrap();
                if !id.starts_with("test-pcpage-") {
                    continue;
                }
                assert_eq!(
                    delta["authChain"][0]["type"], "SIGNER",
                    "{id}: authChain is emitted with the flag off"
                );
                seen.push((delta["localTimestamp"].as_i64().unwrap(), id.to_string()));
            }
            match body["pagination"]["next"].as_str() {
                Some(next) => {
                    assert!(body["pagination"]["moreData"].as_bool().unwrap());
                    assert!(next.starts_with(&format!("?from={}&to=", TS - 1)), "{next}");
                    assert!(!next.contains("includeAuthChain"), "{next}");
                    uri = format!("/pointer-changes{next}");
                }
                None => {
                    assert_eq!(body["pagination"]["moreData"], false);
                    break;
                }
            }
            assert!(pages < 10, "next links never terminated");
        }
        assert!(
            pages >= 3,
            "limit=2 over 6 rows needs at least 3 pages, got {pages}"
        );
        let expected: Vec<(i64, String)> = [
            "test-pcpage-E",
            "test-pcpage-D",
            "test-pcpage-c",
            "test-pcpage-B",
            "test-pcpage-a",
        ]
        .iter()
        .map(|id| (TS, id.to_string()))
        .chain(std::iter::once((TS - 1, "test-pcpage-earlier".to_string())))
        .collect();
        assert_eq!(seen, expected);

        sqlx::query("DELETE FROM content.deployments WHERE entity_id LIKE 'test-pcpage-%'")
            .execute(&pool)
            .await
            .unwrap();
    }

    /// Walks pages from `uri`, continuing each with `continue_after(last delta)` while the
    /// server reports more, and answers the `test-pcmicro-` ids in the order seen.
    async fn walk(
        router: axum::Router,
        mut uri: String,
        continue_after: impl Fn(&serde_json::Value, &serde_json::Value) -> String,
    ) -> Vec<String> {
        let mut seen = Vec::new();
        for _ in 0..10 {
            let body = get(router.clone(), &uri).await;
            let deltas = body["deltas"].as_array().unwrap();
            seen.extend(
                deltas
                    .iter()
                    .filter_map(|d| d["entityId"].as_str())
                    .filter(|id| id.starts_with("test-pcmicro-"))
                    .map(str::to_owned),
            );
            if body["pagination"]["moreData"] != true {
                return seen;
            }
            uri = continue_after(deltas.last().unwrap(), &body);
        }
        panic!("pages never ended; seen {seen:?}");
    }

    /// Rows carry microseconds (`now()`) while the wire carries whole milliseconds, and one
    /// flush stamps its whole batch with one `now()`, so pages end inside a group sharing a
    /// sub-millisecond timestamp. Ascending, the sync client continues with `from` + `lastId`
    /// (`sync::pointer_changes`); descending, readers follow `next`. Both walk the
    /// group once: the cursor is the `lastId` row's own timestamp, not the millisecond.
    #[tokio::test]
    async fn keyset_pages_walk_a_sub_millisecond_group_once_in_both_directions() {
        use crate::tests::support::TestNode;
        require_test_db!(Some(node) = TestNode::with_db("pcmicro").await);
        let pool = node.state.pool.clone();
        let wipe = "DELETE FROM content.deployments WHERE entity_id LIKE 'test-pcmicro-%'";
        sqlx::query(wipe).execute(&pool).await.unwrap();
        const TS: i64 = 1_701_234_568_000;
        for id in ["c", "A", "e", "b", "D"] {
            seed_at(&pool, &format!("test-pcmicro-{id}"), TS as f64 + 0.5).await;
        }
        seed(&pool, "test-pcmicro-later", TS + 2).await;
        let window = format!("from={TS}&to={}&limit=2", TS + 10);
        let ascending: Vec<&str> = vec!["A", "b", "c", "D", "e", "later"];

        let asc = walk(
            node.router(),
            format!("/pointer-changes?sortingOrder=ASC&sortingField=localTimestamp&{window}"),
            |last, _| {
                format!(
                    "/pointer-changes?sortingOrder=ASC&sortingField=localTimestamp&from={}&to={}&limit=2&lastId={}",
                    last["localTimestamp"],
                    TS + 10,
                    last["entityId"].as_str().unwrap()
                )
            },
        )
        .await;
        let expected: Vec<String> = ascending
            .iter()
            .map(|id| format!("test-pcmicro-{id}"))
            .collect();
        assert_eq!(asc, expected);

        let desc = walk(
            node.router(),
            format!("/pointer-changes?{window}"),
            |_, body| {
                format!(
                    "/pointer-changes{}",
                    body["pagination"]["next"].as_str().unwrap()
                )
            },
        )
        .await;
        let expected: Vec<String> = expected.into_iter().rev().collect();
        assert_eq!(desc, expected);

        sqlx::query(wipe).execute(&pool).await.unwrap();
    }
}

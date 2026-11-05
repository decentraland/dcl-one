use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use bytes::Bytes;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::queries::{self, DeploymentQueryOptions};
use super::query_params::{
    QueryParams, camel_to_snake, parse_entity_type, parse_query_string, qs_get_filter_array,
    qs_get_js_bool, qs_get_js_number, qs_get_string, to_ordered_query_string,
};
use super::wire_types::{ControllerDeployment, DeploymentsResponse, HistoryPagination};

const DEFAULT_FIELDS: &[&str] = &["pointers", "content", "metadata"];

const VALID_SORTING_FIELDS: &[&str] = &["local_timestamp", "entity_timestamp"];

const VALID_SORTING_ORDERS: &[&str] = &["ASC", "DESC"];

const VALID_DEPLOYMENT_FIELDS: &[&str] = &["pointers", "content", "metadata", "auditInfo"];

const MAX_DEPLOYMENT_FILTER_VALUES: usize = 1000;

pub(crate) fn normalize_query_string(qs: &str) -> String {
    let mut pairs: Vec<&str> = qs.split('&').filter(|s| !s.is_empty()).collect();
    pairs.sort_unstable();
    pairs.join("&")
}

/// Answers come out of the 5 s memo keyed by the normalized query string. The
/// memo is internal: as upstream, the response carries no `cache-control`.
pub async fn get_deployments(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<Response> {
    let query_string = request.uri().query().unwrap_or("").to_string();
    let cache_key = normalize_query_string(&query_string);
    let state_in_fetch = state.clone();

    let response_bytes = state
        .deployments_cache
        .get_or_fetch(cache_key, move || async move {
            fetch_deployments_response(&state_in_fetch, &query_string).await
        })
        .await?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(response_bytes))
        .unwrap())
}

async fn fetch_deployments_response(state: &SharedState, query_string: &str) -> AppResult<Bytes> {
    let params = parse_query_string(query_string);
    let options = parse_deployment_query_options(&params)?;

    let result = tokio::time::timeout(
        state.settings.deployments_query_timeout,
        queries::get_deployments(&state.pool, &options),
    )
    .await
    .map_err(|_| {
        AppError::ServiceUnavailable(
            "deployments query exceeded server-side time budget; narrow the time range or filters"
                .into(),
        )
    })??;

    // Upstream drops denylisted entities from the page after `moreData` is
    // computed, so a page can be short and still carry `next`. Inherited
    // quirk: a page that is entirely denylisted answers `moreData: true`
    // with no `next` (built from the page's last kept deployment), so a
    // client paging by `next` stops there (`logic/deployments/component.ts`
    // and `get-deployments-handler.ts` at `decentraland/catalyst` a416565).
    let deployments: Vec<ControllerDeployment> = result
        .deployments
        .into_iter()
        .filter(|dep| !state.denylist.contains(&dep.entity_id))
        .map(|dep| project_deployment_fields(dep, &options.fields))
        .collect();

    let next = if result.pagination.more_data && !deployments.is_empty() {
        Some(calculate_next_relative_path(
            &options,
            &deployments[deployments.len() - 1],
        ))
    } else {
        None
    };

    let pagination = HistoryPagination {
        offset: result.pagination.offset,
        limit: result.pagination.limit,
        more_data: result.pagination.more_data,
        next,
        last_id: result.pagination.last_id.clone(),
    };

    let response = DeploymentsResponse {
        deployments,
        filters: result.filters,
        pagination,
    };

    Ok(Bytes::from(
        serde_json::to_vec(&response).unwrap_or_default(),
    ))
}

/// Upstream's checks, in its order (`get-deployments-handler.ts:55-105`):
/// entity types, `offset`, the filter caps, then the sort field and order.
fn parse_deployment_query_options(params: &QueryParams) -> AppResult<DeploymentQueryOptions> {
    let mut entity_types: Vec<String> = Vec::new();
    for raw in qs_get_filter_array(params, "entityType") {
        match parse_entity_type(&raw) {
            Some(canonical) => entity_types.push(canonical.to_string()),
            None => return Err(AppError::invalid("Found an unrecognized entity type")),
        }
    }

    let entity_ids = qs_get_filter_array(params, "entityId");

    let pointers: Vec<String> = qs_get_filter_array(params, "pointer")
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect();

    let deployed_by: Vec<String> = qs_get_filter_array(params, "deployedBy")
        .into_iter()
        .map(|a| a.to_lowercase())
        .collect();

    let only_currently_pointed = qs_get_js_bool(params, "onlyCurrentlyPointed");
    let offset = qs_get_js_number(params, "offset");
    let limit = qs_get_js_number(params, "limit");
    let from = qs_get_js_number(params, "from");
    let to = qs_get_js_number(params, "to");
    let last_id = qs_get_string(params, "lastId").map(|s| s.to_lowercase());

    if offset.is_some_and(|off| off > 5000) {
        return Err(AppError::invalid(
            "Offset can't be higher than 5000. Please use the 'next' property for pagination.",
        ));
    }

    if entity_ids.len() > MAX_DEPLOYMENT_FILTER_VALUES
        || pointers.len() > MAX_DEPLOYMENT_FILTER_VALUES
        || entity_types.len() > MAX_DEPLOYMENT_FILTER_VALUES
    {
        return Err(AppError::invalid(format!(
            "Too many filter values; the maximum allowed per filter is {MAX_DEPLOYMENT_FILTER_VALUES}"
        )));
    }

    // A blank `fields` keeps the defaults; otherwise the listed names, kept
    // as written (upstream compares them untrimmed).
    let requested_fields: Option<Vec<String>> = match qs_get_string(params, "fields") {
        Some(f) if !f.trim().is_empty() => Some(
            f.split(',')
                .filter(|s| VALID_DEPLOYMENT_FIELDS.contains(s))
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    };
    let fields = requested_fields
        .clone()
        .unwrap_or_else(|| DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect());

    let (sorting_field, sorting_order) = parse_sort(params)?;

    Ok(DeploymentQueryOptions {
        entity_types,
        entity_ids,
        pointers,
        deployed_by,
        from,
        to,
        only_currently_pointed,
        fields,
        requested_fields,
        sorting_field,
        sorting_order,
        offset,
        limit,
        last_id,
    })
}

/// `sortingField` and `sortingOrder`, an empty value being absent as
/// upstream's `asEnumValue` reads it; shared with `/pointer-changes`.
pub(crate) fn parse_sort(params: &QueryParams) -> AppResult<(Option<String>, Option<String>)> {
    let sorting_field = match qs_get_string(params, "sortingField").filter(|s| !s.is_empty()) {
        Some(sf) => {
            let snake = camel_to_snake(&sf);
            if !VALID_SORTING_FIELDS.contains(&snake.as_str()) {
                return Err(AppError::invalid("Found an unrecognized sort field param"));
            }
            Some(snake)
        }
        None => None,
    };

    // A repeated `sortingOrder` is an array upstream, which `asEnumValue`
    // reads as unknown.
    if params.get("sortingOrder").is_some_and(|v| v.len() > 1) {
        return Err(AppError::invalid("Found an unrecognized sort order param"));
    }
    let sorting_order = match qs_get_string(params, "sortingOrder").filter(|s| !s.is_empty()) {
        Some(so) => {
            if !VALID_SORTING_ORDERS.contains(&so.as_str()) {
                return Err(AppError::invalid("Found an unrecognized sort order param"));
            }
            Some(so)
        }
        None => None,
    };
    Ok((sorting_field, sorting_order))
}

fn project_deployment_fields(
    mut dep: ControllerDeployment,
    fields: &[String],
) -> ControllerDeployment {
    if !fields.iter().any(|f| f == "pointers") {
        dep.pointers = None;
    }
    if !fields.iter().any(|f| f == "content") {
        dep.content = None;
    }
    if !fields.iter().any(|f| f == "metadata") {
        dep.metadata = None;
    }
    if !fields.iter().any(|f| f == "auditInfo") {
        dep.audit_info = None;
    }
    dep
}

/// Upstream's `calculateNextRelativePath`: `toQueryParams` over
/// `{pointers, entityTypes, entityIds, onlyCurrentlyPointed, from, to, fields,
/// sortingField, sortingOrder, lastId, limit}` in that key order, dropping
/// falsy values (`false`, `0`, `''`) and empty lists. `fields` is sent only
/// when the client listed some. `deployedBy` (this node's filter) follows
/// `entityId`.
fn calculate_next_relative_path(
    options: &DeploymentQueryOptions,
    last_deployment: &ControllerDeployment,
) -> String {
    let field = options
        .sorting_field
        .as_deref()
        .unwrap_or("local_timestamp");
    let order = options.sorting_order.as_deref().unwrap_or("DESC");

    let timestamp = if field == "local_timestamp" {
        last_deployment.local_timestamp
    } else {
        last_deployment.entity_timestamp
    };
    let (from, to) = if order == "ASC" {
        (Some(timestamp), options.to)
    } else {
        (options.from, Some(timestamp))
    };
    let number = |n: Option<i64>| n.filter(|n| *n != 0).map(|n| vec![n.to_string()]);
    let fields = options
        .requested_fields
        .as_ref()
        .map(|f| f.join(","))
        .filter(|f| !f.is_empty());

    let pairs: Vec<(&str, Vec<String>)> = [
        ("pointer", Some(options.pointers.clone())),
        ("entityType", Some(options.entity_types.clone())),
        ("entityId", Some(options.entity_ids.clone())),
        ("deployedBy", Some(options.deployed_by.clone())),
        (
            "onlyCurrentlyPointed",
            (options.only_currently_pointed == Some(true)).then(|| vec!["true".to_string()]),
        ),
        ("from", number(from)),
        ("to", number(to)),
        ("fields", fields.map(|f| vec![f])),
        ("sortingField", Some(vec![field.to_string()])),
        ("sortingOrder", Some(vec![order.to_string()])),
        (
            "lastId",
            (!last_deployment.entity_id.is_empty())
                .then(|| vec![last_deployment.entity_id.clone()]),
        ),
        ("limit", number(options.limit)),
    ]
    .into_iter()
    .filter_map(|(key, values)| values.filter(|v| !v.is_empty()).map(|v| (key, v)))
    .collect();

    format!("?{}", to_ordered_query_string(&pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_implicit_time_window_when_client_sends_no_time_filters() {
        let params = parse_query_string("entityType=emote&limit=3&sortingOrder=DESC");
        let options = parse_deployment_query_options(&params).unwrap();
        assert_eq!(options.from, None);
        assert_eq!(options.to, None);
    }

    #[test]
    fn no_implicit_time_window_on_empty_query() {
        let params = parse_query_string("");
        let options = parse_deployment_query_options(&params).unwrap();
        assert_eq!(options.from, None);
        assert_eq!(options.to, None);
        assert_eq!(options.fields, DEFAULT_FIELDS);
    }

    #[test]
    fn client_provided_time_filters_are_preserved() {
        let params = parse_query_string("from=1&to=2&limit=3");
        let options = parse_deployment_query_options(&params).unwrap();
        assert_eq!(options.from, Some(1));
        assert_eq!(options.to, Some(2));
        assert_eq!(options.limit, Some(3));
    }

    #[test]
    fn validation_errors_carry_the_upstream_messages() {
        let cases = [
            (
                "entityType=totallybogus",
                "Found an unrecognized entity type",
            ),
            (
                "sortingField=bogusField",
                "Found an unrecognized sort field param",
            ),
            (
                "sortingOrder=BOGUS",
                "Found an unrecognized sort order param",
            ),
            (
                "offset=5001",
                "Offset can't be higher than 5000. Please use the 'next' property for pagination.",
            ),
        ];
        for (query, message) in cases {
            let err = parse_deployment_query_options(&parse_query_string(query)).unwrap_err();
            assert_eq!(err.to_string(), message, "{query}");
        }
        assert_eq!(
            parse_deployment_query_options(&parse_query_string("sortingField=entityTimestamp"))
                .unwrap()
                .sorting_field
                .as_deref(),
            Some("entity_timestamp")
        );
    }

    #[test]
    fn next_page_follows_the_sort_direction() {
        let options = DeploymentQueryOptions {
            entity_types: vec!["scene".into()],
            limit: Some(10),
            sorting_order: Some("ASC".into()),
            to: Some(99),
            ..Default::default()
        };
        let last = ControllerDeployment {
            entity_version: "v3".into(),
            entity_type: "scene".into(),
            entity_id: "bafyLast".into(),
            entity_timestamp: 5,
            deployed_by: "0x0".into(),
            pointers: None,
            content: None,
            metadata: None,
            audit_info: None,
            local_timestamp: 7,
        };
        assert_eq!(
            calculate_next_relative_path(&options, &last),
            "?entityType=scene&from=7&to=99&sortingField=local_timestamp&sortingOrder=ASC&lastId=bafyLast&limit=10"
        );
        let projected = project_deployment_fields(last, &["content".to_string()]);
        assert!(projected.pointers.is_none() && projected.audit_info.is_none());
    }

    fn last_deployment() -> ControllerDeployment {
        ControllerDeployment {
            entity_version: "v3".into(),
            entity_type: "scene".into(),
            entity_id: "bafylast".into(),
            entity_timestamp: 5,
            deployed_by: "0x0".into(),
            pointers: None,
            content: None,
            metadata: None,
            audit_info: None,
            local_timestamp: 7,
        }
    }

    fn next_for(query: &str) -> String {
        let options = parse_deployment_query_options(&parse_query_string(query)).unwrap();
        calculate_next_relative_path(&options, &last_deployment())
    }

    /// CS-07: upstream's key order, `fields` only when the client listed
    /// some (even the default set), and falsy values dropped.
    #[test]
    fn next_keeps_upstreams_key_order_and_fields_rule() {
        assert_eq!(
            next_for(
                "limit=5&fields=pointers,content,metadata&onlyCurrentlyPointed=true\
                 &entityId=bafyx&entityType=scene&pointer=0,0&from=3"
            ),
            "?pointer=0%2C0&entityType=scene&entityId=bafyx&onlyCurrentlyPointed=true&from=3&to=7\
             &fields=pointers%2Ccontent%2Cmetadata&sortingField=local_timestamp&sortingOrder=DESC\
             &lastId=bafylast&limit=5"
        );
        assert_eq!(
            next_for("from=0&limit=0&fields=bogus&onlyCurrentlyPointed=false"),
            "?to=7&sortingField=local_timestamp&sortingOrder=DESC&lastId=bafylast"
        );
        assert_eq!(
            next_for("fields=%20"),
            "?to=7&sortingField=local_timestamp&sortingOrder=DESC&lastId=bafylast"
        );
        assert_eq!(
            next_for("sortingField=entityTimestamp&sortingOrder=ASC&to=9"),
            "?from=5&to=9&sortingField=entity_timestamp&sortingOrder=ASC&lastId=bafylast"
        );
    }

    /// CS-06 and CS-08: an empty sort value is absent; numbers and booleans
    /// read as `qsGetNumber`/`qsGetBoolean`; `fields` items are not trimmed.
    #[test]
    fn values_read_as_upstreams_query_params() {
        let options = parse_deployment_query_options(&parse_query_string(
            "sortingField=&sortingOrder=&limit=10abc&from=1.5&offset=&onlyCurrentlyPointed=\
             &fields=pointers,%20content",
        ))
        .unwrap();
        assert_eq!(options.sorting_field, None);
        assert_eq!(options.sorting_order, None);
        assert_eq!(options.limit, Some(10));
        assert_eq!(options.from, Some(1));
        assert_eq!(options.offset, None);
        assert_eq!(options.only_currently_pointed, None);
        assert_eq!(options.fields, vec!["pointers".to_string()]);
        let err = parse_deployment_query_options(&parse_query_string(
            "sortingOrder=ASC&sortingOrder=ASC",
        ))
        .unwrap_err();
        assert_eq!(err.to_string(), "Found an unrecognized sort order param");
    }

    /// CS-21: `offset` is checked right after the entity types, before the
    /// filter caps and the sort values.
    #[test]
    fn offset_is_checked_before_the_caps_and_the_sort() {
        let offset =
            "Offset can't be higher than 5000. Please use the 'next' property for pagination.";
        let pointers: String = (0..1001).map(|i| format!("&pointer={i}")).collect();
        for query in [
            "offset=5001&sortingField=bogus".to_string(),
            "offset=5001&sortingOrder=bogus".to_string(),
            format!("offset=5001{pointers}"),
        ] {
            let err = parse_deployment_query_options(&parse_query_string(&query)).unwrap_err();
            assert_eq!(err.to_string(), offset);
        }
        let err =
            parse_deployment_query_options(&parse_query_string("offset=5001&entityType=bogus"))
                .unwrap_err();
        assert_eq!(err.to_string(), "Found an unrecognized entity type");
    }

    #[test]
    fn cache_key_ignores_parameter_order() {
        assert_eq!(
            normalize_query_string("entityType=scene&limit=10"),
            normalize_query_string("limit=10&entityType=scene")
        );
    }
}

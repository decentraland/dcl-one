use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::Response;
use bytes::Bytes;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::queries::{self, DeploymentQueryOptions};
use super::query_params::{
    QueryParams, camel_to_snake, parse_entity_type, parse_query_string, qs_get_array, qs_get_bool,
    qs_get_number, qs_get_string, to_query_string,
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

/// Answers come out of the 5 s memo keyed by the normalized query string;
/// `x-cache` says whether this call ran the query.
pub async fn get_deployments(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<Response> {
    let query_string = request.uri().query().unwrap_or("").to_string();
    let cache_key = normalize_query_string(&query_string);

    let fetched = Arc::new(AtomicBool::new(false));
    let fetched_in_fetch = fetched.clone();
    let state_in_fetch = state.clone();

    let response_bytes = state
        .deployments_cache
        .get_or_fetch(cache_key, move || async move {
            fetched_in_fetch.store(true, Ordering::Relaxed);
            fetch_deployments_response(&state_in_fetch, &query_string).await
        })
        .await?;

    let cache_status = if fetched.load(Ordering::Relaxed) {
        "MISS"
    } else {
        "HIT"
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("cache-control", "max-age=5")
        .header("x-cache", cache_status)
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

    let deployments: Vec<ControllerDeployment> = result
        .deployments
        .into_iter()
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

fn parse_deployment_query_options(params: &QueryParams) -> AppResult<DeploymentQueryOptions> {
    let mut entity_types: Vec<String> = Vec::new();
    for raw in qs_get_array(params, "entityType") {
        match parse_entity_type(&raw) {
            Some(canonical) => entity_types.push(canonical.to_string()),
            None => return Err(AppError::invalid("Found an unrecognized entity type")),
        }
    }

    let entity_ids = qs_get_array(params, "entityId");

    let pointers: Vec<String> = qs_get_array(params, "pointer")
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect();

    let deployed_by: Vec<String> = qs_get_array(params, "deployedBy")
        .into_iter()
        .map(|a| a.to_lowercase())
        .collect();

    if entity_ids.len() > MAX_DEPLOYMENT_FILTER_VALUES
        || pointers.len() > MAX_DEPLOYMENT_FILTER_VALUES
        || entity_types.len() > MAX_DEPLOYMENT_FILTER_VALUES
    {
        return Err(AppError::invalid(format!(
            "Too many filter values; the maximum allowed per filter is {MAX_DEPLOYMENT_FILTER_VALUES}"
        )));
    }

    let only_currently_pointed = qs_get_bool(params, "onlyCurrentlyPointed");
    let offset = qs_get_number(params, "offset");
    let limit = qs_get_number(params, "limit");
    let from = qs_get_number(params, "from");
    let to = qs_get_number(params, "to");
    let last_id = qs_get_string(params, "lastId").map(|s| s.to_lowercase());

    let fields: Vec<String> = match qs_get_string(params, "fields") {
        Some(f) if !f.trim().is_empty() => f
            .split(',')
            .filter(|s| VALID_DEPLOYMENT_FIELDS.contains(&s.trim()))
            .map(|s| s.trim().to_string())
            .collect(),
        _ => DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect(),
    };

    let sorting_field = match qs_get_string(params, "sortingField") {
        Some(sf) => {
            let snake = camel_to_snake(&sf);
            if !VALID_SORTING_FIELDS.contains(&snake.as_str()) {
                return Err(AppError::invalid("Found an unrecognized sort field param"));
            }
            Some(snake)
        }
        None => None,
    };

    let sorting_order = match qs_get_string(params, "sortingOrder") {
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

    Ok(DeploymentQueryOptions {
        entity_types,
        entity_ids,
        pointers,
        deployed_by,
        from,
        to,
        only_currently_pointed,
        fields,
        sorting_field,
        sorting_order,
        offset,
        limit,
        last_id,
    })
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

    let timestamp_str = timestamp.to_string();
    let last_entity_id = last_deployment.entity_id.as_str();

    let mut next_params: HashMap<String, Vec<String>> = HashMap::new();

    if !options.entity_types.is_empty() {
        next_params.insert("entityType".to_string(), options.entity_types.clone());
    }
    if !options.entity_ids.is_empty() {
        next_params.insert("entityId".to_string(), options.entity_ids.clone());
    }
    if !options.pointers.is_empty() {
        next_params.insert("pointer".to_string(), options.pointers.clone());
    }
    if !options.deployed_by.is_empty() {
        next_params.insert("deployedBy".to_string(), options.deployed_by.clone());
    }
    if options.only_currently_pointed == Some(true) {
        next_params.insert("onlyCurrentlyPointed".to_string(), vec!["true".to_string()]);
    }

    if order == "ASC" {
        next_params.insert("from".to_string(), vec![timestamp_str]);
        if let Some(to_val) = options.to {
            next_params.insert("to".to_string(), vec![to_val.to_string()]);
        }
    } else {
        next_params.insert("to".to_string(), vec![timestamp_str]);
        if let Some(from_val) = options.from {
            next_params.insert("from".to_string(), vec![from_val.to_string()]);
        }
    }

    let is_default_fields = options.fields.len() == DEFAULT_FIELDS.len()
        && options
            .fields
            .iter()
            .all(|f| DEFAULT_FIELDS.contains(&f.as_str()));
    if !is_default_fields {
        let fields_str = options.fields.join(",");
        if !fields_str.is_empty() {
            next_params.insert("fields".to_string(), vec![fields_str]);
        }
    }

    next_params.insert("sortingField".to_string(), vec![field.to_string()]);
    next_params.insert("sortingOrder".to_string(), vec![order.to_string()]);

    if !last_entity_id.is_empty() {
        next_params.insert("lastId".to_string(), vec![last_entity_id.to_string()]);
    }

    if let Some(lim) = options.limit {
        next_params.insert("limit".to_string(), vec![lim.to_string()]);
    }

    format!("?{}", to_query_string(&next_params))
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
            "?entityType=scene&from=7&lastId=bafyLast&limit=10&sortingField=local_timestamp&sortingOrder=ASC&to=99"
        );
        let projected = project_deployment_fields(last, &["content".to_string()]);
        assert!(projected.pointers.is_none() && projected.audit_info.is_none());
    }

    #[test]
    fn cache_key_ignores_parameter_order() {
        assert_eq!(
            normalize_query_string("entityType=scene&limit=10"),
            normalize_query_string("limit=10&entityType=scene")
        );
    }
}

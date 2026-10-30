use axum::Json;
use axum::extract::{Path, RawQuery, Request, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde_json::Value;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::formatters::{EntityField, mask_entity};
use super::queries::{self, EntityDoc, MaskedColumns, retain_non_denylisted};
use super::query_params::{
    QueryParams, parse_deployable_entity_type, parse_pagination, parse_query_string, qs_get_array,
    qs_get_string,
};
use super::validation::{
    ACTIVE_ENTITIES_SELECTOR_MESSAGE, ENTITIES_SELECTOR_MESSAGE, MAX_IDS_OR_POINTERS,
    validate_ids_or_pointers,
};

#[derive(Debug, serde::Deserialize)]
pub struct ActiveEntitiesRequest {
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub pointers: Option<Vec<String>>,
}

pub(crate) fn json_docs_response(docs: &[EntityDoc]) -> Response {
    (
        [(CONTENT_TYPE, "application/json")],
        EntityDoc::json_array(docs),
    )
        .into_response()
}

pub(crate) fn entities_cache_control(max_age: u64) -> Option<String> {
    if max_age > 0 {
        Some(format!("public, max-age={}", max_age))
    } else {
        None
    }
}

fn with_cache_control(state: &SharedState, mut response: Response) -> Response {
    if let Some(cache_control) =
        entities_cache_control(state.settings.entities_cache_control_max_age)
        && let Ok(hv) = cache_control.parse()
    {
        response.headers_mut().insert("Cache-Control", hv);
    }
    response
}

async fn lookup(
    state: &SharedState,
    use_ids: bool,
    values: &[String],
) -> AppResult<Vec<EntityDoc>> {
    let mut docs = if let Some(read_through) = state.read_through() {
        read_through.active_docs(use_ids, values).await?
    } else if use_ids {
        queries::active_entity_docs_by_ids(&state.pool, values).await?
    } else {
        queries::active_entity_docs_by_pointers(&state.pool, values).await?
    };
    retain_non_denylisted(&mut docs, &state.denylist);
    Ok(docs)
}

fn requested_fields(params: &QueryParams) -> Option<Vec<EntityField>> {
    qs_get_string(params, "fields").map(|f| {
        f.split(',')
            .filter_map(|s| EntityField::parse(s.trim()))
            .collect()
    })
}

async fn lookup_masked(
    state: &SharedState,
    use_ids: bool,
    values: &[String],
    fields: &[EntityField],
) -> AppResult<Vec<Value>> {
    let columns = MaskedColumns {
        content: fields.contains(&EntityField::Content),
        metadata: fields.contains(&EntityField::Metadata),
    };
    let mut docs = if use_ids {
        queries::masked_entity_docs_by_ids(&state.pool, values, columns).await?
    } else {
        queries::masked_entity_docs_by_pointers(&state.pool, values, columns).await?
    };
    retain_non_denylisted(&mut docs, &state.denylist);
    Ok(docs
        .iter()
        .filter_map(EntityDoc::to_value)
        .map(|e| mask_entity(&e, Some(fields)))
        .collect())
}

/// POST /entities/active[?fields=content,pointers,metadata]: missing entities are silently
/// filtered, so an unknown id or pointer is `200 []`, never a 404.
pub async fn get_active_entities(
    State(state): State<SharedState>,
    RawQuery(query): RawQuery,
    Json(body): Json<ActiveEntitiesRequest>,
) -> AppResult<Response> {
    let use_ids = validate_ids_or_pointers(
        body.ids.as_deref(),
        body.pointers.as_deref(),
        MAX_IDS_OR_POINTERS,
    )
    .map_err(|e| e.into_app_error(ACTIVE_ENTITIES_SELECTOR_MESSAGE))?;
    let values = if use_ids { &body.ids } else { &body.pointers };
    let values = values.as_deref().unwrap_or_default();

    let response = match requested_fields(&parse_query_string(query.as_deref().unwrap_or(""))) {
        Some(fields) => Json(Value::Array(
            lookup_masked(&state, use_ids, values, &fields).await?,
        ))
        .into_response(),
        None => json_docs_response(&lookup(&state, use_ids, values).await?),
    };
    Ok(with_cache_control(&state, response))
}

/// GET /entities/{type}?pointer=..|id=..[&fields=content,pointers,metadata]
pub async fn get_entities(
    State(state): State<SharedState>,
    Path(entity_type): Path<String>,
    request: Request,
) -> AppResult<Response> {
    let params = parse_query_string(request.uri().query().unwrap_or(""));

    if parse_deployable_entity_type(&entity_type).is_none() {
        return Err(AppError::invalid(format!(
            "Unrecognized type: {entity_type}"
        )));
    }

    let pointers: Vec<String> = qs_get_array(&params, "pointer")
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect();
    let ids = qs_get_array(&params, "id");

    let use_ids = validate_ids_or_pointers(
        (!ids.is_empty()).then_some(ids.as_slice()),
        (!pointers.is_empty()).then_some(pointers.as_slice()),
        MAX_IDS_OR_POINTERS,
    )
    .map_err(|e| e.into_app_error(ENTITIES_SELECTOR_MESSAGE))?;

    let fields = requested_fields(&params);

    let entities = lookup(&state, use_ids, if use_ids { &ids } else { &pointers }).await?;

    let response = match fields {
        Some(fields) => {
            let masked: Vec<Value> = entities
                .iter()
                .filter_map(EntityDoc::to_value)
                .map(|e| mask_entity(&e, Some(&fields)))
                .collect();
            Json(Value::Array(masked)).into_response()
        }
        None => json_docs_response(&entities),
    };
    Ok(with_cache_control(&state, response))
}

/// GET /contents/{hash}/active-entities: the ids of the active entities that
/// reference a content file.
pub async fn get_active_entities_by_hash(
    State(state): State<SharedState>,
    Path(hash_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let mut entity_ids = queries::active_entity_ids_by_content_hash(&state.pool, &hash_id).await?;
    entity_ids.retain(|id| !state.denylist.contains(id));

    if entity_ids.is_empty() {
        return Err(AppError::not_found("The entity was not found"));
    }

    Ok(Json(entity_ids))
}

const KNOWN_NETWORKS: [&str; 7] = [
    "ethereum", "goerli", "sepolia", "matic", "mumbai", "amoy", "arbitrum",
];

fn invalid_urn_format_message(urn: &str) -> String {
    format!("Invalid URN format: {urn}")
}

fn invalid_collection_urn_param_message(urn: &str) -> String {
    format!(
        "Invalid collection urn param, it must be a valid urn prefix of a collection or a third party id, instead: '{urn}'"
    )
}

fn has_url_scheme(urn: &str) -> bool {
    let Some(colon) = urn.find(':') else {
        return false;
    };
    let mut scheme = urn[..colon].chars();
    match scheme.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    scheme.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
}

/// lamb2 answers two different sentences: a string that does not even parse
/// as a URN, and a URN that parses but is not a collection.
fn collection_urn_rejection_message(urn: &str) -> String {
    if has_url_scheme(urn) {
        invalid_collection_urn_param_message(urn)
    } else {
        invalid_urn_format_message(urn)
    }
}

fn is_hex_contract(value: &str) -> bool {
    value.len() > 2 && value.starts_with("0x") && value[2..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_known_network(network: &str) -> bool {
    KNOWN_NETWORKS
        .iter()
        .any(|n| network.eq_ignore_ascii_case(n))
}

fn is_valid_collection_urn(urn: &str) -> bool {
    let p: Vec<&str> = urn.split(':').collect();
    if p.len() != 5 || !p[0].eq_ignore_ascii_case("urn") || p[1] != "decentraland" {
        return false;
    }

    if p[2] == "off-chain" {
        return matches!(p[3], "base-avatars" | "base-emotes") && !p[4].is_empty();
    }

    if !is_known_network(p[2]) {
        return false;
    }

    match p[3] {
        "collections-v1" => is_hex_contract(p[4]) || (p[2] == "ethereum" && !p[4].is_empty()),
        "collections-v2" => is_hex_contract(p[4]),
        "collections-thirdparty" => !p[4].is_empty(),
        _ => false,
    }
}

/// GET /entities/active/collections/{urn}?pageSize=&pageNum=
pub async fn get_entities_by_collection(
    State(state): State<SharedState>,
    Path(collection_urn): Path<String>,
    request: Request,
) -> AppResult<impl IntoResponse> {
    if !is_valid_collection_urn(&collection_urn) {
        return Err(AppError::invalid(collection_urn_rejection_message(
            &collection_urn,
        )));
    }

    let params = parse_query_string(request.uri().query().unwrap_or(""));
    let pagination = parse_pagination(&params, 1000).map_err(AppError::invalid)?;

    let result = queries::active_entity_docs_by_prefix(
        &state.pool,
        &collection_urn,
        pagination.offset,
        pagination.limit,
    )
    .await?;

    let mut entities = result.entities;
    retain_non_denylisted(&mut entities, &state.denylist);

    let mut body = format!("{{\"total\":{},\"entities\":", result.total).into_bytes();
    body.extend_from_slice(&EntityDoc::json_array(&entities));
    body.push(b'}');
    Ok(([(CONTENT_TYPE, "application/json")], Bytes::from(body)))
}

#[cfg(test)]
mod mask_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn selector_use_ids(query_string: &str) -> Result<bool, AppError> {
        let params = parse_query_string(query_string);
        let pointers: Vec<String> = qs_get_array(&params, "pointer")
            .into_iter()
            .map(|p| p.to_lowercase())
            .collect();
        let ids = qs_get_array(&params, "id");
        validate_ids_or_pointers(
            (!ids.is_empty()).then_some(ids.as_slice()),
            (!pointers.is_empty()).then_some(pointers.as_slice()),
            MAX_IDS_OR_POINTERS,
        )
        .map_err(|e| e.into_app_error(ENTITIES_SELECTOR_MESSAGE))
    }

    #[test]
    fn empty_pointer_query_element_is_rejected() {
        let err = selector_use_ids("pointer=").unwrap_err();
        assert_eq!(err.to_string(), ENTITIES_SELECTOR_MESSAGE);
    }

    #[test]
    fn valid_pointer_query_selects_pointers() {
        assert!(!selector_use_ids("pointer=0,0").unwrap());
    }

    #[test]
    fn valid_id_query_selects_ids() {
        assert!(selector_use_ids("id=bafkreitest").unwrap());
    }

    #[test]
    fn cache_control_disabled_at_zero_otherwise_sets_max_age() {
        assert_eq!(entities_cache_control(0), None);
        assert_eq!(
            entities_cache_control(10).as_deref(),
            Some("public, max-age=10")
        );
    }

    fn rejection_message(urn: &str) -> String {
        assert!(!is_valid_collection_urn(urn));
        collection_urn_rejection_message(urn)
    }

    #[test]
    fn invalid_collection_urn_message_matches_upstream_text() {
        assert_eq!(
            invalid_urn_format_message("not-a-real-urn"),
            "Invalid URN format: not-a-real-urn"
        );
    }

    #[test]
    fn invalid_collection_urn_param_message_matches_upstream_text() {
        assert_eq!(
            invalid_collection_urn_param_message("urn:decentraland:ethereum:LAND:0,0"),
            "Invalid collection urn param, it must be a valid urn prefix of a collection or a third party id, instead: 'urn:decentraland:ethereum:LAND:0,0'"
        );
    }

    #[test]
    fn unparseable_urns_report_the_invalid_urn_format_message() {
        for urn in ["not-a-real-urn", "0xdeadbeef"] {
            assert!(!has_url_scheme(urn));
            assert_eq!(rejection_message(urn), invalid_urn_format_message(urn));
        }
    }

    #[test]
    fn parseable_but_unsupported_urns_report_the_collection_urn_param_message() {
        for urn in [
            "urn:decentraland:off-chain:base-scene-emotes:sittingChair2",
            "urn:decentraland:off-chain:other-collection:wave",
            "urn:decentraland:ethereum:LAND:0,0",
            "https://example.com/x",
        ] {
            assert!(has_url_scheme(urn));
            assert_eq!(
                rejection_message(urn),
                invalid_collection_urn_param_message(urn)
            );
        }
    }

    #[test]
    fn only_base_avatars_and_base_emotes_are_listable_off_chain_collections() {
        assert!(is_valid_collection_urn(
            "urn:decentraland:off-chain:base-avatars:eyes_00"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:off-chain:base-emotes:wave"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:off-chain:base-emotes:"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:off-chain:base-emotes:wave:extra"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:off-chain:base-scene-emotes:sittingChair2"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:off-chain:other-collection:wave"
        ));
    }

    #[test]
    fn collection_urns_on_chain_are_validated_by_kind() {
        assert!(is_valid_collection_urn(
            "urn:decentraland:ethereum:collections-v1:halloween_2019"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:ethereum:collections-v1:0x06012c8cf97bead5deae237070f9587f8e7a266d"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:matic:collections-v2:0x1c1b7a6ca21d3ab7a0a9f0c2f1a6ce9c9b9e0a1b"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:matic:collections-v2:0xabc"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:arbitrum:collections-v2:0x1c1b7a6ca21d3ab7a0a9f0c2f1a6ce9c9b9e0a1b"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:matic:collections-thirdparty:crypto-motors"
        ));
        assert!(is_valid_collection_urn(
            "urn:decentraland:mumbai:collections-thirdparty:aThirdParty"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:mumbai:collections-thirdparty:aThirdParty:winterCollection"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:matic:collections-v2:not-an-address"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:matic:collections-v2:0x1c1b7a6ca21d3ab7a0a9f0c2f1a6ce9c9b9e0a1b:0"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:matic:collections-v1:halloween_2019"
        ));
        assert!(is_valid_collection_urn(
            "URN:decentraland:off-chain:base-emotes:wave"
        ));
        assert!(!is_valid_collection_urn(
            "urn:DECENTRALAND:off-chain:base-emotes:wave"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:ETHEREUM:collections-v1:halloween_2019"
        ));
        assert!(!is_valid_collection_urn(
            "urn:decentraland:bogusnet:collections-v2:0x1c1b7a6ca21d3ab7a0a9f0c2f1a6ce9c9b9e0a1b"
        ));
    }

    #[test]
    fn not_a_real_urn_is_rejected() {
        assert!(!is_valid_collection_urn("not-a-real-urn"));
    }

    #[tokio::test]
    async fn invalid_collection_urn_response_is_400_with_envelope_body() {
        let err = AppError::invalid(rejection_message("not-a-real-urn"));
        let resp = err.into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "error": "Invalid URN format: not-a-real-urn" })
        );
    }
}

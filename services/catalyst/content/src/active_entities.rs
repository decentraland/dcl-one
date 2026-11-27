use axum::Json;
use axum::extract::{Path, RawQuery, Request, State};
use axum::http::header::{ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL, CONTENT_TYPE, ETAG};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use sha2::{Digest, Sha256};

use super::SharedState;
use super::errors::{AppError, AppResult, json_error};
use super::formatters::{EntityField, MaskedEntity, if_none_match_matches, mask_entity, to_etag};
use super::queries::{self, EntityDoc, MaskedColumns, retain_non_denylisted};
use super::query_params::{
    QueryParams, parse_content_pagination, parse_entity_type, parse_query_string,
    qs_get_filter_array, qs_get_string,
};
use super::read_through::active_entity_docs;
use super::validation::{
    BodyRefusal, ENTITIES_SELECTOR_MESSAGE, MAX_IDS_OR_POINTERS, active_entities_body,
    ordered_refusal_json, too_many_ids_or_pointers,
};
use super::validator::urn::{self, UrnType};

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
    Ok(active_entity_docs(
        &state.pool,
        state.read_through().map(|r| r.as_ref()),
        use_ids,
        values,
        Some(&state.denylist),
    )
    .await?)
}

/// `fields=` (empty) is no mask, as upstream's `if (fields)` reads it.
fn requested_fields(params: &QueryParams) -> Option<Vec<EntityField>> {
    qs_get_string(params, "fields")
        .filter(|f| !f.is_empty())
        .map(|f| {
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
) -> AppResult<Vec<MaskedEntity>> {
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
/// filtered, so an unknown id or pointer is `200 []`, never a 404. The body is
/// read as JSON whatever its `Content-Type`.
pub async fn get_active_entities(
    State(state): State<SharedState>,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> AppResult<Response> {
    let (use_ids, values) = match active_entities_body(&body) {
        Ok(selector) => selector,
        Err(BodyRefusal::Schema(refusal)) => {
            return Ok((
                StatusCode::BAD_REQUEST,
                [(CONTENT_TYPE, "application/json")],
                ordered_refusal_json(&refusal),
            )
                .into_response());
        }
        // Upstream's bare 500, without the error log `AppError::Internal`
        // writes: any client can send a NUL.
        Err(BodyRefusal::NulElement) => {
            tracing::debug!("POST /entities/active with a NUL byte in an element");
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
            ));
        }
    };
    let values = values.as_slice();

    // No `Cache-Control`: upstream sets none on this POST
    // (`captures/content-reads/CR-14-active-entity-key-order.json`).
    Ok(
        match requested_fields(&parse_query_string(query.as_deref().unwrap_or(""))) {
            Some(fields) => {
                Json(lookup_masked(&state, use_ids, values, &fields).await?).into_response()
            }
            None => json_docs_response(&lookup(&state, use_ids, values).await?),
        },
    )
}

/// GET /entities/active/scenes: every active scene as `{id, pointers,
/// timestamp, base}` ([`queries::ActiveScene`]), so a client maps the whole
/// city in one request instead of paging `POST /entities/active` by pointer.
/// Not an upstream route: decentraland/catalyst#1829 proposed it without
/// `base`, which a client needs to place a scene before reading its entity.
/// It lists what this node holds; read-through peers are not consulted.
/// The answer is memoized with the history routes (5 s, cleared on every
/// commit), denylisted scenes dropped before it is, as `/deployments` does.
/// Its `ETag` is the SHA-256 of the body, the same on every node holding the
/// same scenes (they are ordered by id); a request naming it in
/// `If-None-Match` is answered `304 Not Modified`. `no-cache` lets any cache
/// keep the list but makes it ask again before reusing it. A node that may
/// miss scenes answers 503 `no-store`: a partial list would read as empty land.
pub async fn get_active_scenes(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if !state.holds_every_scene() {
        let mut response =
            AppError::ServiceUnavailable("This node does not hold every scene".to_string())
                .into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return Ok(response);
    }
    let memo = state
        .deployments_cache
        .get_or_fetch("active-scenes".to_string(), || async {
            let mut scenes = queries::active_scenes(&state.pool).await?;
            scenes.retain(|scene| queries::non_denylisted(&scene.id, &state.denylist));
            let body =
                serde_json::to_vec(&scenes).map_err(|e| AppError::Internal(e.to_string()))?;
            // the tag rides in front of the body, so the memo keeps one value per entry
            let mut memo = to_etag(&hex::encode(Sha256::digest(&body))).into_bytes();
            memo.extend_from_slice(&body);
            Ok::<_, AppError>(Bytes::from(memo))
        })
        .await?;
    let (etag, body) = split_active_scenes_memo(&memo);
    let etag = std::str::from_utf8(&etag).map_err(|e| AppError::Internal(e.to_string()))?;
    let cache_headers = [
        (ETAG, etag),
        (CACHE_CONTROL, "no-cache"),
        (ACCESS_CONTROL_EXPOSE_HEADERS, "ETag"),
    ];
    Ok(if if_none_match_matches(&headers, etag) {
        (StatusCode::NOT_MODIFIED, cache_headers).into_response()
    } else {
        (cache_headers, [(CONTENT_TYPE, "application/json")], body).into_response()
    })
}

/// The quoted SHA-256 tag (66 bytes) and the body of an `active-scenes` memo entry.
fn split_active_scenes_memo(memo: &Bytes) -> (Bytes, Bytes) {
    const TAG_LEN: usize = 64 + 2;
    (memo.slice(..TAG_LEN), memo.slice(TAG_LEN..))
}

/// GET /entities/{type}?pointer=..|id=..[&fields=content,pointers,metadata]
pub async fn get_entities(
    State(state): State<SharedState>,
    Path(entity_type): Path<String>,
    request: Request,
) -> AppResult<Response> {
    entities_of_type(&state, &entity_type, request.uri().query()).await
}

/// GET /entities/active: upstream's `/entities/:type` matches it too (the
/// static route is POST only), so it is that handler with the type `active`,
/// which it refuses as an unrecognized type.
pub async fn get_entities_active(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<Response> {
    entities_of_type(&state, "active", request.uri().query()).await
}

async fn entities_of_type(
    state: &SharedState,
    entity_type: &str,
    query: Option<&str>,
) -> AppResult<Response> {
    let params = parse_query_string(query.unwrap_or(""));

    if parse_entity_type(entity_type).is_none() {
        return Err(AppError::invalid(format!(
            "Unrecognized type: {entity_type}"
        )));
    }

    let pointers: Vec<String> = qs_get_filter_array(&params, "pointer")
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect();
    let ids = qs_get_filter_array(&params, "id");

    let use_ids = entities_selector(&ids, &pointers)?;

    let fields = requested_fields(&params);

    // An empty element inside a list is kept by upstream and matches nothing.
    let values: Vec<String> = if use_ids { ids } else { pointers }
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect();
    let entities = if values.is_empty() {
        Vec::new()
    } else {
        lookup(state, use_ids, &values).await?
    };

    // Upstream masks every entity here, `fields` or not, so the key order is
    // its `maskEntity` one (`version,id,type,timestamp,...`), not the stored one.
    let masked: Vec<MaskedEntity> = entities
        .iter()
        .filter_map(EntityDoc::to_value)
        .map(|e| mask_entity(&e, fields.as_deref()))
        .collect();
    Ok(with_cache_control(state, Json(masked).into_response()))
}

/// Upstream's `GET /entities/:type` selector check: `id` or `pointer`, not
/// both, each read by `qsGetArray` (a lone empty value is absent, an empty
/// element in a list is kept); returns whether ids were selected.
fn entities_selector(ids: &[String], pointers: &[String]) -> AppResult<bool> {
    if ids.is_empty() == pointers.is_empty() {
        return Err(AppError::invalid(ENTITIES_SELECTOR_MESSAGE));
    }
    if ids.len() > MAX_IDS_OR_POINTERS || pointers.len() > MAX_IDS_OR_POINTERS {
        return Err(too_many_ids_or_pointers());
    }
    Ok(!ids.is_empty())
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

fn invalid_urn_format_message(urn: &str) -> String {
    format!("Invalid URN format: {urn}")
}

fn invalid_collection_urn_param_message(urn: &str) -> String {
    format!(
        "Invalid collection urn param, it must be a valid urn prefix of a collection or a third party id, instead: '{urn}'"
    )
}

/// Upstream's check of a collection URN: `parseUrn` throwing answers one
/// sentence, and a URN it reads but that is not a listable collection (v1,
/// v2, a third party by name, or the base avatars or emotes) the other.
fn check_collection_urn(input: &str) -> Result<(), String> {
    let parsed = urn::parse_urn(input).map_err(|urn::Throws| invalid_urn_format_message(input))?;
    let listable = parsed.is_some_and(|parsed| {
        matches!(
            parsed.kind,
            UrnType::CollectionV1 | UrnType::CollectionV2 | UrnType::ThirdPartyName
        ) || matches!(
            parsed.off_chain_registry(),
            Some("base-avatars" | "base-emotes")
        )
    });
    if listable {
        Ok(())
    } else {
        Err(invalid_collection_urn_param_message(input))
    }
}

/// GET /entities/active/collections/{urn}?pageSize=&pageNum=
pub async fn get_entities_by_collection(
    State(state): State<SharedState>,
    Path(collection_urn): Path<String>,
    request: Request,
) -> AppResult<impl IntoResponse> {
    check_collection_urn(&collection_urn).map_err(AppError::invalid)?;

    let params = parse_query_string(request.uri().query().unwrap_or(""));
    let pagination = parse_content_pagination(&params, 1000).map_err(AppError::invalid)?;

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
mod scenes_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn selector_use_ids(query_string: &str) -> Result<bool, AppError> {
        let params = parse_query_string(query_string);
        let pointers: Vec<String> = qs_get_filter_array(&params, "pointer")
            .into_iter()
            .map(|p| p.to_lowercase())
            .collect();
        let ids = qs_get_filter_array(&params, "id");
        entities_selector(&ids, &pointers)
    }

    #[test]
    fn a_lone_empty_pointer_is_absent() {
        for query in ["pointer=", "id=", "pointer=&id=", ""] {
            let err = selector_use_ids(query).unwrap_err();
            assert_eq!(err.to_string(), ENTITIES_SELECTOR_MESSAGE, "{query}");
        }
        let err = selector_use_ids("pointer=0,0&id=a").unwrap_err();
        assert_eq!(err.to_string(), ENTITIES_SELECTOR_MESSAGE);
    }

    /// CR-9: `qsGetArray` keeps an empty element inside a list, so
    /// `pointer=a&pointer=` selects pointers (the empty one matches nothing).
    #[test]
    fn an_empty_element_inside_a_list_is_kept() {
        assert!(!selector_use_ids("pointer=0,0&pointer=").unwrap());
        assert!(selector_use_ids("id=a&id=").unwrap());
        assert!(!selector_use_ids("pointer=&pointer=").unwrap());
        // A lone empty `id` is absent, so the pointers select.
        assert!(!selector_use_ids("pointer=0,0&id=").unwrap());
    }

    #[test]
    fn an_empty_fields_value_is_no_mask() {
        assert_eq!(requested_fields(&parse_query_string("fields=")), None);
        assert_eq!(requested_fields(&parse_query_string("")), None);
        assert_eq!(
            requested_fields(&parse_query_string("fields=Content,")),
            Some(vec![EntityField::Content])
        );
        assert_eq!(
            requested_fields(&parse_query_string("fields=bogus")),
            Some(vec![])
        );
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
        check_collection_urn(urn).unwrap_err()
    }

    const C: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
    const CHECKSUMMED: &str = "0xF87E31492Faf9A91B02Ee0dEAAd50d51d56D5d4D";

    #[test]
    fn the_collections_upstream_lists_are_accepted_as_spelled() {
        for urn in [
            format!("urn:decentraland:ethereum:collections-v1:{C}"),
            format!("urn:decentraland:ethereum:collections-v1:{CHECKSUMMED}"),
            "urn:decentraland:ethereum:collections-v1:halloween_2019".into(),
            "urn:decentraland:ethereum:collections-v1:halloween%5F2019".into(),
            format!("urn:decentraland:matic:collections-v2:{C}"),
            format!("urn:decentraland:matic:collections-v2:{CHECKSUMMED}"),
            format!("urn:decentraland:MATIC:collections-v2:{C}"),
            format!("URN:decentraland:matic:collections-v2:{C}"),
            format!("urn:decentraland:arbitrum:collections-v2:{C}"),
            "urn:decentraland:matic:collections-thirdparty:crypto-motors".into(),
            "urn:decentraland:amoy:collections-thirdparty:aThirdParty".into(),
            "urn:decentraland:off-chain:base-avatars:eyes_00".into(),
            "urn:decentraland:off-chain:base-emotes:wave".into(),
            "urn:decentraland:off-chain:base%2Demotes:wave".into(),
            "dcl://base-avatars/eyes_00".into(),
        ] {
            assert_eq!(check_collection_urn(&urn), Ok(()), "{urn}");
        }
    }

    #[test]
    fn urns_upstream_refuses_are_refused_with_its_sentence() {
        for urn in [
            "urn:DECENTRALAND:off-chain:base-emotes:wave".to_string(),
            "urn:decentraland:off-chain:Base-Emotes:wave".into(),
            "urn:decentraland:off-chain:base-scene-emotes:sittingChair2".into(),
            "urn:decentraland:off-chain:other-collection:wave".into(),
            "urn:decentraland:off-chain:base-emotes:".into(),
            "urn:decentraland:off-chain:base-emotes:wave:extra".into(),
            format!("urn:decentraland:matic:Collections-V2:{C}"),
            format!("urn:decentraland:matic:collections-v2:0X{}", &C[2..]),
            "urn:decentraland:matic:collections-v2:not-an-address".into(),
            format!("urn:decentraland:matic:collections-v2:{C}:0"),
            format!("urn:decentraland:matic:collections-v2:{C}:0:1"),
            format!("urn:decentraland:bogusnet:collections-v2:{C}"),
            "urn:decentraland:ethereum:collections-v1:not_a_v1_collection".into(),
            "urn:decentraland:ETHEREUM:collections-v1:halloween_2019".into(),
            "urn:decentraland:matic:collections-v1:halloween_2019".into(),
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat".into(),
            "urn:decentraland:ethereum:collections-thirdparty:crypto-motors".into(),
            "urn:decentraland:amoy:collections-thirdparty:aThirdParty:winterCollection".into(),
            "urn:decentraland:ethereum:LAND:0,0".into(),
            "urn:decentraland:off-chain:base-emotes:%ZZ:extra".into(),
            "https://example.com/x".into(),
        ] {
            assert_eq!(
                rejection_message(&urn),
                invalid_collection_urn_param_message(&urn)
            );
        }
        for urn in [
            "not-a-real-urn",
            "0xdeadbeef",
            "",
            "1urn:decentraland",
            "urn:decentraland:off-chain:base-emotes:%ZZ",
            "urn:decentraland:%ZZ:collections-v2:0x1",
        ] {
            assert_eq!(rejection_message(urn), invalid_urn_format_message(urn));
        }
    }

    #[test]
    fn the_rejection_sentences_match_upstream_text() {
        assert_eq!(
            invalid_urn_format_message("not-a-real-urn"),
            "Invalid URN format: not-a-real-urn"
        );
        assert_eq!(
            invalid_collection_urn_param_message("urn:decentraland:ethereum:LAND:0,0"),
            "Invalid collection urn param, it must be a valid urn prefix of a collection or a third party id, instead: 'urn:decentraland:ethereum:LAND:0,0'"
        );
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

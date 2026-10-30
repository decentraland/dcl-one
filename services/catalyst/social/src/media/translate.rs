//! `POST /translate`: LibreTranslate's request and response shapes, single
//! (`q: string`) or batch (`q: string[]`), the char/batch limits, the cache
//! probe for every text, one engine call for the distinct misses under the
//! request deadline, and the answer in input order.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::backend::TranslatedItem;
use super::{ApiError, JsonBody, MediaState, SharedState, cache};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TranslateRequest {
    Single(SingleReq),
    Batch(BatchReq),
}

#[derive(Debug, Deserialize)]
pub struct SingleReq {
    pub q: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub target: String,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchReq {
    pub q: Vec<String>,
    #[serde(default = "default_source")]
    pub source: String,
    pub target: String,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_source() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "text".to_string()
}

fn resolve_language(detected: String, requested_source: &str) -> String {
    let detected = detected.trim();
    if !detected.is_empty() && !detected.eq_ignore_ascii_case("auto") {
        return detected.to_string();
    }
    let requested = requested_source.trim();
    if !requested.is_empty() && !requested.eq_ignore_ascii_case("auto") {
        return requested.to_string();
    }
    "en".to_string()
}

fn enforce_limits(char_limit: usize, batch_limit: usize, texts: &[String]) -> Result<(), ApiError> {
    if texts.len() > batch_limit {
        return Err(ApiError::bad_request(format!(
            "batch size ({}) exceeds limit ({batch_limit})",
            texts.len()
        )));
    }
    let total_chars: usize = texts.iter().map(|t| t.chars().count()).sum();
    if total_chars > char_limit {
        return Err(ApiError::bad_request(format!(
            "request ({total_chars} characters) exceeds text limit ({char_limit})"
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DetectedLanguageDto {
    pub confidence: f32,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct SingleResponse {
    #[serde(rename = "detectedLanguage")]
    pub detected_language: DetectedLanguageDto,
    #[serde(rename = "translatedText")]
    pub translated_text: String,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    #[serde(rename = "detectedLanguage")]
    pub detected_language: Vec<DetectedLanguageDto>,
    #[serde(rename = "translatedText")]
    pub translated_text: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum TranslateResponse {
    Single(SingleResponse),
    Batch(BatchResponse),
}

pub async fn translate(
    State(state): State<SharedState>,
    JsonBody(req): JsonBody<TranslateRequest>,
) -> Result<Json<TranslateResponse>, ApiError> {
    if state.backend.is_disabled() {
        return Err(ApiError::service_unavailable(
            "translation backend not configured",
        ));
    }
    match req {
        TranslateRequest::Single(s) => {
            enforce_limits(
                state.settings.char_limit,
                state.settings.batch_limit,
                std::slice::from_ref(&s.q),
            )?;
            let items = run(&state, &[s.q], &s.source, &s.target, &s.format).await?;
            let item = items
                .into_iter()
                .next()
                .ok_or_else(|| ApiError::Internal("backend returned no result".into()))?;
            Ok(Json(TranslateResponse::Single(SingleResponse {
                detected_language: DetectedLanguageDto {
                    confidence: item.detected_confidence,
                    language: resolve_language(item.detected_language, &s.source),
                },
                translated_text: item.translated_text,
            })))
        }
        TranslateRequest::Batch(b) => {
            if b.q.is_empty() {
                return Ok(Json(TranslateResponse::Batch(BatchResponse {
                    detected_language: Vec::new(),
                    translated_text: Vec::new(),
                })));
            }
            enforce_limits(state.settings.char_limit, state.settings.batch_limit, &b.q)?;
            let items = run(&state, &b.q, &b.source, &b.target, &b.format).await?;
            if items.len() != b.q.len() {
                return Err(ApiError::Internal(format!(
                    "batch size mismatch: got {} for {} inputs",
                    items.len(),
                    b.q.len()
                )));
            }
            let mut detected_language = Vec::with_capacity(items.len());
            let mut translated_text = Vec::with_capacity(items.len());
            for item in items {
                detected_language.push(DetectedLanguageDto {
                    confidence: item.detected_confidence,
                    language: resolve_language(item.detected_language, &b.source),
                });
                translated_text.push(item.translated_text);
            }
            Ok(Json(TranslateResponse::Batch(BatchResponse {
                detected_language,
                translated_text,
            })))
        }
    }
}

async fn run(
    state: &MediaState,
    texts: &[String],
    source: &str,
    target: &str,
    format: &str,
) -> Result<Vec<TranslatedItem>, ApiError> {
    let target = target.to_lowercase();
    let backend_label = state.backend_label();

    let hashes: Vec<Vec<u8>> = texts.iter().map(|t| cache::hash_text(t)).collect();

    let cached = match cache::fetch(&state.pool, backend_label, &target, &hashes).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "translation cache fetch failed; serving without cache");
            Vec::new()
        }
    };
    let mut by_hash: HashMap<Vec<u8>, TranslatedItem> = cached
        .into_iter()
        .map(|r| {
            (
                r.text_sha256,
                TranslatedItem {
                    translated_text: r.translated_text,
                    detected_language: r.detected_language,
                    detected_confidence: r.detected_confidence,
                },
            )
        })
        .collect();

    let mut miss_indices: Vec<usize> = Vec::new();
    let mut seen_miss_hashes: HashMap<Vec<u8>, ()> = HashMap::new();
    for (i, h) in hashes.iter().enumerate() {
        if !by_hash.contains_key(h) && seen_miss_hashes.insert(h.clone(), ()).is_none() {
            miss_indices.push(i);
        }
    }

    if !miss_indices.is_empty() {
        let miss_texts: Vec<String> = miss_indices.iter().map(|&i| texts[i].clone()).collect();
        let translated = tokio::time::timeout(
            state.settings.request_timeout,
            state
                .backend
                .translate(&miss_texts, source, &target, format),
        )
        .await
        .map_err(|_| {
            let msg = format!(
                "translation timed out after {}s",
                state.settings.request_timeout.as_secs()
            );
            tracing::error!(error = %msg, "translation backend error");
            ApiError::http(502, "translation backend error")
        })?
        .map_err(|e| {
            tracing::error!(error = %e, "translation backend error");
            ApiError::http(502, "translation backend error")
        })?;
        if translated.len() != miss_texts.len() {
            let msg = format!(
                "backend returned {} items for {} inputs",
                translated.len(),
                miss_texts.len()
            );
            tracing::error!(error = %msg, "translation backend error");
            return Err(ApiError::http(502, "translation backend error"));
        }
        let mut to_store: Vec<(Vec<u8>, &TranslatedItem)> = Vec::with_capacity(translated.len());
        for (k, &i) in miss_indices.iter().enumerate() {
            let h = hashes[i].clone();
            to_store.push((h.clone(), &translated[k]));
            by_hash.insert(h, translated[k].clone());
        }
        if let Err(e) = cache::store(&state.pool, backend_label, &target, &to_store).await {
            tracing::warn!(error = %e, "translation cache store failed; result still served");
        }
    }

    let mut out = Vec::with_capacity(texts.len());
    for h in &hashes {
        let item = by_hash
            .get(h)
            .cloned()
            .ok_or_else(|| ApiError::Internal("missing translation after fill".into()))?;
        out.push(item);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::routing::post;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::super::backend::test_support::{client, spawn};
    use super::super::backend::{Backend, HttpBackend};
    use super::super::{
        BackendKind, DEFAULT_TRANSLATE_BATCH_LIMIT, DEFAULT_TRANSLATE_CHAR_LIMIT, Settings, router,
    };
    use super::*;

    fn texts(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn under_both_limits_passes() {
        assert!(enforce_limits(5000, 100, &texts(&["hello", "world"])).is_ok());
    }

    #[test]
    fn batch_over_limit_is_rejected() {
        let batch: Vec<String> = (0..3).map(|i| format!("t{i}")).collect();
        let err = enforce_limits(5000, 2, &batch).unwrap_err();
        assert!(
            matches!(err, ApiError::Http { status: 400, message } if message.contains("batch size (3)"))
        );
    }

    #[test]
    fn chars_are_summed_across_the_batch() {
        let batch = texts(&["aaaa", "bbbb"]);
        let err = enforce_limits(6, 100, &batch).unwrap_err();
        assert!(
            matches!(err, ApiError::Http { status: 400, message } if message.contains("8 characters"))
        );
        assert!(enforce_limits(8, 100, &batch).is_ok());
    }

    #[test]
    fn chars_counted_as_scalars_not_bytes() {
        assert!(enforce_limits(4, 100, &texts(&["\u{65e5}\u{672c}\u{8a9e}\u{3060}"])).is_ok());
        assert!(enforce_limits(3, 100, &texts(&["\u{65e5}\u{672c}\u{8a9e}\u{3060}"])).is_err());
    }

    #[test]
    fn defaults_match_libretranslate_shape() {
        assert_eq!(DEFAULT_TRANSLATE_CHAR_LIMIT, 5000);
        assert_eq!(DEFAULT_TRANSLATE_BATCH_LIMIT, 100);
    }

    #[test]
    fn the_detected_language_falls_back_to_the_request_then_english() {
        assert_eq!(resolve_language("es".into(), "auto"), "es");
        assert_eq!(resolve_language("auto".into(), "fr"), "fr");
        assert_eq!(resolve_language(" ".into(), "auto"), "en");
    }

    fn lazy_pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(300))
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap()
    }

    fn app(backend: Backend, kind: BackendKind, pool: sqlx::PgPool) -> Router {
        router(Arc::new(MediaState::with_backend(
            pool,
            Settings::for_tests(kind),
            backend,
        )))
    }

    async fn post_json(app: &Router, body: Value) -> (StatusCode, Value) {
        let resp = app
            .clone()
            .oneshot(
                Request::post("/translate")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn without_an_engine_the_route_answers_503_in_the_envelope() {
        let app = app(Backend::Disabled, BackendKind::Disabled, lazy_pool());
        let (status, body) = post_json(&app, json!({ "q": "hola", "target": "en" })).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["ok"], false);
        assert_eq!(body["error"], "translation backend not configured");
        assert_eq!(body["message"], body["error"]);
    }

    #[tokio::test]
    async fn a_non_json_body_is_a_400_before_the_engine_is_consulted() {
        let app = app(Backend::Disabled, BackendKind::Disabled, lazy_pool());
        let resp = app
            .oneshot(
                Request::post("/translate")
                    .header(header::CONTENT_TYPE, "text/plain")
                    .body(Body::from("q=hola"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v["error"],
            "Invalid request: expected application/json body"
        );
    }

    /// The mock over a pool that never connects: the cache probe fails and is
    /// logged, the answer is still served (the old "serving without cache").
    #[tokio::test]
    async fn single_batch_and_empty_shapes_over_the_mock_without_a_database() {
        let app = app(Backend::Mock, BackendKind::Mock, lazy_pool());
        let (status, body) = post_json(&app, json!({ "q": "hola", "target": "EN" })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({ "detectedLanguage": { "confidence": 1.0, "language": "en" }, "translatedText": "hola" })
        );

        let (status, body) = post_json(
            &app,
            json!({ "q": ["a", "b"], "source": "es", "target": "en" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["translatedText"], json!(["a", "b"]));
        assert_eq!(body["detectedLanguage"].as_array().unwrap().len(), 2);

        let (status, body) = post_json(&app, json!({ "q": [], "target": "en" })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({ "detectedLanguage": [], "translatedText": [] })
        );

        let (status, body) = post_json(&app, json!({ "target": "en" })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .starts_with("Invalid request:"),
            "{body}"
        );

        let too_many: Vec<String> = (0..=DEFAULT_TRANSLATE_BATCH_LIMIT)
            .map(|i| i.to_string())
            .collect();
        let (status, body) = post_json(&app, json!({ "q": too_many, "target": "en" })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"].as_str().unwrap().contains("batch size (101)"),
            "{body}"
        );
    }

    /// The contract the cache exists for: a second request with the same
    /// texts never reaches the engine, and a batch mixing hits and misses
    /// sends only the misses (deduplicated) while answering in input order.
    #[tokio::test]
    async fn repeated_texts_are_served_from_the_cache_on_the_scratch_database() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let sent = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let (seen, log) = (calls.clone(), sent.clone());
        let addr = spawn(
            "/translate",
            post(move |Json(body): Json<Value>| {
                let (seen, log) = (seen.clone(), log.clone());
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    log.lock().unwrap().push(body["q"].clone());
                    match body["q"].clone() {
                        Value::Array(q) => {
                            let out: Vec<String> = q
                                .iter()
                                .map(|t| format!("{}!", t.as_str().unwrap()))
                                .collect();
                            let det: Vec<Value> = q
                                .iter()
                                .map(|_| json!({ "language": "es", "confidence": 0.8 }))
                                .collect();
                            Json(json!({ "translatedText": out, "detectedLanguage": det }))
                        }
                        Value::String(t) => Json(json!({
                            "translatedText": format!("{t}!"),
                            "detectedLanguage": { "language": "es", "confidence": 0.8 }
                        })),
                        _ => Json(json!({})),
                    }
                }
            }),
        )
        .await;
        let backend = Backend::Http(HttpBackend::new(client(), format!("http://{addr}"), None));
        let app = app(backend, BackendKind::Http, pool.clone());
        let scope = uuid::Uuid::new_v4().simple().to_string();
        let (t1, t2, t3) = (
            format!("{scope} one"),
            format!("{scope} two"),
            format!("{scope} three"),
        );

        let (status, body) = post_json(&app, json!({ "q": t1, "target": "en" })).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["translatedText"], format!("{t1}!"));
        assert_eq!(body["detectedLanguage"]["language"], "es");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let (status, body) = post_json(&app, json!({ "q": t1, "target": "en" })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["translatedText"], format!("{t1}!"));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a repeated text is a cache hit"
        );

        let (status, body) = post_json(
            &app,
            json!({ "q": [t2.clone(), t1.clone(), t3.clone(), t2.clone()], "target": "en" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["translatedText"],
            json!([
                format!("{t2}!"),
                format!("{t1}!"),
                format!("{t3}!"),
                format!("{t2}!")
            ])
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let last = sent.lock().unwrap().last().cloned().unwrap();
        assert_eq!(
            last,
            json!([t2, t3]),
            "only the distinct misses reach the engine"
        );

        let hashes: Vec<Vec<u8>> = [&t1, &t2, &t3]
            .iter()
            .map(|t| cache::hash_text(t))
            .collect();
        sqlx::query(
            "DELETE FROM social.translation_cache WHERE backend = 'http' AND text_sha256 = ANY($1)",
        )
        .bind(&hashes)
        .execute(&pool)
        .await
        .unwrap();
    }
}

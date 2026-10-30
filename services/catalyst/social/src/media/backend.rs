//! The translation engines behind `POST /translate`. The old
//! `TranslationBackend` trait had one implementation per engine and nothing
//! else; [`Backend`] is that dispatch as an enum over the ONE shared client.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{BackendKind, Settings};

#[derive(Debug, Clone)]
pub struct TranslatedItem {
    pub translated_text: String,
    pub detected_language: String,
    pub detected_confidence: f32,
}

pub enum Backend {
    Disabled,
    Mock,
    Http(HttpBackend),
    Llm(LlmBackend),
}

impl Backend {
    pub fn build(settings: &Settings, client: reqwest::Client) -> Self {
        match settings.backend {
            BackendKind::Disabled => Self::Disabled,
            BackendKind::Mock => Self::Mock,
            BackendKind::Http => Self::Http(HttpBackend::new(
                client,
                settings
                    .backend_url
                    .clone()
                    .expect("http backend url checked in Settings::parse"),
                settings.backend_api_key.clone(),
            )),
            BackendKind::Llm => Self::Llm(LlmBackend::new(
                client,
                settings
                    .llm_base_url
                    .clone()
                    .expect("llm base url checked in Settings::parse"),
                settings.llm_api_key.clone(),
                settings.llm_model.clone(),
            )),
        }
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub async fn translate(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<Vec<TranslatedItem>, String> {
        match self {
            Self::Disabled => Err("translation backend not configured".to_string()),
            Self::Mock => Ok(texts
                .iter()
                .map(|t| TranslatedItem {
                    translated_text: t.clone(),
                    detected_language: "en".to_string(),
                    detected_confidence: 1.0,
                })
                .collect()),
            Self::Http(b) => b.translate(texts, source, target, format).await,
            Self::Llm(b) => b.translate(texts, source, target, format).await,
        }
    }
}

pub(crate) const TRANSLATE_CONCURRENCY: usize = 8;

/// Maps `f` over `texts` with at most TRANSLATE_CONCURRENCY in flight, in input order;
/// the first error aborts the rest, as the sequential loop did.
pub(crate) async fn translate_concurrently<F, Fut>(
    texts: &[String],
    f: F,
) -> Result<Vec<TranslatedItem>, String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<TranslatedItem, String>> + Send + 'static,
{
    let semaphore = Arc::new(tokio::sync::Semaphore::new(TRANSLATE_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    let mut out: Vec<Option<TranslatedItem>> = (0..texts.len()).map(|_| None).collect();
    let settle =
        |joined: Result<(usize, Result<TranslatedItem, String>), tokio::task::JoinError>,
         out: &mut Vec<Option<TranslatedItem>>|
         -> Result<(), String> {
            let (i, item) = joined.map_err(|e| format!("translate task failed: {e}"))?;
            out[i] = Some(item?);
            Ok(())
        };
    for (i, text) in texts.iter().cloned().enumerate() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        while let Some(joined) = tasks.try_join_next() {
            settle(joined, &mut out)?;
        }
        let fut = f(text);
        tasks.spawn(async move {
            let _permit = permit;
            (i, fut.await)
        });
    }
    while let Some(joined) = tasks.join_next().await {
        settle(joined, &mut out)?;
    }
    Ok(out.into_iter().flatten().collect())
}

#[derive(Clone)]
pub struct HttpBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct LtDetected {
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    language: String,
}

enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for OneOrMany<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        if value.is_array() {
            serde_json::from_value(value).map(Self::Many)
        } else {
            serde_json::from_value(value).map(Self::One)
        }
        .map_err(serde::de::Error::custom)
    }
}

/// LibreTranslate answers a string `q` with scalars and an array `q` with arrays.
#[derive(Deserialize)]
struct LtResponse {
    #[serde(rename = "detectedLanguage")]
    detected_language: Option<OneOrMany<LtDetected>>,
    #[serde(rename = "translatedText")]
    translated_text: OneOrMany<String>,
}

fn into_items(parsed: LtResponse, n: usize, source: &str) -> Result<Vec<TranslatedItem>, String> {
    let texts = match parsed.translated_text {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    };
    if texts.len() != n {
        return Err(format!(
            "upstream returned {} translations for {n} texts",
            texts.len()
        ));
    }
    let mut detected: Vec<Option<LtDetected>> = match parsed.detected_language {
        Some(OneOrMany::Many(v)) => v.into_iter().map(Some).collect(),
        Some(OneOrMany::One(d)) => vec![Some(d)],
        None => Vec::new(),
    };
    detected.resize_with(n, || None);
    Ok(texts
        .into_iter()
        .zip(detected)
        .map(|(translated_text, d)| {
            let (language, confidence) = match d {
                Some(d) => (
                    if d.language.is_empty() {
                        source.to_string()
                    } else {
                        d.language
                    },
                    d.confidence,
                ),
                None => (source.to_string(), 0.0),
            };
            TranslatedItem {
                translated_text,
                detected_language: language,
                detected_confidence: confidence,
            }
        })
        .collect())
}

enum BatchFailure {
    /// The upstream rejected or misread the array form; per-text requests still apply.
    Unsupported(String),
    Fatal(String),
}

impl HttpBackend {
    pub fn new(client: reqwest::Client, base_url: String, api_key: Option<String>) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    async fn post(
        &self,
        q: Value,
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<reqwest::Response, String> {
        let mut body = json!({
            "q": q,
            "source": source,
            "target": target,
            "format": format,
        });
        if let Some(key) = &self.api_key {
            body["api_key"] = json!(key);
        }
        self.client
            .post(format!("{}/translate", self.base_url))
            .timeout(std::time::Duration::from_secs(30))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e.without_url()))
    }

    async fn translate_one(
        &self,
        text: &str,
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<TranslatedItem, String> {
        let resp = self.post(json!(text), source, target, format).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("upstream {status}: {txt}"));
        }
        let parsed: LtResponse = resp
            .json()
            .await
            .map_err(|e| format!("decode failed: {}", e.without_url()))?;
        into_items(parsed, 1, source).map(|mut v| v.remove(0))
    }

    async fn translate_batch(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<Vec<TranslatedItem>, BatchFailure> {
        let resp = self
            .post(json!(texts), source, target, format)
            .await
            .map_err(BatchFailure::Fatal)?;
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            let msg = format!("upstream {status}: {txt}");
            return Err(if status.is_client_error() {
                BatchFailure::Unsupported(msg)
            } else {
                BatchFailure::Fatal(msg)
            });
        }
        let parsed: LtResponse = resp.json().await.map_err(|e| {
            BatchFailure::Unsupported(format!("decode failed: {}", e.without_url()))
        })?;
        into_items(parsed, texts.len(), source).map_err(BatchFailure::Unsupported)
    }

    pub async fn translate(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<Vec<TranslatedItem>, String> {
        match texts {
            [] => return Ok(Vec::new()),
            [text] => {
                return Ok(vec![
                    self.translate_one(text, source, target, format).await?,
                ]);
            }
            _ => {}
        }
        match self.translate_batch(texts, source, target, format).await {
            Ok(items) => return Ok(items),
            Err(BatchFailure::Fatal(e)) => return Err(e),
            Err(BatchFailure::Unsupported(e)) => {
                tracing::debug!(error = %e, "batched translate unsupported; sending per text");
            }
        }
        let (source, target, format) = (source.to_string(), target.to_string(), format.to_string());
        translate_concurrently(texts, |text| {
            let backend = self.clone();
            let (source, target, format) = (source.clone(), target.clone(), format.clone());
            async move {
                backend
                    .translate_one(&text, &source, &target, &format)
                    .await
            }
        })
        .await
    }
}

#[derive(Clone)]
pub struct LlmBackend {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct LlmTranslatePayload {
    #[serde(default)]
    translated_text: String,
    #[serde(default)]
    detected_language: String,
    #[serde(default)]
    confidence: f32,
}

impl LlmBackend {
    pub fn new(
        client: reqwest::Client,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
        }
    }

    fn system_prompt(source: &str, target: &str, format: &str) -> String {
        let mut p = format!(
            "You are a professional translation engine. Translate the user's message into the \
             language identified by the ISO 639-1 code '{target}'. Respond with ONLY a compact \
             JSON object, no markdown fences, with exactly these keys: \"translated_text\" (the \
             translation as a string), \"detected_language\" (ISO 639-1 code of the source \
             language), \"confidence\" (a number between 0 and 1 for how confident the source \
             detection is). Preserve meaning; do not add commentary."
        );
        let source = source.trim();
        if !source.is_empty() && !source.eq_ignore_ascii_case("auto") {
            p.push_str(&format!(" The source language is '{source}'."));
        }
        if format.eq_ignore_ascii_case("html") {
            p.push_str(
                " The input is HTML: keep every tag and attribute intact and translate only the \
                 human-readable text.",
            );
        }
        p
    }

    async fn translate_one(
        &self,
        text: &str,
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<TranslatedItem, String> {
        let body = json!({
            "model": self.model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": Self::system_prompt(source, target, format) },
                { "role": "user", "content": text },
            ],
        });
        let mut req = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .timeout(std::time::Duration::from_secs(30))
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e.without_url()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("llm proxy {status}: {txt}"));
        }
        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| format!("decode failed: {}", e.without_url()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| "llm proxy returned no choices".to_string())?;
        Ok(map_completion(&content, source))
    }

    pub async fn translate(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
        format: &str,
    ) -> Result<Vec<TranslatedItem>, String> {
        if let [text] = texts {
            return Ok(vec![
                self.translate_one(text, source, target, format).await?,
            ]);
        }
        let (source, target, format) = (source.to_string(), target.to_string(), format.to_string());
        translate_concurrently(texts, |text| {
            let backend = self.clone();
            let (source, target, format) = (source.clone(), target.clone(), format.clone());
            async move {
                backend
                    .translate_one(&text, &source, &target, &format)
                    .await
            }
        })
        .await
    }
}

fn strip_json_fences(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t);
    t.trim().trim_end_matches("```").trim()
}

fn map_completion(content: &str, source: &str) -> TranslatedItem {
    let body = strip_json_fences(content);
    if let Ok(payload) = serde_json::from_str::<LlmTranslatePayload>(body)
        && !payload.translated_text.is_empty()
    {
        let detected = if payload.detected_language.trim().is_empty() {
            source.to_string()
        } else {
            payload.detected_language
        };
        return TranslatedItem {
            translated_text: payload.translated_text,
            detected_language: detected,
            detected_confidence: payload.confidence,
        };
    }
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(body)
        && let Some(t) = map.get("translated_text").and_then(Value::as_str)
    {
        return TranslatedItem {
            translated_text: t.to_string(),
            detected_language: source.to_string(),
            detected_confidence: 0.0,
        };
    }
    TranslatedItem {
        translated_text: content.trim().to_string(),
        detected_language: source.to_string(),
        detected_confidence: 0.0,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::net::SocketAddr;

    use axum::Router;

    /// A loopback engine stub: `handler` answers `path`.
    pub async fn spawn(path: &'static str, handler: axum::routing::MethodRouter) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(path, handler);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    pub fn client() -> reqwest::Client {
        reqwest::Client::new()
    }
}

#[cfg(test)]
mod http_tests {
    use super::test_support::{client, spawn};
    use super::*;
    use axum::Json;
    use axum::routing::post;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn texts() -> Vec<String> {
        vec!["one".to_string(), "two".to_string(), "three".to_string()]
    }

    #[tokio::test]
    async fn batches_many_texts_into_one_request() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let addr = spawn(
            "/translate",
            post(move |Json(body): Json<Value>| {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    let q: Vec<String> = serde_json::from_value(body["q"].clone()).unwrap();
                    let translated: Vec<String> = q.iter().map(|t| format!("{t}!")).collect();
                    let detected: Vec<Value> = q
                        .iter()
                        .map(|_| json!({ "language": "en", "confidence": 0.5 }))
                        .collect();
                    Json(json!({ "translatedText": translated, "detectedLanguage": detected }))
                }
            }),
        )
        .await;
        let backend = HttpBackend::new(client(), format!("http://{addr}"), None);
        let out = backend
            .translate(&texts(), "auto", "es", "text")
            .await
            .unwrap();
        let got: Vec<&str> = out.iter().map(|i| i.translated_text.as_str()).collect();
        assert_eq!(got, vec!["one!", "two!", "three!"]);
        assert_eq!(out[1].detected_language, "en");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn falls_back_to_per_text_when_arrays_are_rejected() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let addr = spawn(
            "/translate",
            post(move |Json(body): Json<Value>| {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    match body["q"].as_str() {
                        Some(t) => (
                            axum::http::StatusCode::OK,
                            Json(json!({ "translatedText": format!("{t}!") })),
                        ),
                        None => (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(json!({ "error": "q must be a string" })),
                        ),
                    }
                }
            }),
        )
        .await;
        let backend = HttpBackend::new(client(), format!("http://{addr}"), None);
        let out = backend
            .translate(&texts(), "auto", "es", "text")
            .await
            .unwrap();
        let got: Vec<&str> = out.iter().map(|i| i.translated_text.as_str()).collect();
        assert_eq!(got, vec!["one!", "two!", "three!"]);
        assert_eq!(out[0].detected_language, "auto");
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn upstream_server_errors_are_not_retried_per_text() {
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let addr = spawn(
            "/translate",
            post(move || {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    (axum::http::StatusCode::BAD_GATEWAY, "down")
                }
            }),
        )
        .await;
        let backend = HttpBackend::new(client(), format!("http://{addr}"), None);
        let err = backend
            .translate(&texts(), "auto", "es", "text")
            .await
            .unwrap_err();
        assert!(err.contains("502"), "{err}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn shape_mismatch_is_an_error() {
        let parsed: LtResponse = serde_json::from_str(r#"{"translatedText":["a","b"]}"#).unwrap();
        assert!(into_items(parsed, 3, "auto").is_err());
        let parsed: LtResponse = serde_json::from_str(r#"{"translatedText":"a"}"#).unwrap();
        let items = into_items(parsed, 1, "fr").unwrap();
        assert_eq!(items[0].detected_language, "fr");
    }

    #[tokio::test]
    async fn the_mock_echoes_and_the_disabled_engine_refuses() {
        let out = Backend::Mock
            .translate(&texts(), "auto", "es", "text")
            .await
            .unwrap();
        assert_eq!(out[2].translated_text, "three");
        assert_eq!(out[2].detected_language, "en");
        assert!(Backend::Disabled.is_disabled());
        assert!(
            Backend::Disabled
                .translate(&texts(), "auto", "es", "text")
                .await
                .is_err()
        );
    }
}

#[cfg(test)]
mod llm_tests {
    use super::test_support::{client, spawn};
    use super::*;
    use axum::Json;
    use axum::routing::post;
    use std::sync::{Arc, Mutex};

    fn completion(content: &str) -> Json<Value> {
        Json(json!({ "choices": [ { "message": { "role": "assistant", "content": content } } ] }))
    }

    #[tokio::test]
    async fn parses_structured_json_completion() {
        let addr = spawn(
            "/v1/chat/completions",
            post(|| async { completion("{\"translated_text\":\"hola\",\"detected_language\":\"en\",\"confidence\":0.97}") }),
        )
        .await;
        let backend = LlmBackend::new(
            client(),
            format!("http://{addr}"),
            None,
            "test-model".into(),
        );
        let out = backend
            .translate(&["hello".to_string()], "auto", "es", "text")
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].translated_text, "hola");
        assert_eq!(out[0].detected_language, "en");
        assert!((out[0].detected_confidence - 0.97).abs() < 1e-6);
    }

    #[tokio::test]
    async fn strips_code_fences_and_defaults_language_to_source() {
        let addr = spawn(
            "/v1/chat/completions",
            post(|| async { completion("```json\n{\"translated_text\":\"bonjour\"}\n```") }),
        )
        .await;
        let backend = LlmBackend::new(
            client(),
            format!("http://{addr}"),
            None,
            "test-model".into(),
        );
        let out = backend
            .translate(&["hello".to_string()], "en", "fr", "text")
            .await
            .unwrap();
        assert_eq!(out[0].translated_text, "bonjour");
        assert_eq!(out[0].detected_language, "en");
    }

    #[tokio::test]
    async fn falls_back_to_raw_content_when_not_json() {
        let addr = spawn(
            "/v1/chat/completions",
            post(|| async { completion("hallo welt") }),
        )
        .await;
        let backend = LlmBackend::new(
            client(),
            format!("http://{addr}"),
            None,
            "test-model".into(),
        );
        let out = backend
            .translate(&["hello world".to_string()], "en", "de", "text")
            .await
            .unwrap();
        assert_eq!(out[0].translated_text, "hallo welt");
        assert_eq!(out[0].detected_language, "en");
    }

    type SeenRequest = Arc<Mutex<Option<(Option<String>, Value)>>>;

    #[tokio::test]
    async fn forwards_bearer_key_and_prompt() {
        let seen: SeenRequest = Arc::new(Mutex::new(None));
        let sink = seen.clone();
        let addr = spawn(
            "/v1/chat/completions",
            post(move |headers: axum::http::HeaderMap, Json(b): Json<Value>| {
                let sink = sink.clone();
                async move {
                    let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).map(str::to_string);
                    *sink.lock().unwrap() = Some((auth, b));
                    completion("{\"translated_text\":\"ok\",\"detected_language\":\"en\",\"confidence\":1}")
                }
            }),
        )
        .await;
        let backend = LlmBackend::new(
            client(),
            format!("http://{addr}"),
            Some("secret-key".into()),
            "m".into(),
        );
        backend
            .translate(&["hi".to_string()], "auto", "ja", "html")
            .await
            .unwrap();
        let guard = seen.lock().unwrap();
        let (auth, body) = guard.as_ref().unwrap();
        assert_eq!(auth.as_deref(), Some("Bearer secret-key"));
        assert_eq!(body["model"], "m");
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("'ja'"));
        assert!(system.contains("HTML"));
    }

    #[tokio::test]
    async fn upstream_error_is_surfaced() {
        let backend = LlmBackend::new(client(), "http://127.0.0.1:1".into(), None, "m".into());
        let err = backend
            .translate(&["x".to_string()], "auto", "es", "text")
            .await
            .unwrap_err();
        assert!(err.contains("request failed"), "{err}");
    }
}

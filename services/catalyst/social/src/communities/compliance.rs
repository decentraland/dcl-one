//! Community compliance at social-service-ea b6b2cef. The existing DAPPS
//! snapshot gates a lazy, process-local provider client; cache entries live on
//! the existing SQL pool and expire after the source Redis cache's two hours.
use super::http::CommError;
use axum::http::{HeaderValue, StatusCode};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

const SYSTEM_PROMPT: &str = "You are Decentraland's compliance expert. Analyze proposed community content against:\n• Code of Ethics (conduct, law, business integrity, confidentiality/privacy, no violence, drugs/alcohol limits, equal opportunity, conflicts of interest, KYC/legitimacy, reputational/legal risk).\n• Content Policy (no illegality: piracy/terrorism/obscenity/child sexual abuse, no IP infringement, no hate/harassment, no false/misleading/libel/privacy invasion, no data-privacy violations; gambling must be licensed & geo-blocked; age-restricted content (intense violence/gambling/sexual) must be 18+ with warnings).\n\nIMPORTANT:\n1) Analyze each field INDEPENDENTLY (do NOT infer issues across fields).\n2) Be strict but fair. Return concise issue tags (≤25 chars) per field. Max 3 per field.\n3) Output ONLY JSON that matches the provided schema.";
const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["isCompliant","issues","confidence"],"properties":{"isCompliant":{"type":"boolean"},"issues":{"type":"object","additionalProperties":false,"required":["name","description","image"],"properties":{"name":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":25}},"description":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":25}},"image":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":25}}}},"confidence":{"type":"number","minimum":0,"maximum":1}}}"#;

#[derive(Clone)]
pub struct Settings {
    pub environment: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub organization: Option<String>,
    pub project: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            environment: String::new(),
            model: "gpt-5-nano".into(),
            api_key: None,
            base_url: "https://api.openai.com/v1".into(),
            organization: None,
            project: None,
        }
    }
}
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComplianceSettings")
            .field("environment", &self.environment)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "[redacted]"))
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Content<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub thumbnail: Option<&'a [u8]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Issues {
    pub name: Vec<String>,
    pub description: Vec<String>,
    pub image: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResultValue {
    pub is_compliant: bool,
    pub issues: Issues,
    pub confidence: f64,
}
impl ResultValue {
    fn from_value(value: Value) -> Result<Self, String> {
        let value: Self = serde_json::from_value(value)
            .map_err(|_| "Invalid response structure from OpenAI API".to_owned())?;
        if !(0.0..=1.0).contains(&value.confidence)
            || [
                &value.issues.name,
                &value.issues.description,
                &value.issues.image,
            ]
            .iter()
            .any(|issues| issues.len() > 3 || issues.iter().any(|s| s.chars().count() > 25))
        {
            return Err("Invalid response structure from OpenAI API".into());
        }
        Ok(value)
    }
}
pub(crate) fn cache_hash(content: Content<'_>) -> [u8; 32] {
    let image = content
        .thumbnail
        .map(|bytes| hex::encode(Sha256::digest(bytes)))
        .unwrap_or_else(|| "no-img".into());
    Sha256::digest(format!(
        "{}\n{}\n{}",
        content.name.unwrap_or(""),
        content.description.unwrap_or(""),
        image
    ))
    .into()
}
pub(crate) fn request_body(model: &str, content: Content<'_>) -> Value {
    let name = content.name.filter(|s| !s.is_empty());
    let description = content.description.filter(|s| !s.is_empty());
    let text = format!(
        "Analyze this community content:\n{}{}{}Return JSON matching the schema.",
        name.map(|s| format!("Name: \"{s}\"")).unwrap_or_default(),
        description
            .map(|s| format!("\nDescription: \"{s}\""))
            .unwrap_or_default(),
        if name.is_some() || description.is_some() {
            "\n"
        } else {
            ""
        }
    );
    let input = if let Some(bytes) = content.thumbnail {
        let mime = super::thumbnail::detect_image_mime_type(bytes)
            .map(|mime| mime.as_str())
            .unwrap_or("application/octet-stream");
        json!([{"type":"message","role":"user","content":[{"type":"input_text","text":text},{"type":"input_image","image_url":format!("data:{mime};base64,{}",STANDARD.encode(bytes)),"detail":"auto"}]}])
    } else {
        json!(text)
    };
    json!({"model":model,"instructions":SYSTEM_PROMPT,"input":input,"text":{"format":{"type":"json_schema","name":"ComplianceValidationResult","schema":serde_json::from_str::<Value>(SCHEMA).unwrap()}}})
}

// Operational bound on the whole provider effect, as `referral_slack`'s
// EFFECT_TIMEOUT: the check runs inside POST/PATCH /v1/communities, and the
// source SDK's 3 attempts × 600 s with Retry-After waits have no total cap.
// Above the shared readers' 25 s so a slow but valid answer still lands.
pub(crate) const EFFECT_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) struct Adapter {
    http: crate::http::Outbound,
    settings: Settings,
}
impl Adapter {
    fn new(mut settings: Settings) -> Result<Self, String> {
        if settings.model.is_empty() {
            settings.model = "gpt-5-nano".into();
        }
        if settings.base_url.is_empty() {
            settings.base_url = "https://api.openai.com/v1".into();
        }
        let http = crate::http::Outbound::new(crate::http::HttpClientCfg {
            total_timeout: None,
            connect_timeout: Some(Duration::from_secs(10)),
            read_timeout: Some(Duration::from_secs(300)),
            max_redirects: Some(20),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?
        .allowing_configured_provider_hosts();
        Ok(Self { http, settings })
    }
    /// [`Self::validate`] within [`EFFECT_TIMEOUT`]; past it, the provider is unavailable.
    async fn validate_bounded(&self, content: Content<'_>) -> Result<ResultValue, String> {
        tokio::time::timeout(EFFECT_TIMEOUT, self.validate(content))
            .await
            .unwrap_or_else(|_| {
                Err("Unexpected error during compliance validation: Request timed out.".into())
            })
    }
    async fn validate(&self, content: Content<'_>) -> Result<ResultValue, String> {
        let Some(key) = self.settings.api_key.as_deref() else {
            return Err("Unexpected error during compliance validation: Missing credentials. Please pass an `apiKey`, or set the `OPENAI_API_KEY` environment variable.".into());
        };
        let mut bearer = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
            "Unexpected error during compliance validation: Invalid API key header".to_owned()
        })?;
        bearer.set_sensitive(true);
        let body = request_body(&self.settings.model, content);
        for attempt in 0..3u32 {
            let mut request = self
                .http
                .request(
                    reqwest::Method::POST,
                    format!("{}/responses", self.settings.base_url.trim_end_matches('/')),
                )
                .following_redirects(20)
                .header("authorization", bearer.clone())
                .header("x-stainless-retry-count", attempt.to_string())
                .header("x-stainless-timeout", "600")
                .json(&body);
            if let Some(org) = &self.settings.organization {
                request = request.header("openai-organization", org);
            }
            if let Some(project) = &self.settings.project {
                request = request.header("openai-project", project);
            }
            // The SDK clears its abort timer when fetch returns headers. Body
            // reads retain the transport inactivity timeout, with no total cap.
            let response =
                match tokio::time::timeout(Duration::from_secs(600), request.send()).await {
                    Ok(Ok(response)) => response,
                    failure => {
                        if attempt < 2 {
                            tokio::time::sleep(retry_delay(attempt, None)).await;
                            continue;
                        }
                        return Err(format!(
                            "Unexpected error during compliance validation: {}",
                            if match failure {
                                Err(_) => true,
                                Ok(Err(error)) => error.is_timeout(),
                                Ok(Ok(_)) => unreachable!(),
                            } {
                                "Request timed out."
                            } else {
                                "Connection error."
                            }
                        ));
                    }
                };
            let status = response.status();
            let headers = response.headers().clone();
            if !status.is_success()
                && attempt < 2
                && retryable(
                    status,
                    headers.get("x-should-retry").and_then(|v| v.to_str().ok()),
                )
            {
                drop(response);
                tokio::time::sleep(retry_delay(attempt, Some(&headers))).await;
                continue;
            }
            let bytes = response.bytes().await.map_err(|_| {
                // Node fetch terminates a response stream after inactivity or
                // a broken socket. Parsing happens after SDK transport retries.
                let message = if status.is_success() {
                    "terminated".to_owned()
                } else {
                    api_error(status, b"terminated")
                };
                format!("Unexpected error during compliance validation: {message}")
            })?;
            if !status.is_success() {
                return Err(format!(
                    "Unexpected error during compliance validation: {}",
                    api_error(status, &bytes)
                ));
            }
            let response: Value = serde_json::from_slice(&bytes).map_err(|error| {
                format!(
                    "Unexpected error during compliance validation: {}",
                    json_diagnostic(&String::from_utf8_lossy(&bytes), &error)
                )
            })?;
            let text = output_text(&response)?;
            if text.is_empty() {
                return Err("No content received from OpenAI API".into());
            }
            let value: Value = serde_json::from_str(&text).map_err(|error| {
                format!(
                    "Invalid JSON response from OpenAI API: {}",
                    json_diagnostic(&text, &error)
                )
            })?;
            return ResultValue::from_value(value);
        }
        unreachable!()
    }
}
pub(crate) fn output_text(response: &Value) -> Result<String, String> {
    if response["object"] != "response" {
        return Ok(response["output_text"].as_str().unwrap_or("").to_owned());
    }
    let mut text = String::new();
    let outputs = response["output"].as_array().ok_or_else(|| {
        "Unexpected error during compliance validation: rsp.output is not iterable".to_owned()
    })?;
    for output in outputs {
        if output["type"] != "message" {
            continue;
        }
        let contents = output["content"].as_array().ok_or_else(|| {
            "Unexpected error during compliance validation: output.content is not iterable"
                .to_owned()
        })?;
        for content in contents {
            if content["type"] == "output_text" {
                if let Some(value) = content["text"].as_str() {
                    text.push_str(value);
                }
            }
        }
    }
    Ok(text)
}
fn retryable(status: StatusCode, override_value: Option<&str>) -> bool {
    match override_value {
        Some("true") => true,
        Some("false") => false,
        _ => matches!(status.as_u16(), 408 | 409 | 429) || status.is_server_error(),
    }
}
fn retry_delay(attempt: u32, headers: Option<&reqwest::header::HeaderMap>) -> Duration {
    if let Some(headers) = headers {
        let millis = headers
            .get("retry-after-ms")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok());
        let millis = millis.filter(|n| *n != 0.0).or_else(|| {
            headers
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| {
                    s.parse::<f64>().map(|n| n * 1000.0).ok().or_else(|| {
                        chrono::DateTime::parse_from_rfc2822(s).ok().map(|date| {
                            (date.with_timezone(&chrono::Utc) - chrono::Utc::now())
                                .num_milliseconds() as f64
                        })
                    })
                })
        });
        if let Some(ms) = millis.filter(|n| *n > 0.0 && *n < 60000.0) {
            return Duration::from_secs_f64(ms / 1000.0);
        }
    }
    let jitter = 1.0 - (uuid::Uuid::new_v4().as_u128() as u64 as f64 / u64::MAX as f64) * 0.25;
    Duration::from_secs_f64(0.5 * 2f64.powi(attempt as i32) * jitter)
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}
fn api_error(status: StatusCode, body: &[u8]) -> String {
    let parsed = serde_json::from_slice::<Value>(body).ok();
    let error = parsed
        .as_ref()
        .and_then(|v| v.get("error"))
        .filter(|v| truthy(v));
    let message = error
        .and_then(|v| v.get("message"))
        .filter(|v| truthy(v))
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| v.to_string())
        })
        .or_else(|| error.map(Value::to_string))
        .or_else(|| {
            if parsed.as_ref().is_some_and(truthy) {
                None
            } else {
                Some(String::from_utf8_lossy(body).into_owned())
            }
        });
    if let Some(message) = message.filter(|s| !s.is_empty()) {
        format!("{} {message}", status.as_u16())
    } else {
        format!("{} status code (no body)", status.as_u16())
    }
}

fn unavailable(message: impl Into<String>) -> CommError {
    CommError::Body(
        StatusCode::BAD_REQUEST,
        json!({"error":"Community content validation unavailable","message":message.into(),"communityContentValidationUnavailable":true}),
    )
}

// Node26's JSON.parse diagnostics are part of the source AI-error message.
// Keep byte offsets for serde, then report ECMAScript UTF-16 positions.
pub(crate) fn json_diagnostic(text: &str, error: &serde_json::Error) -> String {
    let reason = error.to_string();
    let prefix = text
        .split_inclusive('\n')
        .take(error.line().saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let offset = if error.is_eof() {
        text.len()
    } else {
        prefix + error.column().saturating_sub(1)
    }
    .min(text.len());
    let offset = (0..=offset)
        .rev()
        .find(|n| text.is_char_boundary(*n))
        .unwrap_or(0);
    let before = &text[..offset];
    let position = before.encode_utf16().count();
    let line = before.chars().filter(|c| *c == '\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .encode_utf16()
        .count()
        + 1;
    let at = |message: &str| {
        format!("{message} in JSON at position {position} (line {line} column {column})")
    };
    if reason.starts_with("trailing characters") {
        return at("Unexpected non-whitespace character after JSON").replacen(" in JSON", "", 1);
    }
    if reason.starts_with("EOF while parsing a string") {
        return at("Unterminated string");
    }
    if reason.starts_with("expected `:`")
        || reason.starts_with("EOF while parsing an object") && text.trim_end().ends_with('"')
    {
        return at("Expected ':' after property name");
    }
    if reason.starts_with("expected `,` or `]`") || reason.starts_with("EOF while parsing a list") {
        return at("Expected ',' or ']' after array element");
    }
    if reason.starts_with("expected `,` or `}`")
        || reason.starts_with("EOF while parsing an object") && !text.trim_end().ends_with('{')
    {
        return at("Expected ',' or '}' after property value");
    }
    if reason.starts_with("key must be a string")
        || reason.starts_with("EOF while parsing an object")
    {
        return at(if before.trim_end().ends_with('{') {
            "Expected property name or '}'"
        } else {
            "Expected double-quoted property name"
        });
    }
    if error.is_eof() {
        return "Unexpected end of JSON input".into();
    }
    let trimmed = text.trim();
    if matches!(
        trimmed,
        "undefined" | "NaN" | "Infinity" | "[object Object]"
    ) {
        return format!("\"{text}\" is not valid JSON");
    }
    let token = text[offset..].chars().next().unwrap_or(' ');
    if text.encode_utf16().count() <= 20 {
        format!("Unexpected token '{token}', \"{text}\" is not valid JSON")
    } else {
        at(&format!("Unexpected token '{token}'"))
    }
}
pub(crate) fn changed(new: &str, old: &str) -> bool {
    let trim = |c: char| matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}');
    new.to_lowercase().trim_matches(trim) != old.to_lowercase().trim_matches(trim)
}
impl super::State {
    pub(crate) async fn validate_content(&self, content: Content<'_>) -> Result<(), CommError> {
        if !self.feature_enabled(super::features::AI_COMPLIANCE)
            || (content.name.unwrap_or("").is_empty()
                && content.description.unwrap_or("").is_empty()
                && content.thumbnail.is_none())
        {
            return Ok(());
        }
        if content
            .thumbnail
            .is_some_and(|bytes| super::thumbnail::detect_image_mime_type(bytes).is_none())
        {
            return Err(CommError::bad_request(
                super::thumbnail::ThumbnailError::UnsupportedSignature.message(),
            ));
        }
        if self.settings.compliance.environment != "prd"
            && !self.feature_enabled(super::features::DEV_AI_COMPLIANCE)
        {
            return Ok(());
        }
        let started = std::time::Instant::now();
        let hash = cache_hash(content);
        let model = if self.settings.compliance.model.is_empty() {
            "gpt-5-nano"
        } else {
            &self.settings.compliance.model
        };
        // As upstream, cache read failure is outside the provider's AI-error wrapper.
        let cached:Option<Value>=sqlx::query_scalar("SELECT result FROM social.community_compliance_cache WHERE model=$1 AND input_hash=$2 AND expires_at>timezone('UTC',now())").bind(model).bind(hash.as_slice()).fetch_optional(&self.pool).await.map_err(|error|{tracing::error!(%error,"community compliance cache read failed");CommError::Internal})?;
        let result = if let Some(value) = cached {
            let result = ResultValue::from_value(value).map_err(unavailable)?;
            metrics::counter!("ai_compliance_validation_total", "result" => if result.is_compliant { "compliant_cached" } else { "non-compliant_cached" }).increment(1);
            metrics::histogram!("ai_compliance_validation_duration_seconds")
                .record(started.elapsed().as_secs_f64());
            result
        } else {
            let adapter = self
                .compliance
                .get_or_init(|| Adapter::new(self.settings.compliance.clone()));
            let adapter = adapter.as_ref().map_err(|error| {
                metrics::counter!("ai_compliance_validation_total", "result" => "failed")
                    .increment(1);
                unavailable(format!(
                    "Unexpected error during compliance validation: {error}"
                ))
            })?;
            let result = adapter.validate_bounded(content).await.map_err(|error| {
                metrics::counter!("ai_compliance_validation_total", "result" => "failed")
                    .increment(1);
                unavailable(error)
            })?;
            metrics::histogram!("ai_compliance_validation_duration_seconds")
                .record(started.elapsed().as_secs_f64());
            metrics::counter!("ai_compliance_validation_total", "result" => if result.is_compliant { "compliant" } else { "non-compliant" }).increment(1);
            sqlx::query("INSERT INTO social.community_compliance_cache(model,input_hash,result,expires_at) VALUES($1,$2,$3,timezone('UTC',now())+interval '2 hours') ON CONFLICT(model,input_hash) DO UPDATE SET result=EXCLUDED.result,expires_at=EXCLUDED.expires_at")
                .bind(model).bind(hash.as_slice()).bind(serde_json::to_value(&result).unwrap()).execute(&self.pool).await.map_err(|error| {
                    metrics::counter!("ai_compliance_validation_total", "result" => "failed").increment(1);
                    unavailable(format!("Unexpected error during compliance validation: {error}"))
                })?;
            result
        };
        if result.is_compliant {
            Ok(())
        } else {
            Err(CommError::Body(
                StatusCode::BAD_REQUEST,
                json!({"error":"Community not compliant","message":"Community content violates Decentraland's Code of Ethics","data":{"issues":result.issues}}),
            ))
        }
    }
}
pub fn spawn_job(state: &Arc<super::State>, jobs: &crate::jobs::Jobs) {
    let pool = state.pool.clone();
    jobs.spawn_periodic_leased("community-compliance-cache",crate::jobs::PeriodicCfg::every(Duration::from_secs(3600)),move|_ctx|{let pool=pool.clone();async move{sqlx::query("DELETE FROM social.community_compliance_cache WHERE expires_at<=timezone('UTC',now()) AND (model,input_hash) IN (SELECT model,input_hash FROM social.community_compliance_cache WHERE expires_at<=timezone('UTC',now()) ORDER BY expires_at LIMIT 1000)").execute(&pool).await?;Ok(())}});
}

#[cfg(test)]
#[path = "compliance_transport_tests.rs"]
mod transport_tests;

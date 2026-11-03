//! Social's analytics-component 1.0.5 HTTP contract at social-service-ea b6b2cef.
//! Uses the passed Outbound; tasks are transient and never hold mutation results.
use std::{sync::Arc, time::Duration};

use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct Settings {
    pub context: Option<String>,
    pub api_url: Option<String>,
    pub api_token: Option<String>,
    pub environment: Option<String>,
    pub request_timeout_ms: Option<f64>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsSettings")
            .field("configured", &self.context.is_some())
            .field("api_token", &self.api_token.as_ref().map(|_| "[redacted]"))
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Analytics(Arc<Inner>);

struct Inner {
    http: crate::http::Outbound,
    transport: std::sync::OnceLock<Result<crate::http::Outbound, String>>,
    context: String,
    url: String,
    token: String,
    environment: String,
    timeout: Duration,
}

impl Analytics {
    /// The monolith permits analytics to be absent; a configured replacement
    /// requires the source's four strings, including explicitly empty values.
    pub fn new(http: &crate::http::Outbound, settings: Settings) -> Result<Option<Self>, String> {
        if settings.context.is_none()
            && settings.api_token.is_none()
            && settings.environment.is_none()
            && settings.request_timeout_ms.is_none()
            && settings.api_url.as_deref().is_none_or(str::is_empty)
        {
            return Ok(None);
        }
        let required = |value: Option<String>, name: &str| {
            value.ok_or_else(|| format!("Configuration: string {name} is required"))
        };
        let context = required(settings.context, "ANALYTICS_CONTEXT")?;
        let url = required(settings.api_url, "ANALYTICS_API_URL")?;
        let token = required(settings.api_token, "ANALYTICS_API_TOKEN")?;
        let environment = required(settings.environment, "ENV")?;
        let timeout = request_timeout(settings.request_timeout_ms);
        Ok(Some(Self(Arc::new(Inner {
            http: http.clone(),
            transport: std::sync::OnceLock::new(),
            context,
            url,
            token,
            environment,
            timeout,
        }))))
    }

    pub fn fire_event(&self, event: &str, mut properties: Value) {
        let Some(body) = properties.as_object_mut() else {
            tracing::error!(
                event,
                "Error sending event to Analytics: properties must be an object"
            );
            return;
        };
        body.insert("env".into(), Value::String(self.0.environment.clone()));
        let payload = json!({"event":event,"body":properties,"context":self.0.context});
        let inner = self.0.clone();
        let event = event.to_owned();
        tokio::spawn(async move {
            // The source timer covers fetch through response headers, not body
            // consumption. One deadline covers DNS and all redirect hops.
            let http = match inner.transport.get_or_init(|| {
                inner
                    .http
                    .provider_profile(
                        Some(Duration::from_secs(10)),
                        Some(Duration::from_secs(300)),
                    )
                    .map(crate::http::Outbound::allowing_configured_provider_hosts)
                    .map_err(|error| error.to_string())
            }) {
                Ok(http) => http,
                Err(_) => {
                    tracing::error!(
                        event,
                        "Error sending event to Analytics: client setup failed"
                    );
                    return;
                }
            };
            let send = http
                .post(&inner.url)
                .header("x-token", &inner.token)
                .header("Content-Type", "application/json")
                .following_redirects(20)
                .json(&payload)
                .send();
            match tokio::time::timeout(inner.timeout, send).await {
                Ok(Ok(response)) => {
                    let status = response.status();
                    // Dropping the response cancels its unconsumed body; no
                    // buffering, draining, JSON parse, retry or body deadline.
                    drop(response);
                    if !status.is_success() {
                        tracing::error!(
                            event,
                            status = status.as_u16(),
                            "Error sending event to Analytics"
                        );
                    }
                }
                Ok(Err(_)) => {
                    tracing::error!(event, "Error sending event to Analytics: transport failed")
                }
                Err(_) => tracing::error!(
                    event,
                    "Error sending event to Analytics: Request aborted (timed out)"
                ),
            }
        });
    }
}

fn request_timeout(configured: Option<f64>) -> Duration {
    let milliseconds = match configured {
        Some(number) if number.is_finite() && number > 0.0 => number,
        Some(_) => {
            tracing::warn!("Invalid ANALYTICS_REQUEST_TIMEOUT; using default 10000ms");
            10_000.0
        }
        None => 10_000.0,
    };
    // Node's setTimeout integer bucket, including finite overflow/sub-ms input.
    let bucket = if (1.0..=2_147_483_647.0).contains(&milliseconds) {
        milliseconds.trunc() as u64
    } else {
        1
    };
    Duration::from_millis(bucket)
}

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;

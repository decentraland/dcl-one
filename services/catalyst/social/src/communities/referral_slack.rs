//! Referral-only Slack effects at social-service-ea b6b2cef. The installed
//! source SDK's payloads and failure classifications use the existing Outbound.
//! Awaited effects have a finite budget, including admission and retries, so a
//! Slack outage cannot hold referral HTTP requests or the SQS consumer forever.
use crate::http::Outbound;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::Instant;

// Operational safety bounds, not a delivery SLO. Keep each effect well below
// the legacy queue's 60-second visibility and do not detach persisted effects.
const EFFECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(2);
const MAX_ATTEMPTS: usize = 3;

#[derive(Clone, Default)]
pub struct Settings {
    pub token: Option<String>,
    pub dashboard: String,
    pub environment: String,
}
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReferralSlackSettings")
            .field("token", &self.token.as_ref().map(|_| "[redacted]"))
            .field("dashboard", &self.dashboard)
            .field("environment", &self.environment)
            .finish()
    }
}

pub(super) struct Adapter {
    http: Outbound,
    settings: Settings,
    endpoint: String,
    slots: tokio::sync::Semaphore,
    paused_until: tokio::sync::Mutex<Option<Instant>>,
}
impl Adapter {
    pub fn new(http: Outbound, settings: Settings) -> Self {
        Self {
            http,
            settings,
            endpoint: "https://slack.com/api/chat.postMessage".into(),
            slots: tokio::sync::Semaphore::new(100),
            paused_until: Default::default(),
        }
    }
    #[cfg(test)]
    pub(super) fn set_test_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
    }
    pub fn timing(&self, referrer: &str, newest: i64, previous: i64) -> Value {
        timing_message(&self.settings, referrer, newest, previous)
    }
    pub fn ip(&self, referrer: &str, invited: &str, ip: &str, maximum: f64) -> Value {
        ip_message(&self.settings, referrer, invited, ip, maximum)
    }
    pub fn milestone(&self, referrer: &str) -> Value {
        milestone_message(&self.settings, referrer)
    }
    pub async fn send(&self, message: Value) {
        let delays = retry_delays(|| {
            let bytes = uuid::Uuid::new_v4().into_bytes();
            let sample = bytes[..6]
                .iter()
                .fold(0_u64, |value, byte| value * 256 + u64::from(*byte));
            sample as f64 / (1_u64 << 48) as f64
        });
        if let Err(error) = self.send_with_delays(message, &delays).await {
            // The source awaits and swallows these effects after its SQL write.
            tracing::warn!(%error, "referral Slack notification failed after persistence");
        }
    }
    async fn send_with_delays(&self, message: Value, delays: &[Duration]) -> anyhow::Result<()> {
        let Some(token) = self.settings.token.as_deref() else {
            return Ok(());
        };
        tokio::time::timeout(EFFECT_TIMEOUT, self.send_attempts(token, message, delays))
            .await
            .map_err(|_| anyhow::anyhow!("Slack effect exceeded its time budget"))?
    }
    async fn send_attempts(
        &self,
        token: &str,
        message: Value,
        delays: &[Duration],
    ) -> anyhow::Result<()> {
        let body = form_body(&message);
        for attempt in 0..MAX_ATTEMPTS {
            let permit = self.slots.acquire().await?;
            loop {
                let remaining = self
                    .paused_until
                    .lock()
                    .await
                    .map(|until| until.saturating_duration_since(Instant::now()));
                if let Some(remaining) = remaining.filter(|duration| !duration.is_zero()) {
                    tokio::time::sleep(remaining).await;
                } else {
                    break;
                }
            }
            let mut request = self
                .http
                .post(&self.endpoint)
                .max_redirects(0)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body.clone());
            // An explicitly empty required source token still sends the call;
            // WebClient omits its Authorization header for the empty string.
            if !token.is_empty() {
                request = request.header("authorization", format!("Bearer {token}"));
            }
            let response = request.send().await;
            let failure = match response {
                Err(error) => error.to_string(),
                Ok(response) if response.status().as_u16() == 429 => {
                    let delay = retry_after(
                        response
                            .headers()
                            .get("retry-after")
                            .and_then(|value| value.to_str().ok()),
                    )
                    .ok_or_else(|| anyhow::anyhow!("Slack retry-after header is invalid"))?;
                    let until = Instant::now() + delay;
                    *self.paused_until.lock().await = Some(until);
                    tokio::time::sleep(delay).await;
                    "Slack rate limit exceeded".into()
                }
                Ok(response) if response.status().as_u16() == 200 => {
                    match response.bytes().await {
                        Err(error) => error.to_string(),
                        Ok(bytes) => {
                            let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
                                anyhow::anyhow!("Slack returned an invalid response")
                            })?;
                            if !value.get("ok").is_some_and(js_truthy) {
                                // Source platform errors are checked after makeRequest,
                                // so its transport retry ladder does not run here.
                                anyhow::bail!("Slack rejected the message");
                            }
                            return Ok(());
                        }
                    }
                }
                Ok(response) => format!("Slack returned HTTP {}", response.status().as_u16()),
            };
            drop(permit);
            let Some(delay) = delays.get(attempt).filter(|_| attempt + 1 < MAX_ATTEMPTS) else {
                anyhow::bail!("{failure}")
            };
            tokio::time::sleep(*delay).await;
        }
        unreachable!("each final attempt returns")
    }
}
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|n| n != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
fn retry_delays(mut random: impl FnMut() -> f64) -> Vec<Duration> {
    let mut delays = (0..MAX_ATTEMPTS - 1)
        .map(|attempt| {
            Duration::from_millis(
                ((random() + 1.0) * 1000.0 * 1.96821_f64.powi(attempt as i32)).round() as u64,
            )
        })
        .collect::<Vec<_>>();
    delays.sort();
    delays
}
fn retry_after(raw: Option<&str>) -> Option<Duration> {
    // Retain SDK parseInt syntax; cap positive waits instead of allowing its
    // long sleeps or overflow-dependent timer behavior.
    let raw = raw?.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (negative, raw) = match raw.as_bytes().first() {
        Some(b'-') => (true, &raw[1..]),
        Some(b'+') => (false, &raw[1..]),
        _ => (false, raw),
    };
    let (radix, raw) = if raw.starts_with("0x") || raw.starts_with("0X") {
        (16, &raw[2..])
    } else {
        (10, raw)
    };
    let mut number = 0_f64;
    let mut digits = 0;
    for digit in raw.chars().map_while(|c| c.to_digit(radix)) {
        number = number * f64::from(radix) + f64::from(digit);
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    let millis = if negative { -number } else { number } * 1000.0;
    Some(if millis.is_sign_positive() && millis > 0.0 {
        Duration::from_secs_f64((millis / 1000.0).min(MAX_RETRY_AFTER.as_secs_f64()))
    } else {
        Duration::from_millis(1)
    })
}
fn encode(raw: &str) -> String {
    let mut result = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            result.push(char::from(byte))
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").expect("string write");
        }
    }
    result
}
fn form_body(message: &Value) -> String {
    // The source adapter supplies these fields in this order. Referral builders
    // set three; undefined optional fields are omitted by the SDK serializer.
    [
        "channel",
        "text",
        "blocks",
        "attachments",
        "username",
        "icon_emoji",
        "icon_url",
        "thread_ts",
        "reply_broadcast",
        "unfurl_links",
        "unfurl_media",
    ]
    .iter()
    .filter_map(|key| {
        message.get(*key).map(|value| {
            let text = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            format!("{}={}", encode(key), encode(&text))
        })
    })
    .collect::<Vec<_>>()
    .join("&")
}
fn channel(settings: &Settings) -> &'static str {
    if settings.environment == "dev" {
        "notifications-dev"
    } else {
        "referral-notifications"
    }
}
fn header(text: &str) -> Value {
    json!({"type":"header","text":{"type":"plain_text","text":text,"emoji":true}})
}
fn section(text: String) -> Value {
    json!({"type":"section","text":{"type":"mrkdwn","text":text}})
}
fn action(settings: &Settings, style: &str) -> Value {
    json!({"type":"actions","elements":[{"type":"button","text":{"type":"plain_text","text":"View Referral Dashboard","emoji":true},"url":settings.dashboard,"style":style}]})
}
fn timing_message(settings: &Settings, referrer: &str, newest: i64, previous: i64) -> Value {
    let minutes = (((newest - previous) as f64 / 60_000.0) * 100.0).round() / 100.0;
    let iso = |ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .expect("database timestamp")
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    };
    json!({"channel":channel(settings),"text":"⚠️ Suspicious Referral Timing - Multiple invitations within 5 minutes","blocks":[
        header("⚠️ Suspicious Referral Timing"),
        section(format!("*🕒 Rapid Invitations Detected*\n\n*Referrer:* `{referrer}`\n*Time Between Invitations:* {minutes} minutes")),
        // The pinned source passes timestamp strings in these two user fields.
        section(format!("*📊 Invitation Details:*\n*New Invited User:* `{newest}`\n*Time:* {}\n\n*Previous Invited User:* `{previous}`\n*Time:* {}",iso(newest),iso(previous))),
        section(format!("*🔍 Analysis:*\n• Two invitations created within {minutes} minutes\n• Potential automated behavior or coordination\n• May require manual review")), action(settings,"primary") ]})
}
fn ip_message(settings: &Settings, referrer: &str, invited: &str, ip: &str, maximum: f64) -> Value {
    json!({"channel":channel(settings),"text":"🚨 IP Match Rejection - Referral blocked due to IP limit exceeded","blocks":[
        header("🚨 IP Match Rejection Detected"),
        section(format!("*⚠️ Suspicious Activity Detected*\n\n*Invited User:* `{invited}`\n*IP Address:* `{ip}`\n*Referrer:* `{referrer}`")),
        section(format!("*📊 Details:*\n• IP address has reached maximum of {maximum} referrals\n• Potential abuse or automated system detected\n• Referral automatically rejected\n\n*🔍 Next Steps:*\n• Monitor for similar patterns\n• Review IP address activity")),action(settings,"danger") ]})
}
fn milestone_message(settings: &Settings, referrer: &str) -> Value {
    json!({"channel":channel(settings),"text":format!("🎉 Referral Milestone Reached! - 100 Invites Tier Achieved by referrer {referrer}"),"blocks":[
        header("🎉 Referral Milestone Reached!"),section(format!("*🎯 100 Invites Tier Achieved!*\n\n*Referrer Wallet:* `{referrer}`")),
        section("*📊 Details:*\n• Referrer has successfully invited 100 users\n• Tier milestone unlocked\n• Ready for outreach and communication\n\n*💬 Next Steps:*\n• Monitor for email submission from referrer".into()),action(settings,"primary") ]})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn fixture() -> Value {
        serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/source-contracts/referral-slack.json"))).unwrap()
    }
    fn fields(raw: &str) -> Value {
        Value::Object(
            url::form_urlencoded::parse(raw.as_bytes())
                .map(|(key, value)| {
                    let value = if key == "blocks" {
                        serde_json::from_str(&value).unwrap()
                    } else {
                        Value::String(value.into_owned())
                    };
                    (key.into_owned(), value)
                })
                .collect(),
        )
    }
    async fn stub(
        scenario: &str,
    ) -> (
        String,
        Arc<tokio::sync::Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let scenario = scenario.to_string();
        let received = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let requests = received.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let end = loop {
                    let mut chunk = [0_u8; 1024];
                    let count = socket.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        break None;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        break Some(end + 4);
                    }
                };
                let Some(end) = end else { continue };
                let header = String::from_utf8(bytes[..end].to_vec()).unwrap();
                let get = |name: &str| {
                    header
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(key, _)| key.eq_ignore_ascii_case(name))
                        .map(|(_, value)| value.trim().to_string())
                };
                let length: usize = get("content-length")
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(0);
                while bytes.len() - end < length {
                    let mut chunk = [0_u8; 1024];
                    let count = socket.read(&mut chunk).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]);
                }
                let number = {
                    let mut requests = requests.lock().await;
                    requests.push(
                        json!({"path":header.lines().next().unwrap().split(' ').nth(1).unwrap(),
                        "authorization":get("authorization"),"content_type":get("content-type"),
                        "body":String::from_utf8(bytes[end..end+length].to_vec()).unwrap()}),
                    );
                    requests.len()
                };
                if scenario == "transport-exhausted"
                    || (scenario == "transport-then-success" && number == 1)
                {
                    continue;
                }
                let status = if scenario == "redirect" {
                    302
                } else if scenario == "exhausted"
                    || (scenario == "http-then-success" && number == 1)
                {
                    500
                } else if scenario.starts_with("rate-") && number == 1 {
                    429
                } else {
                    200
                };
                let body = json!({"ok": scenario != "platform-error"}).to_string();
                let extra = if scenario == "rate-then-success" && number == 1 {
                    "retry-after: 0\r\n"
                } else if scenario == "rate-huge" && number == 1 {
                    "retry-after: 999999999\r\n"
                } else if scenario == "redirect" {
                    "location: /must-not-follow\r\n"
                } else {
                    ""
                };
                let response = format!(
                    "HTTP/1.1 {status} fixture\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        (
            format!("http://{address}/chat.postMessage"),
            received,
            server,
        )
    }
    #[tokio::test]
    async fn referral_slack_payloads_and_forms_match_the_executed_source_sdk() {
        let fixture = fixture();
        let (endpoint, requests, server) = stub("success").await;
        for case in fixture["cases"].as_array().unwrap() {
            let settings = Settings {
                token: Some("fixture-token".into()),
                dashboard: case["dashboard"].as_str().unwrap().into(),
                environment: if case["is_dev"] == true {
                    "dev".into()
                } else {
                    "prd".into()
                },
            };
            let mut adapter = Adapter::new(Outbound::for_tests(), settings);
            adapter.set_test_endpoint(endpoint.clone());
            let referrer = case["referrer"].as_str().unwrap();
            let message = match case["kind"].as_str().unwrap() {
                "timing" => adapter.timing(
                    referrer,
                    case["newest_ms"].as_i64().unwrap(),
                    case["previous_ms"].as_i64().unwrap(),
                ),
                "ip" => adapter.ip(
                    referrer,
                    case["invited"].as_str().unwrap(),
                    "192.0.2.1",
                    2.0,
                ),
                "milestone" => adapter.milestone(referrer),
                other => panic!("unknown source kind {other}"),
            };
            assert_eq!(message, case["message"]);
            adapter
                .send_with_delays(message, &[Duration::ZERO; 10])
                .await
                .unwrap();
            let requests = requests.lock().await;
            let actual = requests.last().unwrap();
            assert_eq!(actual["authorization"], case["request"]["authorization"]);
            assert_eq!(actual["content_type"], case["request"]["content_type"]);
            // Object property order within JSON blocks is not a Slack field.
            assert_eq!(
                fields(actual["body"].as_str().unwrap()),
                fields(case["request"]["body"].as_str().unwrap())
            );
        }
        let actual = retry_delays(|| 0.5)
            .iter()
            .map(|delay| delay.as_millis() as u64)
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            Value::Array(
                fixture["retry_timeout_ms_at_random_half"]
                    .as_array()
                    .unwrap()[..MAX_ATTEMPTS - 1]
                    .to_vec()
            )
        );
        server.abort();
    }
    #[tokio::test]
    async fn referral_slack_keeps_source_failure_classifications_with_bounded_attempts() {
        let fixture = fixture();
        for case in fixture["transport"].as_array().unwrap() {
            let (endpoint, requests, server) = stub(case["scenario"].as_str().unwrap()).await;
            // A redirect-following default proves this request's override is used.
            let http = Outbound::new(crate::http::HttpClientCfg::default().following_redirects(2))
                .unwrap()
                .allowing_private_hosts();
            let mut adapter = Adapter::new(
                http,
                Settings {
                    token: Some("fixture-token".into()),
                    ..Default::default()
                },
            );
            adapter.set_test_endpoint(endpoint);
            let result = adapter
                .send_with_delays(
                    fixture["cases"][0]["message"].clone(),
                    &[Duration::ZERO; 10],
                )
                .await;
            assert_eq!(
                if result.is_ok() { "ok" } else { "error" },
                case["result"],
                "{}",
                case["scenario"]
            );
            let requests = requests.lock().await;
            assert_eq!(
                requests.len(),
                (case["attempts"].as_u64().unwrap() as usize).min(MAX_ATTEMPTS),
                "{}",
                case["scenario"]
            );
            assert!(
                requests
                    .iter()
                    .all(|request| request["path"] == "/chat.postMessage")
            );
            server.abort();
        }
    }
    #[tokio::test]
    async fn slack_large_retry_after_and_repeated_transport_failure_finish_within_budget() {
        for (scenario, attempts, succeeds) in [
            ("rate-huge", 2, true),
            ("transport-exhausted", MAX_ATTEMPTS, false),
        ] {
            let (endpoint, requests, server) = stub(scenario).await;
            let mut adapter = Adapter::new(
                Outbound::for_tests(),
                Settings {
                    token: Some("fixture-token".into()),
                    ..Default::default()
                },
            );
            adapter.set_test_endpoint(endpoint);
            let started = Instant::now();
            let result = adapter
                .send_with_delays(
                    json!({"channel":"fixture","text":"hello"}),
                    &[Duration::ZERO; 10],
                )
                .await;
            assert_eq!(result.is_ok(), succeeds, "{scenario}");
            assert_eq!(requests.lock().await.len(), attempts, "{scenario}");
            assert!(started.elapsed() < EFFECT_TIMEOUT);
            if scenario == "rate-huge" {
                assert!(started.elapsed() >= MAX_RETRY_AFTER);
            }
            assert_eq!(adapter.slots.available_permits(), 100);
            server.abort();
        }
    }

    #[tokio::test]
    async fn an_explicit_empty_slack_token_sends_without_authorization_like_the_source() {
        let source: Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"))).unwrap();
        let (endpoint, requests, server) = stub("success").await;
        let mut adapter = Adapter::new(
            Outbound::for_tests(),
            Settings {
                token: Some(String::new()),
                ..Default::default()
            },
        );
        adapter.set_test_endpoint(endpoint);
        adapter
            .send_with_delays(json!({"channel":"fixture", "text":"hello"}), &[])
            .await
            .unwrap();
        let requests = requests.lock().await;
        assert_eq!(
            requests.len() as u64,
            source["slack"]["empty_token"]["requests"]
        );
        assert_eq!(
            requests[0]["authorization"],
            source["slack"]["empty_token"]["authorization"]
        );
        server.abort();
    }

    #[tokio::test]
    async fn slack_effect_budget_cancels_stalled_headers_and_bodies_and_releases_permits() {
        let fixture: Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/provider-boundary-source-cases.json"))).unwrap();
        // This source capture remains unchanged: our finite budget is deliberate.
        assert_eq!(fixture["slack"]["axios_timeout_ms"], 0);
        for stalled_body in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}/chat.postMessage", listener.local_addr().unwrap());
            let (entered, received) = tokio::sync::oneshot::channel();
            let (release, released) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0; 4096];
                socket.read(&mut buffer).await.unwrap();
                if stalled_body {
                    socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\nconnection: close\r\n\r\n{\"ok\":").await.unwrap();
                }
                entered.send(()).unwrap();
                released.await.unwrap();
            });
            let mut adapter = Adapter::new(
                Outbound::for_tests(),
                Settings {
                    token: Some("fixture-token".into()),
                    ..Default::default()
                },
            );
            adapter.set_test_endpoint(endpoint);
            let adapter = Arc::new(adapter);
            let running = adapter.clone();
            let pending = tokio::spawn(async move {
                let started = Instant::now();
                let result = running
                    .send_with_delays(
                        json!({"channel":"fixture","text":"hello"}),
                        &[Duration::ZERO; 10],
                    )
                    .await;
                (started.elapsed(), result)
            });
            received.await.unwrap();
            tokio::time::pause();
            // Awaiting the task lets the runtime drive its timer and advance
            // only to the next deadline. advance()+yield_now() does not promise
            // that expired timers have run. The driver rounds both the deadline
            // and the remaining parking duration up to millisecond boundaries.
            let (elapsed, result) = pending.await.unwrap();
            let error = result.unwrap_err();
            assert!(error.to_string().contains("time budget"), "{error}");
            assert!(
                elapsed <= EFFECT_TIMEOUT + Duration::from_millis(2),
                "Slack effect took {elapsed:?} with stalled_body={stalled_body}"
            );
            eprintln!("Slack stalled_body={stalled_body}: budget error after {elapsed:?}");
            assert_eq!(adapter.slots.available_permits(), 100);
            tokio::time::resume();
            release.send(()).unwrap();
            server.await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn slack_effect_budget_includes_permit_wait_and_shared_rate_limit_pause() {
        for wait_for_permit in [false, true] {
            let adapter = Arc::new(Adapter::new(
                Outbound::for_tests(),
                Settings {
                    token: Some("fixture-token".into()),
                    ..Default::default()
                },
            ));
            let held = if wait_for_permit {
                Some(adapter.slots.acquire_many(100).await.unwrap())
            } else {
                *adapter.paused_until.lock().await =
                    Some(Instant::now() + Duration::from_secs(3600));
                None
            };
            let started = Instant::now();
            let result = adapter
                .send_with_delays(json!({"channel":"fixture","text":"hello"}), &[])
                .await;
            assert!(result.unwrap_err().to_string().contains("time budget"));
            assert_eq!(started.elapsed(), EFFECT_TIMEOUT);
            drop(held);
            assert_eq!(adapter.slots.available_permits(), 100);
        }
    }

    #[test]
    fn slack_retry_after_cannot_extend_the_effect_with_large_or_overflowing_values() {
        for raw in [
            "2",
            "999999999",
            "99999999999999999999999999999999999999999999999999999999999999999999999999",
        ] {
            assert_eq!(retry_after(Some(raw)), Some(MAX_RETRY_AFTER));
        }
        assert_eq!(retry_after(Some("-1")), Some(Duration::from_millis(1)));
        assert_eq!(retry_after(Some("not-a-number")), None);
    }

    #[tokio::test]
    async fn referral_slack_failures_preserve_rejection_milestone_and_retry_side_effects() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let (endpoint, requests, server) = stub("platform-error").await;
        let address = || {
            crate::Address::parse(&format!(
                "0x{}{}",
                uuid::Uuid::new_v4().simple(),
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            ))
            .unwrap()
        };
        let referrer = address();
        let invited = [address(), address(), address()];
        let settings = super::super::referral::Settings {
            slack: Settings {
                token: Some("fixture-token".into()),
                environment: "dev".into(),
                ..Default::default()
            },
            ..super::super::referral::Settings::for_tests()
        };
        let mut referral = super::super::referral::Referral::new(
            pool.clone(),
            Outbound::for_tests(),
            settings.clone(),
        );
        referral.set_test_slack_endpoint(endpoint.clone());
        referral
            .create(&referrer, &invited[0], "192.0.2.1")
            .await
            .unwrap();
        sqlx::query("UPDATE social.referral_progress SET created_at=created_at-interval '1 minute' WHERE invited_user=$1").bind(&invited[0]).execute(&pool).await.unwrap();
        referral
            .create(&referrer, &invited[1], "192.0.2.1")
            .await
            .unwrap();
        assert!(
            referral
                .create(&referrer, &invited[2], "192.0.2.1")
                .await
                .is_err()
        );
        let status: String =
            sqlx::query_scalar("SELECT status FROM social.referral_progress WHERE invited_user=$1")
                .bind(&invited[2])
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "rejected_ip_match");
        assert_eq!(requests.lock().await.len(), 3);
        // The same-referrer retry exits before timing and IP notifications.
        referral
            .create(&referrer, &invited[2], "192.0.2.1")
            .await
            .unwrap();
        assert_eq!(requests.lock().await.len(), 3);
        for _ in 0..99 {
            sqlx::query("INSERT INTO social.referral_progress(invited_user,referrer,status,tier_granted) VALUES($1,$2,'tier_granted',true)").bind(address()).bind(&referrer).execute(&pool).await.unwrap();
        }
        let mut settings = settings;
        settings.min_login_days = 0.0;
        settings.suspicious_window_ms = 0.0;
        let mut referral =
            super::super::referral::Referral::new(pool.clone(), Outbound::for_tests(), settings);
        referral.set_test_slack_endpoint(endpoint);
        let milestone = address();
        referral
            .create(&referrer, &milestone, "192.0.2.2")
            .await
            .unwrap();
        referral
            .finalize_referral(milestone.as_str())
            .await
            .unwrap();
        let granted: bool = sqlx::query_scalar(
            "SELECT tier_granted FROM social.referral_progress WHERE invited_user=$1",
        )
        .bind(&milestone)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(granted);
        let captured = requests.lock().await;
        assert_eq!(captured.len(), 4);
        assert!(
            fields(captured[3]["body"].as_str().unwrap())["text"]
                .as_str()
                .unwrap()
                .contains("100 Invites")
        );
        drop(captured);
        referral
            .finalize_referral(milestone.as_str())
            .await
            .unwrap();
        assert_eq!(requests.lock().await.len(), 4);
        server.abort();
    }
}

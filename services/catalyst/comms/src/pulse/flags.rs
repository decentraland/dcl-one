//! The four known Pulse configuration keys projected from `pulse.json` flags/variants.
use super::ip_limiter::IpLimiterOptions;
use futures::StreamExt;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use std::time::Duration;
use std::{
    collections::{HashMap, HashSet},
    fmt,
};

const MAX_BODY: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const INITIAL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_secs(60);

pub struct FlagsPoller {
    client: reqwest::Client,
    url: reqwest::Url,
    fallback: IpLimiterOptions,
}

impl FlagsPoller {
    pub fn new(url: &str, fallback: IpLimiterOptions) -> anyhow::Result<Self> {
        let url: reqwest::Url = url.parse()?;
        let loopback = url
            .host_str()
            .and_then(|h| h.trim_matches(['[', ']']).parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            anyhow::bail!(
                "PULSE_FLAGS_URL requires a full HTTPS document URL (loopback HTTP permitted)"
            );
        }
        let client = crate::http::HttpClientCfg::default()
            .with_total_timeout(REQUEST_TIMEOUT)
            .builder()
            .no_proxy()
            .build()?;
        Ok(Self {
            client,
            url,
            fallback,
        })
    }
    pub async fn start(
        self,
    ) -> (
        IpLimiterOptions,
        tokio::sync::watch::Receiver<IpLimiterOptions>,
    ) {
        let initial = match tokio::time::timeout(INITIAL_TIMEOUT, self.fetch()).await {
            Ok(Ok(options)) => options,
            _ => {
                tracing::warn!("Pulse initial flags fetch failed; using local settings");
                self.fallback.clone()
            }
        };
        let (tx, rx) = tokio::sync::watch::channel(initial.clone());
        tokio::spawn(async move {
            loop {
                tokio::select! { biased; _ = tx.closed() => break, _ = tokio::time::sleep(POLL_INTERVAL) => {} }
                tokio::select! {
                    biased;
                    _ = tx.closed() => break,
                    result = self.fetch() => match result {
                        Ok(options) => { tx.send_if_modified(|previous| { if *previous == options { false } else { *previous = options; true } }); },
                        Err(error) => tracing::warn!(%error, "Pulse flags fetch failed; retaining last good settings"),
                    }
                }
            }
        });
        (initial, rx)
    }

    async fn fetch(&self) -> anyhow::Result<IpLimiterOptions> {
        tokio::time::timeout(REQUEST_TIMEOUT, async {
            let response = self
                .client
                .get(self.url.clone())
                .header("X-Debug", "false")
                .send()
                .await?
                .error_for_status()?;
            if response
                .content_length()
                .is_some_and(|size| size > MAX_BODY as u64)
            {
                anyhow::bail!("Pulse flags body exceeds 1MiB");
            }
            let mut body = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if body.len().saturating_add(chunk.len()) > MAX_BODY {
                    anyhow::bail!("Pulse flags body exceeds 1MiB");
                }
                body.extend_from_slice(&chunk);
            }
            project(&body, &self.fallback)
        })
        .await?
    }
}

// Retain object order and duplicate keys. serde_json::Value sorts keys, which would change the
// upstream last-occurrence rule when prefixed or case-insensitive flag names collide.
#[derive(Debug)]
enum Node {
    Object(Vec<(String, Node)>),
    Scalar(serde_json::Value),
    Array(Vec<Node>),
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Ordered;
        impl<'de> Visitor<'de> for Ordered {
            type Value = Node;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON value")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Node, M::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Node::Object(entries))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Node, S::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<Node>()? {
                    values.push(value);
                }
                Ok(Node::Array(values))
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Node, E> {
                Ok(Node::Scalar(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Node, E> {
                Ok(Node::Scalar(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Node, E> {
                Ok(Node::Scalar(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Node, E> {
                Ok(Node::Scalar(v.into()))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Node, E> {
                Ok(Node::Scalar(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Node, E> {
                Ok(Node::Scalar(serde_json::Value::Null))
            }
        }
        d.deserialize_any(Ordered)
    }
}
impl Node {
    fn field(&self, name: &str) -> Option<&Node> {
        match self {
            Self::Object(entries) => entries
                .iter()
                .rev()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            Self::Scalar(v) => v.as_str(),
            _ => None,
        }
    }
    fn boolean(&self) -> Option<bool> {
        match self {
            Self::Scalar(v) => v.as_bool(),
            _ => None,
        }
    }
}

fn unqualified(key: &str) -> String {
    let key = key.to_ascii_lowercase();
    key.strip_prefix("pulse-").unwrap_or(&key).to_owned()
}

fn project(body: &[u8], fallback: &IpLimiterOptions) -> anyhow::Result<IpLimiterOptions> {
    let root: Node = serde_json::from_slice(body)?;
    if !matches!(root, Node::Object(_)) {
        anyhow::bail!("flags document must be an object");
    }
    let mut flags = Vec::<(String, bool)>::new();
    if let Some(node) = root.field("flags") {
        match node {
            Node::Object(entries) => {
                for (key, value) in entries {
                    let enabled = value
                        .boolean()
                        .ok_or_else(|| anyhow::anyhow!("flag must be boolean"))?;
                    let key = unqualified(key);
                    if let Some((_, existing)) = flags.iter_mut().find(|(k, _)| *k == key) {
                        *existing = enabled;
                    } else {
                        flags.push((key, enabled));
                    }
                }
            }
            Node::Scalar(serde_json::Value::Null) => {}
            _ => anyhow::bail!("flags must be an object"),
        }
    }
    let mut variants = HashMap::new();
    if let Some(node) = root.field("variants") {
        match node {
            Node::Object(entries) => {
                for (key, value) in entries {
                    validate_variant(value)?;
                    variants.insert(unqualified(key), value);
                }
            }
            Node::Scalar(serde_json::Value::Null) => {}
            _ => anyhow::bail!("variants must be an object"),
        }
    }
    let mut overrides = HashMap::new();
    for (flag, enabled) in flags {
        if !enabled {
            continue;
        }
        let Some(variant) = variants.get(&flag) else {
            continue;
        };
        if matches!(variant, Node::Scalar(serde_json::Value::Null)) {
            anyhow::bail!("enabled variant cannot be null");
        }
        if variant.field("enabled").and_then(Node::boolean) != Some(true)
            || !variant
                .field("name")
                .and_then(Node::text)
                .is_some_and(|s| s.eq_ignore_ascii_case("configuration"))
        {
            continue;
        }
        let Some(payload) = variant.field("payload") else {
            continue;
        };
        if !payload
            .field("type")
            .and_then(Node::text)
            .is_some_and(|s| s.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let Some(text) = payload
            .field("value")
            .and_then(Node::text)
            .filter(|s| !s.trim().is_empty())
        else {
            continue;
        };
        let fragment: Node = serde_json::from_slice(&configuration_json(text)?)?;
        if !matches!(fragment, Node::Object(_)) {
            anyhow::bail!("configuration fragment must be an object");
        }
        let mut leaves = Vec::new();
        flatten(&fragment, "", &mut leaves);
        let mut seen = HashSet::new();
        for (key, value) in leaves {
            if !seen.insert(key.clone()) {
                anyhow::bail!("duplicate flattened configuration key");
            }
            let Some(value) = value else {
                continue;
            };
            if valid_override(&key, &value) {
                overrides.insert(key, value);
            } else {
                tracing::warn!(
                    key,
                    "remote Pulse key unknown or invalid; using local fallback"
                );
            }
        }
    }
    let mut next = fallback.clone();
    for (key, value) in overrides {
        match key.rsplit(':').next().unwrap_or_default() {
            "enabled" => next.enabled = value.trim().eq_ignore_ascii_case("true"),
            "maxconcurrency" => next.max_concurrency = value.trim().parse::<i32>()? as usize,
            "scenelistenermaxconcurrency" => {
                next.scene_listener_max_concurrency = value.trim().parse::<i32>()? as usize
            }
            "whitelist" => {
                next.whitelist =
                    super::hardening::IpWhitelist::parse(&value).map_err(anyhow::Error::msg)?
            }
            _ => unreachable!(),
        }
    }
    Ok(next)
}

fn validate_variant(variant: &Node) -> anyhow::Result<()> {
    let Node::Object(entries) = variant else {
        if matches!(variant, Node::Scalar(serde_json::Value::Null)) {
            return Ok(());
        }
        anyhow::bail!("variant must be an object");
    };
    for (key, value) in entries {
        match key.to_ascii_lowercase().as_str() {
            "enabled" if value.boolean().is_none() => {
                anyhow::bail!("variant enabled must be boolean")
            }
            "name"
                if value.text().is_none()
                    && !matches!(value, Node::Scalar(serde_json::Value::Null)) =>
            {
                anyhow::bail!("variant name must be a string")
            }
            "payload" => match value {
                Node::Scalar(serde_json::Value::Null) => {}
                Node::Object(fields) => {
                    for (key, value) in fields {
                        if (key.eq_ignore_ascii_case("type") || key.eq_ignore_ascii_case("value"))
                            && value.text().is_none()
                            && !matches!(value, Node::Scalar(serde_json::Value::Null))
                        {
                            anyhow::bail!("variant payload type and value must be strings");
                        }
                    }
                }
                _ => anyhow::bail!("variant payload must be an object"),
            },
            _ => {}
        }
    }
    Ok(())
}

// Microsoft JSON configuration permits comments and trailing commas in a fragment. The outer
// Unleash response remains strict JSON. Replace only syntax outside strings before deserializing.
fn configuration_json(text: &str) -> anyhow::Result<Vec<u8>> {
    let mut bytes = text.as_bytes().to_vec();
    let mut cursor = 0;
    let mut quoted = false;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if quoted => {
                cursor += 2;
                continue;
            }
            b'"' => quoted = !quoted,
            b'/' if !quoted && bytes.get(cursor + 1) == Some(&b'/') => {
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    bytes[cursor] = b' ';
                    cursor += 1;
                }
                continue;
            }
            b'/' if !quoted && bytes.get(cursor + 1) == Some(&b'*') => {
                let start = cursor;
                cursor += 2;
                while cursor + 1 < bytes.len() && &bytes[cursor..cursor + 2] != b"*/" {
                    cursor += 1;
                }
                if cursor + 1 >= bytes.len() {
                    anyhow::bail!("unterminated configuration comment");
                }
                cursor += 2;
                bytes[start..cursor].fill(b' ');
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    quoted = false;
    cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' if quoted => {
                cursor += 2;
                continue;
            }
            b'"' => quoted = !quoted,
            b',' if !quoted => {
                // A trailing comma follows a completed value. A leading/comma-only object
                // or array is malformed and must reach serde unchanged, not become empty.
                let after_value = bytes[..cursor]
                    .iter()
                    .rev()
                    .find(|b| !b.is_ascii_whitespace())
                    .is_some_and(|b| matches!(b, b'"' | b'}' | b']' | b'0'..=b'9' | b'e' | b'l'));
                if after_value
                    && bytes[cursor + 1..]
                        .iter()
                        .find(|b| !b.is_ascii_whitespace())
                        .is_some_and(|b| matches!(b, b'}' | b']'))
                {
                    bytes[cursor] = b' ';
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    Ok(bytes)
}

fn flatten(node: &Node, prefix: &str, leaves: &mut Vec<(String, Option<String>)>) {
    match node {
        Node::Object(entries) => {
            if entries.is_empty() && !prefix.is_empty() {
                leaves.push((prefix.into(), None));
            }
            for (key, value) in entries {
                let key = if prefix.is_empty() {
                    key.to_ascii_lowercase()
                } else {
                    format!("{prefix}:{}", key.to_ascii_lowercase())
                };
                flatten(value, &key, leaves);
            }
        }
        Node::Scalar(serde_json::Value::Null) => leaves.push((prefix.into(), None)),
        Node::Scalar(value) => leaves.push((
            prefix.into(),
            Some(
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string()),
            ),
        )),
        Node::Array(values) => {
            if values.is_empty() {
                leaves.push((prefix.into(), None));
            }
            for (index, value) in values.iter().enumerate() {
                flatten(value, &format!("{prefix}:{index}"), leaves);
            }
        }
    }
}

fn valid_override(key: &str, value: &str) -> bool {
    let Some(knob) = key.strip_prefix("transport:hardening:iplimiter:") else {
        return false;
    };
    match knob {
        "enabled" => {
            value.trim().eq_ignore_ascii_case("true") || value.trim().eq_ignore_ascii_case("false")
        }
        "maxconcurrency" | "scenelistenermaxconcurrency" => {
            value.trim().parse::<i32>().is_ok_and(|n| n >= 0)
        }
        "whitelist" => super::hardening::IpWhitelist::parse(value).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document(payload: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "flags": {"pulse-limit": true},
            "variants": {"pulse-limit": {"enabled": true, "name": "configuration",
                "payload": {"type": "json", "value": payload}}}
        }))
        .unwrap()
    }

    #[test]
    fn baseline_nested_and_colon_keys_convert_scalars_case_insensitively() {
        let actual = project(&document(r#"{"transport":{"hardening":{"iplimiter":{"enabled":"TRUE","MaxConcurrency":" +7 ","SceneListenerMaxConcurrency":0,"Whitelist":"::ffff:192.0.2.1"}}}}"#), &IpLimiterOptions::default()).unwrap();
        assert!(actual.enabled);
        assert_eq!(actual.max_concurrency, 7);
        assert_eq!(actual.scene_listener_max_concurrency, 0);
        assert!(actual.whitelist.contains("192.0.2.1"));
        let actual = project(
            &document(r#"{"Transport:Hardening:IpLimiter:Enabled":true}"#),
            &IpLimiterOptions::default(),
        )
        .unwrap();
        assert!(actual.enabled);
    }

    #[test]
    fn malformed_document_or_fragment_retains_snapshot_and_valid_removal_uses_local_fallback() {
        let fallback = IpLimiterOptions {
            max_concurrency: 9,
            ..Default::default()
        };
        assert!(project(b"null", &fallback).is_err());
        assert!(project(b"{", &fallback).is_err());
        assert!(project(&document("{"), &fallback).is_err());
        assert!(project(&document("{,}"), &fallback).is_err());
        assert!(project(&document(r#"{"Transport":{"Hardening":{,}}}"#), &fallback).is_err());
        assert!(
            project(
                br#"{"flags":{"x":true},"variants":{"x":{"enabled":"true"}}}"#,
                &fallback
            )
            .is_err()
        );
        assert_eq!(project(b"{}", &fallback).unwrap(), fallback);
        let body = br#"{"flags":{"pulse-limit":false},"variants":{}}"#;
        assert_eq!(project(body, &fallback).unwrap(), fallback);
    }

    #[test]
    fn invalid_keys_are_skipped_individually_and_never_expand_remote_scope() {
        let body = document(
            r#"{"Transport:Hardening:IpLimiter":{"Enabled":true,"MaxConcurrency":"ten","SceneListenerMaxConcurrency":-1,"Whitelist":["192.0.2.1"]},"PULSE_V4_ENABLED":false}"#,
        );
        let actual = project(&body, &IpLimiterOptions::default()).unwrap();
        assert!(actual.enabled);
        assert_eq!(actual.max_concurrency, 10);
        assert_eq!(actual.scene_listener_max_concurrency, 2);
        assert!(actual.whitelist.is_empty());
    }

    #[test]
    fn duplicate_flattened_keys_reject_document_but_last_flag_collision_wins() {
        assert!(project(&document(r#"{"Transport:Hardening:IpLimiter:Enabled":true,"transport:hardening:iplimiter:enabled":false}"#), &IpLimiterOptions::default()).is_err());
        assert!(project(&document(r#"{"Transport:Hardening:IpLimiter:Enabled":true,"transport:hardening:iplimiter:enabled":null}"#), &IpLimiterOptions::default()).is_err());
        assert!(
            project(
                &document(
                    r#"{"Transport:Hardening:IpLimiter:Whitelist":[{"IP":"192.0.2.1","ip":null}]}"#
                ),
                &IpLimiterOptions::default()
            )
            .is_err()
        );
        let body = br#"{"flags":{"pulse-limit":false,"LIMIT":true},"variants":{"pulse-limit":{"enabled":true,"name":"configuration","payload":{"type":"json","value":"{\"Transport:Hardening:IpLimiter:Enabled\":true}"}}}}"#;
        assert!(project(body, &IpLimiterOptions::default()).unwrap().enabled);
    }

    #[test]
    fn configuration_fragments_accept_baseline_comments_and_trailing_commas() {
        let body = document(
            "{ /* knob */ \"Transport:Hardening:IpLimiter:Enabled\": true, // enabled\n }",
        );
        assert!(
            project(&body, &IpLimiterOptions::default())
                .unwrap()
                .enabled
        );
    }

    async fn http(
        body: Vec<u8>,
    ) -> (
        String,
        std::sync::Arc<std::sync::Mutex<(u16, Vec<u8>)>>,
        tokio::task::JoinHandle<()>,
    ) {
        use axum::{
            Router,
            extract::State,
            http::{HeaderMap, StatusCode},
            routing::get,
        };
        let state = std::sync::Arc::new(std::sync::Mutex::new((200u16, body)));
        let app = Router::new()
            .route(
                "/pulse.json",
                get(
                    |State(state): State<std::sync::Arc<std::sync::Mutex<(u16, Vec<u8>)>>>,
                     headers: HeaderMap| async move {
                        assert_eq!(headers.get("x-debug").unwrap(), "false");
                        assert!(headers.get("referer").is_none());
                        let (status, body) = state.lock().unwrap().clone();
                        (StatusCode::from_u16(status).unwrap(), body)
                    },
                ),
            )
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/pulse.json", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (url, state, task)
    }

    // Requests run on real time: a paused clock jumps to their timeouts whenever loopback I/O is
    // not ready yet, as on macOS. Only the poll interval is skipped.
    #[tokio::test]
    async fn http_initial_refresh_failure_and_removal_preserve_the_right_snapshot() {
        let (url, state, server) = http(document(
            r#"{"Transport:Hardening:IpLimiter:Enabled":true}"#,
        ))
        .await;
        let fallback = IpLimiterOptions {
            max_concurrency: 7,
            ..Default::default()
        };
        let poller = FlagsPoller::new(&url, fallback.clone()).unwrap();
        let (initial, mut rx) = poller.start().await;
        assert!(initial.enabled);
        assert_eq!(initial.max_concurrency, 7);
        let refresh = FlagsPoller::new(&url, fallback.clone()).unwrap();
        for (status, body) in [
            (500, b"bad".to_vec()),
            (200, b"{".to_vec()),
            (200, document("{")),
            (200, document("{,}")),
            (200, document(r#"{"Transport":{"Hardening":{,}}}"#)),
            (200, br#"{"flags":{"x":true},"variants":{"x":{"enabled":"true"}}}"#.to_vec()),
            (200, br#"{"flags":{"x":true},"variants":{"x":{"enabled":true,"name":"configuration","payload":{"type":"json","value":{}}}}}"#.to_vec()),
        ] {
            *state.lock().unwrap() = (status, body);
            assert!(refresh.fetch().await.is_err());
        }
        assert!(rx.borrow().enabled);
        *state.lock().unwrap() = (200, b"{}".to_vec());
        let mut removed = false;
        for _ in 0..5 {
            tokio::time::pause();
            tokio::time::advance(POLL_INTERVAL).await;
            tokio::time::resume();
            if let Ok(changed) = tokio::time::timeout(Duration::from_secs(1), rx.changed()).await {
                changed.unwrap();
                removed = true;
                break;
            }
        }
        assert!(removed);
        assert_eq!(*rx.borrow(), fallback);
        drop(rx);
        server.abort();
    }

    #[tokio::test]
    async fn http_oversized_body_falls_back_without_applying_remote_values() {
        let (url, _, server) = http(vec![b' '; 1024 * 1024 + 1]).await;
        let fallback = IpLimiterOptions::default();
        let (initial, rx) = FlagsPoller::new(&url, fallback.clone())
            .unwrap()
            .start()
            .await;
        assert_eq!(initial, fallback);
        drop(rx);
        server.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn initial_stalled_request_returns_local_settings_after_five_seconds() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/pulse.json", listener.local_addr().unwrap());
        let before = tokio::time::Instant::now();
        let fallback = IpLimiterOptions::default();
        let (initial, rx) = FlagsPoller::new(&url, fallback.clone())
            .unwrap()
            .start()
            .await;
        assert_eq!(initial, fallback);
        assert_eq!(before.elapsed(), Duration::from_secs(5));
        drop(rx);
        drop(listener);
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_refresh_request_times_out_after_ten_seconds() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/pulse.json", listener.local_addr().unwrap());
        let poller = FlagsPoller::new(&url, IpLimiterOptions::default()).unwrap();
        let before = tokio::time::Instant::now();
        assert!(poller.fetch().await.is_err());
        assert_eq!(before.elapsed(), Duration::from_secs(10));
        drop(listener);
    }

    #[tokio::test]
    async fn dropping_receiver_cancels_an_inflight_body_and_stops_polling() {
        use axum::{Router, body::Body, response::Response, routing::get};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct OnDrop(Arc<tokio::sync::Notify>);
        impl Drop for OnDrop {
            fn drop(&mut self) {
                self.0.notify_one();
            }
        }
        let started = Arc::new(tokio::sync::Notify::new());
        let dropped = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/pulse.json",
            get({
                let started = started.clone();
                let dropped = dropped.clone();
                let calls = calls.clone();
                move || {
                    let started = started.clone();
                    let dropped = dropped.clone();
                    let calls = calls.clone();
                    async move {
                        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            return Response::new(Body::from(b"{}".to_vec()));
                        }
                        let guard = OnDrop(dropped);
                        started.notify_one();
                        let stream = futures::stream::pending::<Result<Vec<u8>, std::io::Error>>()
                            .map(move |item| {
                                let _ = &guard;
                                item
                            });
                        Response::new(Body::from_stream(stream))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/pulse.json", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let (_, rx) = FlagsPoller::new(&url, IpLimiterOptions::default())
            .unwrap()
            .start()
            .await;
        tokio::task::yield_now().await;
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(60)).await;
        tokio::time::resume();
        tokio::time::timeout(Duration::from_secs(3), started.notified())
            .await
            .unwrap();
        drop(rx);
        tokio::time::timeout(Duration::from_secs(3), dropped.notified())
            .await
            .unwrap();
        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(180)).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        server.abort();
    }
}

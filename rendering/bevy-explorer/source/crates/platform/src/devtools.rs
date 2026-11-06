//! Read-only Chrome DevTools Network target for the engine's HTTP transport.
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, bail, Result};
use async_std::{
    future::timeout,
    io::{ReadExt, WriteExt},
    net::{TcpListener, TcpStream},
    task,
};
use async_tungstenite::WebSocketStream;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, Semaphore};
use tungstenite::{
    handshake::{
        machine::TryParse,
        server::{create_response, write_response, Request},
    },
    protocol::{Role, WebSocketConfig},
    Message,
};

const TARGET: &str = "/devtools/page/explorer";
const EVENT_CAPACITY: usize = 256;
const MAX_URL_CHARS: usize = 4096;
const IO_TIMEOUT: Duration = Duration::from_secs(3);
static REPORTER: OnceLock<Reporter> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Reporter {
    started: Instant,
    events: broadcast::Sender<String>,
}

/// Start a read-only Chrome DevTools Network target on loopback for engine HTTP requests.
/// Called only when explicitly requested at native startup.
pub fn start(port: u16) -> Result<SocketAddr> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let listener = TcpListener::from(listener);
    let (events, _) = broadcast::channel(EVENT_CAPACITY);
    REPORTER
        .set(Reporter {
            started: Instant::now(),
            events,
        })
        .map_err(|_| anyhow!("Network inspector already running"))?;
    task::spawn(async move {
        let connections = Arc::new(Semaphore::new(8));
        while let Ok((stream, _)) = listener.accept().await {
            if let Ok(permit) = connections.clone().try_acquire_owned() {
                task::spawn(async move {
                    let _permit = permit;
                    let _ = serve(stream, addr).await;
                });
            }
        }
    });
    Ok(addr)
}

async fn read_request(stream: &mut TcpStream) -> Result<(Request, Vec<u8>)> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    while bytes.len() < 8192 {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some((used, request)) = Request::try_parse(&bytes)? {
            return Ok((request, bytes.split_off(used)));
        }
    }
    bail!("Incomplete or oversized inspector handshake")
}

async fn serve(mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let (request, pending) = timeout(IO_TIMEOUT, read_request(&mut stream)).await??;
    let headers = request.headers();
    let hosts = headers.get_all("host");
    let origins = headers.get_all("origin");
    let host_ok = hosts.iter().count() == 1
        && hosts.iter().all(|v| {
            v.to_str().is_ok_and(|host| {
                host == addr.to_string() || host == format!("localhost:{}", addr.port())
            })
        });
    // Browsers cannot forge Origin. Local command-line clients omit it.
    let origin_ok = origins.iter().count() <= 1
        && origins.iter().all(|v| {
            matches!(
                v.to_str(),
                Ok("devtools://devtools" | "chrome-devtools://devtools")
            )
        });
    let path = request.uri().path();
    if !host_ok || !origin_ok {
        reply(&mut stream, "403 Forbidden", "").await?;
    } else if path == TARGET {
        let mut handshake = Vec::new();
        write_response(&mut handshake, &create_response(&request)?)?;
        timeout(IO_TIMEOUT, stream.write_all(&handshake)).await??;
        let limits = WebSocketConfig {
            // Chrome's normal initialization can include >20 KiB of debugger patterns.
            max_message_size: Some(128 * 1024),
            max_frame_size: Some(128 * 1024),
            write_buffer_size: 0,
            max_write_buffer_size: 32768,
            ..Default::default()
        };
        let socket =
            WebSocketStream::from_partially_read(stream, pending, Role::Server, Some(limits)).await;
        inspect(socket).await?;
    } else {
        let ws = format!("ws://{addr}{TARGET}");
        let body = match path {
            "/json" | "/json/list" => json!([{
                "id": "explorer", "type": "page", "title": "Decentraland Explorer Network",
                "url":"about:blank", "webSocketDebuggerUrl": ws,
                "devtoolsFrontendUrl":format!("devtools://devtools/bundled/inspector.html?panel=network&ws={addr}{TARGET}")
            }]),
            "/json/version" => json!({"Browser":"Decentraland Explorer", "Protocol-Version":"1.3"}),
            _ => return reply(&mut stream, "404 Not Found", "").await,
        };
        reply(&mut stream, "200 OK", &body.to_string()).await?;
    }
    Ok(())
}

async fn reply(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let message = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
    timeout(IO_TIMEOUT, stream.write_all(message.as_bytes())).await??;
    Ok(())
}

async fn next_event(receiver: &mut Option<broadcast::Receiver<String>>) -> Result<String> {
    match receiver {
        Some(rx) => Ok(rx.recv().await?),
        None => std::future::pending().await,
    }
}

async fn inspect(mut socket: WebSocketStream<TcpStream>) -> Result<()> {
    let mut receiver = None;
    loop {
        let reply = tokio::select! {
            event = next_event(&mut receiver) => event?, // detach a lagging inspector
            incoming = socket.next() => {
                let Some(message) = incoming else { break };
                let command = match message? {
                    Message::Text(text) => serde_json::from_str::<Value>(&text)?,
                    Message::Close(_) => break,
                    Message::Ping(_) => { timeout(IO_TIMEOUT, socket.flush()).await??; continue; }
                    _ => continue,
                };
                let Some(id) = command["id"].as_u64() else { continue };
                match command["method"].as_str() {
                    Some("Network.enable") => {
                        receiver.get_or_insert_with(|| REPORTER.get().unwrap().events.subscribe());
                        json!({"id":id,"result":{}})
                    },
                    Some("Network.disable") => {
                        receiver = None;
                        json!({"id":id,"result":{}})
                    },
                    _ => json!({"id":id, "error":{"code":-32601,"message":"Method not supported"}}),
                }.to_string()
            }
        };
        timeout(IO_TIMEOUT, socket.send(Message::Text(reply))).await??;
    }
    Ok(())
}

// Keep the normalized path useful for local debugging, including individual content hashes.
// Paths may contain sensitive identifiers; userinfo, query and fragment are never emitted.
fn public_url(url: &reqwest::Url) -> String {
    url.origin()
        .ascii_serialization()
        .chars()
        .chain(url.path().chars())
        .take(MAX_URL_CHARS)
        .collect()
}

pub(crate) struct Trace {
    reporter: &'static Reporter,
    id: String,
    url: String,
    done: bool,
}

impl Trace {
    pub(crate) fn start(req: &reqwest::Request) -> Option<Self> {
        let reporter = REPORTER.get()?;
        if reporter.events.receiver_count() == 0 {
            return None;
        }
        let trace = Self {
            reporter,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string(),
            url: public_url(req.url()),
            done: false,
        };
        let method = match req.method().as_str() {
            m @ ("GET" | "POST" | "PUT" | "HEAD" | "PATCH" | "DELETE" | "OPTIONS") => m,
            _ => "OTHER",
        };
        trace.emit("requestWillBeSent", json!({
            "loaderId":"explorer","documentURL":trace.url, "type":"Fetch",
            "wallTime":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64(),
            "request":{"url":trace.url,"method":method,"headers":{},"initialPriority":"Medium","referrerPolicy":"no-referrer"},
            "initiator":{"type":"other"}
        }));
        Some(trace)
    }

    fn emit(&self, event: &str, mut params: Value) {
        params["requestId"] = json!(self.id);
        params["timestamp"] = json!(self.reporter.started.elapsed().as_secs_f64());
        let _ = self
            .reporter
            .events
            .send(json!({"method":format!("Network.{event}"), "params":params}).to_string());
    }

    pub(crate) fn response(&self, response: &reqwest::Response) {
        self.emit("responseReceived", json!({
            "loaderId":"explorer", "type":"Fetch",
            "response":{"url":public_url(response.url()),"status":response.status().as_u16(),
            "statusText":response.status().canonical_reason().unwrap_or(""), "headers":{},"mimeType":"",
            "charset":"","connectionReused":false,"connectionId":0,"encodedDataLength":0,
            "securityState":"unknown"}
        }));
    }

    pub(crate) fn finish(&mut self, size: u64) {
        self.emit("loadingFinished", json!({"encodedDataLength":size}));
        self.done = true;
    }

    pub(crate) fn fail(&mut self, reason: &str) {
        self.emit("loadingFailed", json!({"type":"Fetch","errorText":reason}));
        self.done = true;
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if !self.done {
            self.fail("Request cancelled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn display_urls_preserve_normalized_paths_without_private_url_parts() {
        for (raw, expected) in [
            (
                "https://secret-user:secret-password@EXAMPLE.test:443/peer/content/bafy-first?token=secret-query#secret-fragment",
                "https://example.test/peer/content/bafy-first",
            ),
            (
                "https://example.test/peer/content/bafy-second",
                "https://example.test/peer/content/bafy-second",
            ),
            (
                "https://example.test/old/../profile-images/avatar%20one.png?secret-query#secret-fragment",
                "https://example.test/profile-images/avatar%20one.png",
            ),
            (
                "http://secret-user:secret-password@[::1]:8080/scenes/world-main?secret-query#secret-fragment",
                "http://[::1]:8080/scenes/world-main",
            ),
        ] {
            assert_eq!(public_url(&raw.parse().unwrap()), expected);
        }
    }

    #[test]
    fn display_urls_are_bounded_after_redaction() {
        let origin = "https://example.test/";
        for length in [4095, 4096, 4097] {
            let path = "a".repeat(length - origin.len());
            let url = format!("{origin}{path}?secret-query#secret-fragment")
                .parse()
                .unwrap();
            let displayed = public_url(&url);
            assert_eq!(displayed.chars().count(), length.min(4096));
            assert_eq!(
                displayed,
                format!("{origin}{}", "a".repeat(length.min(4096) - origin.len()))
            );
        }
        let url = format!("{origin}{}?secret-query", "é/".repeat(4096))
            .parse()
            .unwrap();
        let displayed = public_url(&url);
        assert_eq!(displayed.chars().count(), 4096);
        assert!(displayed.starts_with("https://example.test/%C3%A9/"));
        assert!(!displayed.contains("secret-"));
    }

    #[test]
    fn inspector_discovers_and_observes_real_requests() {
        let addr = start(0).unwrap();
        let http = |path: &str, host: String, origin: &str| {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {host}\r\n{origin}Connection: close\r\n\r\n"
            )
            .unwrap();
            let mut text = String::new();
            stream.read_to_string(&mut text).unwrap();
            text
        };
        let discovery = http("/json/list", addr.to_string(), "");
        let list: Value =
            serde_json::from_str(discovery.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(http("/json", "external.test".into(), "").starts_with("HTTP/1.1 403"));
        assert!(http(
            "/json",
            addr.to_string(),
            "Origin: https://evil.example\r\n"
        )
        .starts_with("HTTP/1.1 403"));
        let target = list[0]["webSocketDebuggerUrl"].as_str().unwrap();
        use tungstenite::client::IntoClientRequest;
        let mut bad = target.into_client_request().unwrap();
        bad.headers_mut()
            .insert("origin", "https://evil.example".parse().unwrap());
        assert!(tungstenite::connect(bad).is_err());
        let mut good = target.into_client_request().unwrap();
        good.headers_mut()
            .insert("origin", "devtools://devtools".parse().unwrap());
        let (mut ws, _) = tungstenite::connect(good).unwrap();
        if let tungstenite::stream::MaybeTlsStream::Plain(tcp) = ws.get_mut() {
            tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        }
        ws.send(Message::Text(
            r#"{"id":1,"method":"Network.enable"}"#.into(),
        ))
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&ws.read().unwrap().into_text().unwrap()).unwrap(),
            json!({"id":1,"result":{}})
        );
        // The Chrome frontend sends sizeable initial configuration messages. Unsupported
        // commands get an error and leave the observer connected.
        for command in [
            json!({"id":2,"method":"Runtime.evaluate"}),
            json!({"id":3,"method":"Debugger.setBlackboxPatterns","params":{"patterns":["x".repeat(22000)]}}),
        ] {
            let notification_request = reqwest::Request::new(
                reqwest::Method::GET,
                format!("http://{addr}/command-notification/{}", command["id"])
                    .parse()
                    .unwrap(),
            );
            drop(Trace::start(&notification_request).unwrap());
            // Queue a notification before the reply, as concurrent HTTP requests can do.
            if let tungstenite::stream::MaybeTlsStream::Plain(tcp) = ws.get_mut() {
                assert!(tcp.peek(&mut [0; 1]).unwrap() > 0);
            }
            ws.send(Message::Text(command.to_string())).unwrap();
            let response = loop {
                let message: Value =
                    serde_json::from_str(&ws.read().unwrap().into_text().unwrap()).unwrap();
                if message["id"] == command["id"] {
                    break message;
                }
                assert!(message["id"].is_null(), "{message}");
            };
            assert_eq!(response["error"]["code"], -32601);
            assert_eq!(response["error"]["message"], "Method not supported");
        }
        let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = fixture.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = fixture.accept().unwrap();
            let mut buf = [0; 2048];
            stream.read(&mut buf).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nX-Private: secret-response-header\r\nConnection: close\r\n\r\nsecret-body").unwrap();
        });
        struct Granted;
        impl crate::http::HttpAttemptPermit for Granted {}
        let run = || {
            let client = reqwest::Client::new();
            let req = client
                .get(format!(
                    "http://secret-user:secret-password@{origin}/content/bafy-first?token=secret-query#secret-fragment"
                ))
                .header("authorization", "secret-header")
                .build()
                .unwrap();
            async_std::task::block_on(async_compat::Compat::new(crate::http::fetch_with_permit(
                Granted,
                req,
                |req| client.execute(req),
                Duration::from_secs(3),
                Duration::from_secs(3),
                |_| {},
            )))
        };
        assert!(run().is_ok());
        server.join().unwrap();
        assert!(run().is_err());
        // Other transport tests can emit events while this process-wide inspector is attached.
        let mut own_requests = std::collections::HashSet::new();
        let mut next_event = |url: &str| loop {
            let raw = ws.read().unwrap().into_text().unwrap();
            let event: Value = serde_json::from_str(&raw).unwrap();
            let params = &event["params"];
            let id = params["requestId"].as_str().unwrap().to_owned();
            if params["request"]["url"].as_str() == Some(url) {
                own_requests.insert(id.clone());
            }
            if own_requests.contains(&id) {
                break (raw, event);
            }
        };
        let mut previous = 0.0;
        let mut ids = Vec::new();
        for method in [
            "requestWillBeSent",
            "responseReceived",
            "loadingFinished",
            "requestWillBeSent",
            "loadingFailed",
        ] {
            let (raw, event) = next_event(&format!("http://{origin}/content/bafy-first"));
            assert!(
                !raw.contains("secret-") && !raw.contains("authorization"),
                "{raw}"
            );
            assert_eq!(event["method"], format!("Network.{method}"));
            let params = &event["params"];
            let now = params["timestamp"].as_f64().unwrap();
            assert!(now >= previous);
            previous = now;
            ids.push(params["requestId"].as_str().unwrap().to_owned());
            match method {
                "requestWillBeSent" => {
                    assert!(params["wallTime"].as_f64().unwrap() > 1_600_000_000.0);
                    assert_eq!(
                        params["request"]["url"],
                        format!("http://{origin}/content/bafy-first")
                    );
                    assert_eq!(params["request"]["headers"], json!({}));
                }
                "responseReceived" => {
                    assert_eq!(params["response"]["status"], 200);
                    assert_eq!(
                        params["response"]["url"],
                        format!("http://{origin}/content/bafy-first")
                    );
                    assert_eq!(params["response"]["headers"], json!({}));
                }
                "loadingFinished" => assert_eq!(params["encodedDataLength"], 11),
                _ => {}
            }
        }
        assert_eq!(ids[0], ids[2]);
        assert_ne!(ids[0], ids[3]);
        assert_eq!(ids[3], ids[4]);

        let cancelled_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let cancelled_url = format!(
            "http://{}/contents/cancelled-body",
            cancelled_server.local_addr().unwrap()
        );
        let (finish, resume) = std::sync::mpsc::channel::<()>();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = cancelled_server.accept().unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nb")
                .unwrap();
            let _ = resume.recv();
        });
        async_std::task::block_on(async_compat::Compat::new(async {
            let client = reqwest::Client::new();
            let (chunks, first_chunk) = async_std::channel::bounded(1);
            let mut fetch = Box::pin(crate::http::fetch_with_permit(
                Granted,
                client.get(&cancelled_url).build().unwrap(),
                |request| client.execute(request),
                Duration::from_secs(5),
                Duration::from_secs(5),
                |_| {
                    let _ = chunks.try_send(());
                },
            ));
            let ready = timeout(
                Duration::from_secs(5),
                futures_util::future::select(Box::pin(first_chunk.recv()), &mut fetch),
            )
            .await
            .unwrap();
            assert!(matches!(
                ready,
                futures_util::future::Either::Left((Ok(()), _))
            ));
            drop(fetch);
        }));
        drop(finish);
        server.join().unwrap();
        let mut cancelled_id = None;
        for method in ["requestWillBeSent", "responseReceived", "loadingFailed"] {
            let (_, event) = next_event(&cancelled_url);
            assert_eq!(event["method"], format!("Network.{method}"));
            let params = &event["params"];
            let id = params["requestId"].as_str().unwrap();
            assert_eq!(
                cancelled_id.get_or_insert_with(|| id.to_owned()).as_str(),
                id
            );
            if method == "loadingFailed" {
                assert_eq!(params["errorText"], "Request cancelled");
            }
        }

        let json_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let json_url = format!(
            "http://{}/scenes/world-main",
            json_server.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = json_server.accept().unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).unwrap();
            let body = r#"{"value":"secret-json-body"}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nX-Private: secret-response-header\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        async_std::task::block_on(async_compat::Compat::new(async {
            let response = crate::TracedJsonResponse::send(
                reqwest::Client::new()
                    .post(format!("{json_url}?secret-query#secret-fragment"))
                    .header("x-identity-auth-chain-1", "secret-header"),
            )
            .await
            .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            assert_eq!(
                response.json::<Value>().await.unwrap(),
                json!({"value":"secret-json-body"})
            );
        }));
        server.join().unwrap();
        for method in ["requestWillBeSent", "responseReceived", "loadingFinished"] {
            let (raw, event) = next_event(&json_url);
            assert!(!raw.contains("secret-") && !raw.contains("identity-auth"));
            assert_eq!(event["method"], format!("Network.{method}"));
            if method == "responseReceived" {
                assert_eq!(event["params"]["response"]["url"], json_url);
                assert_eq!(event["params"]["response"]["headers"], json!({}));
            }
        }

        // A redirect keeps the initial request identity and reports the final public path.
        // Neither URL's query, fragment or userinfo, nor either response's headers, is exposed.
        let redirect_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let redirect_origin = redirect_server.local_addr().unwrap();
        let initial_url = format!("http://{redirect_origin}/contents/bafy-initial");
        let final_url = format!("http://{redirect_origin}/contents/bafy-final");
        let server = std::thread::spawn(move || {
            for redirected in [false, true] {
                let (mut stream, _) = redirect_server.accept().unwrap();
                let mut request = [0; 2048];
                stream.read(&mut request).unwrap();
                if redirected {
                    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nX-Private: secret-final-header\r\nConnection: close\r\n\r\nsecret-body").unwrap();
                } else {
                    write!(stream, "HTTP/1.1 302 Found\r\nLocation: http://secret-user:secret-password@{redirect_origin}/contents/bafy-final?token=secret-redirect-query#secret-redirect-fragment\r\nX-Private: secret-redirect-header\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                }
            }
        });
        let response = async_std::task::block_on(async_compat::Compat::new(crate::fetch(
            reqwest::Client::new()
                .get(format!("{initial_url}?token=secret-query#secret-fragment"))
                .header("authorization", "secret-header"),
            Duration::from_secs(3),
            Duration::from_secs(3),
        )))
        .ok()
        .unwrap();
        assert_eq!(response.body, b"secret-body");
        server.join().unwrap();
        for method in ["requestWillBeSent", "responseReceived", "loadingFinished"] {
            let (raw, event) = next_event(&initial_url);
            assert!(
                !raw.contains("secret-") && !raw.contains("authorization"),
                "{raw}"
            );
            assert_eq!(event["method"], format!("Network.{method}"));
            match method {
                "requestWillBeSent" => {
                    assert_eq!(event["params"]["request"]["url"], initial_url);
                    assert_eq!(event["params"]["documentURL"], initial_url);
                }
                "responseReceived" => {
                    assert_eq!(event["params"]["response"]["url"], final_url);
                    assert_eq!(event["params"]["response"]["status"], 200);
                    assert_eq!(event["params"]["response"]["headers"], json!({}));
                }
                "loadingFinished" => assert_eq!(event["params"]["encodedDataLength"], 11),
                _ => unreachable!(),
            }
        }

        // Bodies can fail during transfer or after a complete transfer, during JSON decoding.
        for (status, declared, body, terminal) in [
            (200, 20, "secret-body", "loadingFailed"),
            (200, 16, "secret-json-body", "loadingFinished"),
            (404, 99, "", "loadingFinished"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let target = format!("http://{}/about", listener.local_addr().unwrap());
            let serving = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0; 1024];
                stream.read(&mut buf).unwrap();
                write!(stream, "HTTP/1.1 {status} Reply\r\nContent-Length: {declared}\r\nX-Private: secret-response-header\r\nConnection: close\r\n\r\n{body}").unwrap();
            });
            let failed = async_std::task::block_on(async_compat::Compat::new(async {
                let response = crate::TracedJsonResponse::send(
                    reqwest::Client::new()
                        .get(format!("{target}?token=secret-query#secret-fragment"))
                        .header("authorization", "secret-header"),
                )
                .await
                .unwrap();
                if response.status().as_u16() == 404 {
                    return response.discard().as_u16() == 404;
                }
                response.json::<Value>().await.is_err()
            }));
            assert!(failed);
            serving.join().unwrap();
            for method in ["requestWillBeSent", "responseReceived", terminal] {
                let (raw, event) = next_event(&target);
                assert!(
                    !raw.contains("secret-") && !raw.contains("authorization"),
                    "{raw}"
                );
                assert_eq!(event["method"], format!("Network.{method}"));
                if method == "loadingFinished" {
                    assert_eq!(event["params"]["encodedDataLength"], body.len());
                }
            }
        }

        // A stalled subscriber has fixed capacity and cannot backpressure engine requests.
        let events = &REPORTER.get().unwrap().events;
        let mut slow = events.subscribe();
        for _ in 0..(EVENT_CAPACITY + 1) {
            events.send(String::new()).unwrap();
        }
        assert!(matches!(
            slow.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(_))
        ));
        drop(ws);
        assert!(http("/json/version", addr.to_string(), "").contains("Protocol-Version"));
    }
}

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
const IO_TIMEOUT: Duration = Duration::from_secs(3);
static REPORTER: OnceLock<Reporter> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct Reporter {
    started: Instant,
    events: broadcast::Sender<String>,
}

/// Allocate no socket or queue unless explicitly requested at native startup.
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

// Never retain raw request metadata. Even path components may be bearer tokens.
// Reveal only known static route labels, stopping at the first dynamic segment.
fn public_url(url: &reqwest::Url) -> String {
    let mut label = url.origin().ascii_serialization();
    if label.len() > 256 {
        return "https://redacted.invalid/".into();
    }
    for part in url.path().split('/').skip(1) {
        label.push('/');
        match part {
            ""
            | "content"
            | "contents"
            | "entities"
            | "active"
            | "about"
            | "places"
            | "lambdas"
            | "profiles"
            | "main"
            | "realm-provider-ea"
            | "profile-images"
            | "asset-bundle-registry"
            | "comms-gatekeeper"
            | "camera-reel-service"
            | "worlds-content-server"
            | "social-api"
            | "auth-api"
            | "api"
            | "v1"
            | "v2" => label.push_str(part),
            _ => {
                label.push('…');
                break;
            }
        }
        if label.len() > 512 {
            break;
        }
    }
    label
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
        ws.send(Message::Text(
            r#"{"id":2,"method":"Runtime.evaluate"}"#.into(),
        ))
        .unwrap();
        assert!(ws.read().unwrap().into_text().unwrap().contains("-32601"));
        // The Chrome frontend sends sizeable initial configuration messages. Unsupported
        // commands get an error and leave the observer connected.
        ws.send(Message::Text(json!({"id":3,"method":"Debugger.setBlackboxPatterns","params":{"patterns":["x".repeat(22000)]}}).to_string())).unwrap();
        assert!(ws.read().unwrap().into_text().unwrap().contains("-32601"));
        let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = fixture.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = fixture.accept().unwrap();
            let mut buf = [0; 2048];
            stream.read(&mut buf).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nsecret-body").unwrap();
        });
        struct Granted;
        impl crate::http::HttpAttemptPermit for Granted {}
        let run = || {
            let client = reqwest::Client::new();
            let req = client
                .get(format!(
                    "http://{origin}/content/secret-path?token=secret-query"
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
            let (raw, event) = next_event(&format!("http://{origin}/content/…"));
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
                        format!("http://{origin}/content/…")
                    );
                    assert_eq!(params["request"]["headers"], json!({}));
                }
                "responseReceived" => assert_eq!(params["response"]["status"], 200),
                "loadingFinished" => assert_eq!(params["encodedDataLength"], 11),
                _ => {}
            }
        }
        assert_eq!(ids[0], ids[2]);
        assert_ne!(ids[0], ids[3]);
        assert_eq!(ids[3], ids[4]);

        let json_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let json_url = format!("http://{}/about", json_server.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = json_server.accept().unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{ \"ok\": true }").unwrap();
        });
        async_std::task::block_on(async_compat::Compat::new(async {
            let response = crate::TracedJsonResponse::send(
                reqwest::Client::new()
                    .post(format!("{json_url}?secret-query"))
                    .header("x-identity-auth-chain-1", "secret-header"),
            )
            .await
            .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            assert_eq!(response.json::<Value>().await.unwrap(), json!({"ok":true}));
        }));
        server.join().unwrap();
        for method in ["requestWillBeSent", "responseReceived", "loadingFinished"] {
            let (raw, event) = next_event(&json_url);
            assert!(!raw.contains("secret-") && !raw.contains("identity-auth"));
            assert_eq!(event["method"], format!("Network.{method}"));
        }

        // Bodies can fail during transfer or after a complete transfer, during JSON decoding.
        for (status, declared, body, terminal) in [
            (200, 20, "{}", "loadingFailed"),
            (200, 3, "bad", "loadingFinished"),
            (404, 99, "", "loadingFinished"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let target = format!("http://{}/about", listener.local_addr().unwrap());
            let serving = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0; 1024];
                stream.read(&mut buf).unwrap();
                write!(stream, "HTTP/1.1 {status} Reply\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n{body}").unwrap();
            });
            let failed = async_std::task::block_on(async_compat::Compat::new(async {
                let response = crate::TracedJsonResponse::send(reqwest::Client::new().get(&target))
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
                let (_, event) = next_event(&target);
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

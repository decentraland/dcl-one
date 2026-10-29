//! A server in the test's own process, and a client that writes HTTP by hand so a test controls
//! every byte of a request: the Host header, a missing length, a body cut short.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use benchmark::access::Access;
use benchmark::byte_stream::ByteStream;
use benchmark::limits::Limits;
use benchmark::server::{self, EndSignal, Server, Shared};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// An empty folder for a test, under cargo's scratch space for integration tests.
pub fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("benchmark")
        .join(format!("{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The fake app, the crate's own test page.
pub fn fake_app() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fake-app")
}

pub struct TestServer {
    pub port: u16,
    pub shared: Arc<Shared>,
    pub dir: PathBuf,
    server: Option<Server>,
}

impl TestServer {
    pub async fn start(name: &str) -> Self {
        let dir = scratch(name);
        let root = dir.join("root");
        std::fs::create_dir_all(&root).unwrap();
        Self::start_with(dir, root, Limits::DEFAULT).await
    }

    pub async fn start_with(dir: PathBuf, root: PathBuf, limits: Limits) -> Self {
        let listeners = server::bind(0).await.unwrap();
        let port = listeners.port();
        let shared = Arc::new(Shared {
            access: Access::new(port),
            csv: ByteStream::new(dir.join("change_build.csv"), limits.file_bytes),
            log: ByteStream::new(dir.join("change_build.log"), limits.file_bytes),
            end: EndSignal::default(),
            missing: Default::default(),
            root,
            limits,
        });
        let server = Some(listeners.serve(shared.clone()));
        Self {
            port,
            shared,
            dir,
            server,
        }
    }

    pub fn token(&self) -> &str {
        self.shared.access.token()
    }

    /// `/__perf/<token>/<what>`
    pub fn perf(&self, what: &str) -> String {
        format!("{}/{what}", self.shared.access.stream_path())
    }

    pub fn host(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    pub fn csv_path(&self) -> PathBuf {
        self.dir.join("change_build.csv")
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("change_build.log")
    }

    /// The request with this server's Host, `Connection: close` and a `Content-Length` of the
    /// body.
    pub async fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Reply {
        let mut request = Request::new(method, path).header("Host", &self.host());
        for (name, value) in headers {
            request = request.header(name, value);
        }
        request.body(body).send(self.port).await
    }

    pub async fn post_chunk(&self, kind: &str, offset: &str, body: &[u8]) -> Reply {
        self.send(
            "POST",
            &self.perf(kind),
            &[
                ("X-Perf-Offset", offset),
                ("Content-Type", "application/octet-stream"),
            ],
            body,
        )
        .await
    }

    pub async fn post_end(&self, json: &str) -> Reply {
        self.send(
            "POST",
            &self.perf("end"),
            &[("Content-Type", "application/json")],
            json.as_bytes(),
        )
        .await
    }

    pub async fn stop(mut self) {
        if let Some(server) = self.server.take() {
            server.stop().await;
        }
    }
}

/// A request written byte by byte.
pub struct Request {
    head: Vec<u8>,
    body: Vec<u8>,
    length: Option<usize>,
    cut: Option<usize>,
}

impl Request {
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            head: format!("{method} {path} HTTP/1.1\r\n").into_bytes(),
            body: Vec::new(),
            length: None,
            cut: None,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.head.extend(format!("{name}: {value}\r\n").bytes());
        self
    }

    /// A body with its own `Content-Length`.
    pub fn body(mut self, body: &[u8]) -> Self {
        self.length = Some(body.len());
        self.body = body.to_vec();
        self
    }

    /// A body with no length: sent as one chunk of a chunked request.
    pub fn chunked(mut self, body: &[u8]) -> Self {
        self.head.extend(b"Transfer-Encoding: chunked\r\n");
        let mut framed = format!("{:x}\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed.extend(b"\r\n0\r\n\r\n");
        self.body = framed;
        self.length = None;
        self
    }

    /// Announces `length` bytes and sends only the first `sent` of `body`, then closes its end.
    pub fn short_body(mut self, body: &[u8], length: usize, sent: usize) -> Self {
        self.length = Some(length);
        self.body = body[..sent].to_vec();
        self.cut = Some(sent);
        self
    }

    pub async fn send(mut self, port: u16) -> Reply {
        if let Some(length) = self.length {
            self.head
                .extend(format!("Content-Length: {length}\r\n").bytes());
        }
        self.head.extend(b"Connection: close\r\n\r\n");

        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(&self.head).await.unwrap();
        // The server may answer and close before a large body is written out; that is its call.
        let _ = stream.write_all(&self.body).await;
        let _ = stream.flush().await;
        if self.cut.is_some() {
            let _ = stream.shutdown().await;
        }

        let mut raw = Vec::new();
        let _ = stream.read_to_end(&mut raw).await;
        Reply::parse(&raw)
    }
}

#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    fn parse(raw: &[u8]) -> Self {
        // A server that closes without answering is a status of 0.
        let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
            return Self {
                status: 0,
                headers: Vec::new(),
                body: raw.to_vec(),
            };
        };
        let head = String::from_utf8_lossy(&raw[..split]).into_owned();
        let mut lines = head.split("\r\n");
        let status = lines
            .next()
            .and_then(|line| line.split(' ').nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("no status in {head:?}"));
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
            .collect();
        Self {
            status,
            headers,
            body: raw[split + 4..].to_vec(),
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// The headers every response carries, whatever the request.
pub fn assert_isolation_headers(reply: &Reply) {
    for (name, value) in [
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-embedder-policy", "require-corp"),
        ("cross-origin-resource-policy", "cross-origin"),
        ("cache-control", "no-store"),
        ("content-security-policy", "frame-ancestors 'self'"),
        ("x-frame-options", "SAMEORIGIN"),
    ] {
        assert_eq!(
            reply.header(name),
            Some(value),
            "{name} on a {} response",
            reply.status
        );
    }
}

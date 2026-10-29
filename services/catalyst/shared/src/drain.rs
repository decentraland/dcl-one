//! A request answered before its body was read: a writes gate's 503, an auth
//! check's 401, a 404 or a 413. hyper closes a connection whose request body
//! is dropped before its end, and the kernel answers the bytes the client is
//! still sending with a reset. nginx buffers an upload and writes it upstream
//! after the answer is already there, so it fails the write (`Broken pipe`),
//! retries the next process, answers 502 and counts both down (`max_fails`).
//! [`drain_unread`] hands every body a handler drops early to a task that
//! reads and discards the rest, as nginx itself does with a body it refuses:
//! the answer goes out at once, the client finishes sending, and the
//! connection stays open for its next request.

use std::pin::Pin;
use std::task::{Context, Poll};

use axum::Router;
use axum::body::{Body, Bytes, HttpBody};
use axum::extract::Request;
use axum::http::header;
use axum::middleware;
use futures::StreamExt;
use http_body::{Frame, SizeHint};

/// Wraps every request `router` answers, fallbacks included. A body dropped
/// unread is drained in the background, up to `limit` more bytes; one
/// declaring more than that, or longer than that, closes the connection as
/// before.
pub fn drain_unread(router: Router, limit: u64) -> Router {
    router.layer(middleware::map_request(move |req: Request| async move {
        wrap(req, limit)
    }))
}

fn wrap(req: Request, limit: u64) -> Request {
    let awaits_continue = req
        .headers()
        .get(header::EXPECT)
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"100-continue"));
    req.map(|body| {
        Body::new(DrainOnDrop {
            body: Some(body),
            limit,
            awaits_continue,
            polled: false,
        })
    })
}

struct DrainOnDrop {
    body: Option<Body>,
    limit: u64,
    /// `Expect: 100-continue` and never polled: the client has not been told
    /// to send, so nothing is in flight to drain.
    awaits_continue: bool,
    polled: bool,
}

impl HttpBody for DrainOnDrop {
    type Data = Bytes;
    type Error = axum::BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        this.polled = true;
        match this.body.as_mut() {
            Some(body) => Pin::new(body)
                .poll_frame(cx)
                .map_err(axum::Error::into_inner),
            None => Poll::Ready(None),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.body.as_ref().is_none_or(HttpBody::is_end_stream)
    }

    fn size_hint(&self) -> SizeHint {
        self.body
            .as_ref()
            .map_or_else(|| SizeHint::with_exact(0), HttpBody::size_hint)
    }
}

impl Drop for DrainOnDrop {
    fn drop(&mut self) {
        let Some(body) = self.body.take() else {
            return;
        };
        if body.is_end_stream()
            || (self.awaits_continue && !self.polled)
            || body.size_hint().lower() > self.limit
        {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let limit = self.limit;
        runtime.spawn(async move {
            let mut rest = body.into_data_stream();
            let mut read = 0u64;
            while let Some(Ok(chunk)) = rest.next().await {
                read += chunk.len() as u64;
                if read > limit {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writes::{DataWrites, Gate, guard_with};
    use axum::extract::DefaultBodyLimit;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use axum::routing::{get, put};
    use std::io;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{TcpSocket, TcpStream};

    /// Far past what the kernel buffers between the two ends (both sockets
    /// pinned to `SOCKET_BUFFER`): the client is still writing when the
    /// server answers, as nginx is with a 2 MiB upload.
    const BODY: usize = 16 * 1024 * 1024;
    const SOCKET_BUFFER: u32 = 64 * 1024;
    const LIMIT: u64 = 32 * 1024 * 1024;
    /// A hang guard only: every exchange here ends in milliseconds.
    const HANG_GUARD: Duration = Duration::from_secs(60);

    fn refuse(_: &crate::writes::Refusal) -> Response {
        (StatusCode::SERVICE_UNAVAILABLE, "writes are off").into_response()
    }

    const OFF: Gate = Gate {
        enabled: false,
        open_while_disabled: &[],
        refuse,
        blobs: false,
    };

    fn uploads() -> Router {
        Router::new()
            .route(
                "/manifest",
                put(|body: Bytes| async move { body.len().to_string() })
                    .layer(DefaultBodyLimit::max(BODY)),
            )
            .route("/ping", get(|| async { "pong" }))
    }

    /// Serves `app` the way `serve` does, on a listener whose accepted
    /// sockets inherit the small receive buffer.
    async fn serve(app: Router) -> SocketAddr {
        let socket = TcpSocket::new_v4().unwrap();
        socket.set_recv_buffer_size(SOCKET_BUFFER).unwrap();
        socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let listener = socket.listen(16).unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, drain_unread(app, LIMIT)).await });
        addr
    }

    async fn connect(addr: SocketAddr) -> BufReader<TcpStream> {
        let socket = TcpSocket::new_v4().unwrap();
        socket.set_send_buffer_size(SOCKET_BUFFER).unwrap();
        BufReader::new(socket.connect(addr).await.unwrap())
    }

    /// Writes the head and the whole `len`-byte body, as nginx does with a
    /// buffered upload, before reading anything.
    async fn send(conn: &mut BufReader<TcpStream>, head: &str, len: usize) -> io::Result<()> {
        let stream = conn.get_mut();
        stream.write_all(head.as_bytes()).await?;
        let chunk = vec![0u8; 64 * 1024];
        let mut left = len;
        while left > 0 {
            let n = left.min(chunk.len());
            stream.write_all(&chunk[..n]).await?;
            left -= n;
        }
        Ok(())
    }

    /// One response off the connection: its status and body.
    async fn response(conn: &mut BufReader<TcpStream>) -> io::Result<(u16, String)> {
        let mut line = String::new();
        conn.read_line(&mut line).await?;
        let status = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| io::Error::other(format!("status line {line:?}")))?;
        let mut length = 0;
        loop {
            line.clear();
            conn.read_line(&mut line).await?;
            if line.trim_end().is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().map_err(io::Error::other)?;
            }
        }
        let mut body = vec![0; length];
        conn.read_exact(&mut body).await?;
        Ok((status, String::from_utf8_lossy(&body).into_owned()))
    }

    /// PUTs `BODY` bytes to `path`, reads the answer, then asks `/ping` on
    /// the same connection: the refusal left it orderly, not reset.
    async fn upload_then_ping(addr: SocketAddr, path: &str) -> (u16, String) {
        tokio::time::timeout(HANG_GUARD, async {
            let mut conn = connect(addr).await;
            let head = format!(
                "PUT {path} HTTP/1.1\r\nhost: node\r\ncontent-type: application/octet-stream\r\ncontent-length: {BODY}\r\n\r\n"
            );
            send(&mut conn, &head, BODY)
                .await
                .expect("the server reads the whole body: no reset, no broken pipe");
            let answer = response(&mut conn).await.expect("the answer arrives intact");
            send(&mut conn, "GET /ping HTTP/1.1\r\nhost: node\r\n\r\n", 0)
                .await
                .expect("the connection stays open");
            let ping = response(&mut conn).await.expect("the next request is answered");
            assert_eq!(ping, (200, "pong".to_string()));
            answer
        })
        .await
        .expect("hang guard")
    }

    #[tokio::test]
    async fn a_writes_gate_refusal_lets_the_upload_finish_and_keeps_the_connection() {
        let addr = serve(guard_with(uploads(), OFF)).await;
        assert_eq!(
            upload_then_ping(addr, "/manifest").await,
            (503, "writes are off".to_string())
        );
    }

    #[tokio::test]
    async fn the_data_writes_refusal_lets_the_upload_finish_and_keeps_the_connection() {
        let addr = serve(DataWrites::guard(uploads(), false)).await;
        let (status, body) = upload_then_ping(addr, "/manifest").await;
        assert_eq!(status, 503);
        assert!(body.contains(DataWrites::DISABLED_MESSAGE), "{body}");
    }

    #[tokio::test]
    async fn a_not_found_or_an_over_limit_upload_is_drained_too() {
        let app = uploads().route(
            "/small",
            put(|body: Bytes| async move { body.len().to_string() })
                .layer(DefaultBodyLimit::max(1024 * 1024)),
        );
        let addr = serve(app).await;
        assert_eq!(upload_then_ping(addr, "/nowhere").await.0, 404);
        assert_eq!(upload_then_ping(addr, "/small").await.0, 413);
        assert_eq!(
            upload_then_ping(addr, "/manifest").await,
            (200, BODY.to_string())
        );
    }

    #[tokio::test]
    async fn a_client_awaiting_100_continue_is_refused_without_being_asked_to_send() {
        let addr = serve(guard_with(uploads(), OFF)).await;
        tokio::time::timeout(HANG_GUARD, async {
            let mut conn = connect(addr).await;
            let head = format!(
                "PUT /manifest HTTP/1.1\r\nhost: node\r\nexpect: 100-continue\r\ncontent-length: {BODY}\r\n\r\n"
            );
            send(&mut conn, &head, 0).await.unwrap();
            assert_eq!(
                response(&mut conn).await.unwrap(),
                (503, "writes are off".to_string())
            );
            let mut rest = Vec::new();
            conn.read_to_end(&mut rest).await.unwrap();
            assert!(rest.is_empty(), "{:?}", String::from_utf8_lossy(&rest));
        })
        .await
        .expect("hang guard");
    }

    #[tokio::test]
    async fn a_body_declared_past_the_limit_is_not_drained() {
        let addr = serve(guard_with(uploads(), OFF)).await;
        let over = LIMIT as usize + 1;
        tokio::time::timeout(HANG_GUARD, async {
            let mut conn = connect(addr).await;
            let head =
                format!("PUT /manifest HTTP/1.1\r\nhost: node\r\ncontent-length: {over}\r\n\r\n");
            send(&mut conn, &head, over)
                .await
                .expect_err("the server stops reading and closes");
        })
        .await
        .expect("hang guard");
    }
}

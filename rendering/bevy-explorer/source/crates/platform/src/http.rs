use std::{future::Future, time::Duration};

use futures_util::{
    future::{select, Either},
    pin_mut, StreamExt,
};

/// Internal marker: the [`with_timeout`] timer fired before the future resolved.
struct Elapsed;

/// Race `fut` against a `dur` timer, returning `Err(Elapsed)` if the timer wins.
///
/// The timer uses `async-std` on native and `setTimeout` on WASM.
async fn with_timeout<F: Future>(dur: Duration, fut: F) -> Result<F::Output, Elapsed> {
    let timer = async_std::task::sleep(dur);
    pin_mut!(fut, timer);
    match select(fut, timer).await {
        Either::Left((out, _)) => Ok(out),
        Either::Right(_) => Err(Elapsed),
    }
}

/// A successfully-fetched response: its headers and fully-read body.
pub struct FetchedResponse {
    pub headers: reqwest::header::HeaderMap,
    pub body: Vec<u8>,
}

/// Why a [`fetch`] failed. `E` is the error type of the supplied send future,
/// `reqwest::Error` for a bare `RequestBuilder::send`, or e.g. `anyhow::Error` when
/// the send is routed through a wrapper.
pub enum FetchError<E> {
    /// Timed out awaiting the response headers (connect + time-to-first-byte).
    Headers,
    /// The send itself failed before any response was obtained.
    Send(E),
    /// The server returned a non-success status; the body is not read.
    Status(reqwest::StatusCode, reqwest::header::HeaderMap),
    /// The body transfer stalled because no chunk arrived within the idle window.
    Stalled,
    /// The body stream errored mid-transfer.
    Body(reqwest::Error),
}

/// A grant issued by the scheduler that admitted this individual attempt.
pub trait HttpAttemptPermit {}

/// Buffer an already-admitted attempt and count each chunk while the permit is held.
pub async fn fetch_with_permit<P, E, F>(
    _permit: P,
    send: impl FnOnce() -> F,
    headers_timeout: Duration,
    idle_timeout: Duration,
    on_chunk: impl Fn(usize),
) -> Result<FetchedResponse, FetchError<E>>
where
    P: HttpAttemptPermit,
    F: Future<Output = Result<reqwest::Response, E>>,
{
    fetch_observed(send(), headers_timeout, idle_timeout, on_chunk).await
}

/// Fetch with separate timeouts for headers and inactivity between body chunks.
pub async fn fetch<E>(
    send: impl Future<Output = Result<reqwest::Response, E>>,
    headers_timeout: Duration,
    idle_timeout: Duration,
) -> Result<FetchedResponse, FetchError<E>> {
    fetch_observed(send, headers_timeout, idle_timeout, |_| {}).await
}

async fn fetch_observed<E>(
    send: impl Future<Output = Result<reqwest::Response, E>>,
    headers_timeout: Duration,
    idle_timeout: Duration,
    on_chunk: impl Fn(usize),
) -> Result<FetchedResponse, FetchError<E>> {
    let response = match with_timeout(headers_timeout, send).await {
        Ok(result) => result.map_err(FetchError::Send)?,
        Err(Elapsed) => return Err(FetchError::Headers),
    };
    if !response.status().is_success() {
        return Err(FetchError::Status(
            response.status(),
            response.headers().clone(),
        ));
    }
    let headers = response.headers().clone();
    let mut stream = Box::pin(response.bytes_stream());
    let mut body = Vec::new();
    loop {
        match with_timeout(idle_timeout, stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                on_chunk(chunk.len());
                body.extend_from_slice(&chunk);
            }
            Ok(Some(Err(e))) => return Err(FetchError::Body(e)),
            Ok(None) => return Ok(FetchedResponse { headers, body }),
            Err(Elapsed) => return Err(FetchError::Stalled),
        }
    }
}

pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Parse a delay in seconds or an HTTP date. Callers decide whether the delay fits their deadline.
pub fn parse_retry_after(value: &str, now: std::time::SystemTime) -> Option<Duration> {
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return Some(Duration::from_secs(value.parse().unwrap_or(u64::MAX)));
    }
    Some(
        httpdate::parse_http_date(value)
            .ok()?
            .duration_since(now)
            .unwrap_or_default(),
    )
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod admission_tests {
    use super::*;
    use std::{
        cell::Cell,
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        time::Duration,
    };

    struct Grant<'a>(&'a Cell<bool>);

    impl HttpAttemptPermit for Grant<'_> {}

    impl Drop for Grant<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    fn paused_body(status: &str) -> (reqwest::RequestBuilder, mpsc::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let request =
            reqwest::Client::new().get(format!("http://{}", listener.local_addr().unwrap()));
        let (finish, resume) = mpsc::channel();
        let headers = format!("HTTP/1.1 {status}\r\nContent-Length: 4\r\n\r\nb");
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = socket.read(&mut request).unwrap();
            socket.write_all(headers.as_bytes()).unwrap();
            if resume.recv().is_ok() {
                let _ = socket.write_all(b"ody");
            }
        });
        (request, finish)
    }

    #[test]
    fn grant_is_held_through_body_read_and_released_on_cancellation() {
        async_std::task::block_on(async_compat::Compat::new(async {
            let limit = Duration::from_secs(5);
            for cancel in [false, true] {
                let released = Cell::new(false);
                let (request, finish) = paused_body("200 OK");
                let (chunks, first_chunk) = async_std::channel::bounded(1);
                let mut fetch = Box::pin(fetch_with_permit(
                    Grant(&released),
                    || request.send(),
                    limit,
                    limit,
                    |_| {
                        let _ = chunks.try_send(());
                    },
                ));
                let ready = select(Box::pin(first_chunk.recv()), &mut fetch).await;
                assert!(matches!(ready, Either::Left((Ok(()), _))));
                assert!(!released.get());
                if cancel {
                    drop(fetch);
                } else {
                    finish.send(()).unwrap();
                    assert_eq!(fetch.await.ok().unwrap().body, b"body");
                }
                assert!(released.get());
            }
        }));
    }

    #[test]
    fn cancellation_before_headers_drops_the_grant() {
        async_std::task::block_on(async {
            let released = Cell::new(false);
            let limit = Duration::from_secs(5);
            let mut waiting = Box::pin(fetch_with_permit(
                Grant(&released),
                || std::future::pending::<Result<reqwest::Response, ()>>(),
                limit,
                limit,
                |_| {},
            ));
            assert!(futures_util::poll!(&mut waiting).is_pending());
            drop(waiting);
            assert!(released.get());
        });
    }

    #[test]
    fn failed_sends_and_stalled_bodies_return_their_grants() {
        async_std::task::block_on(async_compat::Compat::new(async {
            let released = Cell::new(false);
            let limit = Duration::from_secs(5);
            let error = fetch_with_permit(
                Grant(&released),
                || async { Err::<reqwest::Response, _>(()) },
                limit,
                limit,
                |_| {},
            )
            .await;
            assert!(matches!(error, Err(FetchError::Send(()))));
            assert!(released.get());

            let headers = Duration::from_secs(5);
            let idle = Duration::from_millis(50);
            released.set(false);
            let result = fetch_with_permit(
                Grant(&released),
                || std::future::pending::<Result<reqwest::Response, ()>>(),
                idle,
                idle,
                |_| {},
            )
            .await;
            assert!(matches!(result, Err(FetchError::Headers)));
            assert!(released.get());

            released.set(false);
            let (request, _finish) = paused_body("200 OK");
            let result =
                fetch_with_permit(Grant(&released), || request.send(), headers, idle, |_| {}).await;
            assert!(matches!(result, Err(FetchError::Stalled)));
            assert!(released.get());

            released.set(false);
            let (request, finish) = paused_body("200 OK");
            drop(finish);
            let result = fetch_with_permit(
                Grant(&released),
                || request.send(),
                headers,
                headers,
                |_| {},
            )
            .await;
            assert!(matches!(result, Err(FetchError::Body(_))));
            assert!(released.get());
        }));
    }

    #[test]
    fn non_success_preserves_retry_after_header() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).unwrap();
            stream.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 5\r\nContent-Length: 0\r\n\r\n").unwrap();
        });
        let error = async_std::task::block_on(async_compat::Compat::new(fetch_with_permit(
            Grant(&Cell::new(false)),
            || {
                reqwest::Client::new()
                    .get(format!("http://{address}"))
                    .send()
            },
            Duration::from_secs(5),
            Duration::from_secs(5),
            |_| {},
        )));
        match error {
            Err(FetchError::Status(status, headers)) => {
                assert_eq!(status.as_u16(), 429);
                assert_eq!(headers.get("retry-after").unwrap(), "5");
            }
            _ => panic!("expected HTTP status and headers"),
        }
        server.join().unwrap();
    }

    #[test]
    fn retry_after_obeys_server_delay_and_limit() {
        let now = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let bounded_delay = |s, now| parse_retry_after(s, now).filter(|d| *d <= MAX_RETRY_AFTER);
        assert_eq!(bounded_delay("5", now), Some(Duration::from_secs(5)));
        assert_eq!(bounded_delay("61", now), None);
        assert_eq!(bounded_delay("invalid", now), None);
        assert_eq!(bounded_delay("Wed, 31 Dec 1969 23:59:59 -0000", now), None);
        assert_eq!(bounded_delay("Wed, 31 Dec 1969 23:59:59 GMT", now), None);
        assert_eq!(
            bounded_delay("Thu, 01 Jan 1970 00:00:00 GMT", now),
            Some(Duration::ZERO)
        );
        assert_eq!(
            bounded_delay("Mon, 12 Jan 1970 13:47:10 GMT", now),
            Some(Duration::from_secs(30))
        );
    }
}

//! The capped body reader. Edition-2021 clean: dcl-scene-cli compiles it by path.

use anyhow::{Result, anyhow};
use futures::StreamExt;
use tokio::time::{Duration, Instant};

const MIN_TRANSFER_BYTES_PER_SECOND: u64 = 4 * 1024;
const TRANSFER_RATE_WINDOW: Duration = Duration::from_secs(60);

/// Checks throughput in successive read windows. Count wire bytes before decompression
/// and exclude time the consumer spends processing the body.
#[derive(Default)]
pub struct TransferRateGuard {
    read_time: Duration,
    bytes_read: u64,
}

impl TransferRateGuard {
    #[allow(
        clippy::cast_precision_loss,
        reason = "byte counts round only above 2^53 bytes, which no transfer reaches"
    )]
    pub fn check(&mut self, chunk_bytes: u64, last_read: Duration) -> Result<()> {
        self.read_time += last_read;
        self.bytes_read = self.bytes_read.saturating_add(chunk_bytes);
        if self.read_time <= TRANSFER_RATE_WINDOW {
            return Ok(());
        }

        let seconds = self.read_time.as_secs_f64();
        let bytes_read = self.bytes_read;
        if (bytes_read as f64) < seconds * MIN_TRANSFER_BYTES_PER_SECOND as f64 {
            return Err(anyhow!(
                "transfer of {bytes_read} bytes over {seconds:.1}s fell below {MIN_TRANSFER_BYTES_PER_SECOND} bytes/s"
            ));
        }

        // A completed window cannot subsidize a later trickle.
        *self = Self::default();
        Ok(())
    }
}

/// The advertised `Content-Length` is checked first so an oversized body costs
/// nothing, but the running total is enforced too: a chunked or lying
/// upstream never gets to grow the buffer past the cap.
pub async fn read_body_capped(resp: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    read_body(resp, max_bytes, None).await
}

/// The capped reader with a throughput floor for peer sync responses.
pub async fn read_peer_body_capped(resp: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    read_body(resp, max_bytes, Some(TransferRateGuard::default())).await
}

async fn read_body(
    resp: reqwest::Response,
    max_bytes: usize,
    mut rate: Option<TransferRateGuard>,
) -> Result<Vec<u8>> {
    if let Some(len) = resp.content_length().filter(|len| *len > max_bytes as u64) {
        return Err(anyhow!(
            "response body advertises {len} bytes, over the {max_bytes} byte cap"
        ));
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    loop {
        let read_started = rate.as_ref().map(|_| Instant::now());
        let Some(chunk) = stream.next().await else {
            break;
        };
        // A body error names the URL; a configured upstream's path can carry
        // a gateway key (TRADE_RPC_URLS, ETH_RPC_URL), so it is stripped as
        // `SendError` strips it from a transport error.
        let chunk = chunk.map_err(|e| e.without_url())?;
        let bytes_read = buf.len().saturating_add(chunk.len());
        if bytes_read > max_bytes {
            return Err(anyhow!("response body exceeds the {max_bytes} byte cap"));
        }
        if let (Some(rate), Some(started)) = (&mut rate, read_started) {
            rate.check(chunk.len() as u64, started.elapsed())?;
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trickle_fails_after_the_grace_period() {
        let mut rate = TransferRateGuard::default();
        for _ in 0..3 {
            assert!(rate.check(1, Duration::from_secs(20)).is_ok());
        }
        let err = rate.check(1, Duration::from_secs(1)).unwrap_err();
        assert!(err.to_string().contains("fell below 4096 bytes/s"), "{err}");
    }

    #[test]
    fn a_transfer_at_the_rate_floor_can_continue() {
        let mut rate = TransferRateGuard::default();
        let time = Duration::from_secs(90);
        assert!(rate.check(4096 * time.as_secs(), time).is_ok());
    }

    #[test]
    fn an_initial_burst_cannot_subsidize_a_later_trickle() {
        let mut rate = TransferRateGuard::default();
        assert!(rate.check(1024 * 1024, Duration::ZERO).is_ok());
        for _ in 0..7 {
            assert!(rate.check(1, Duration::from_secs(16)).is_ok());
        }
        let error = rate.check(1, Duration::from_secs(16)).unwrap_err();
        assert!(error.to_string().contains("fell below 4096 bytes/s"));
    }

    #[tokio::test]
    async fn a_peer_response_body_is_aborted_when_it_trickles() {
        use axum::{Router, body::Body, routing::get};

        let (send, receive) = tokio::sync::mpsc::channel(2);
        let receive = std::sync::Arc::new(tokio::sync::Mutex::new(Some(receive)));
        let app = Router::new().route(
            "/",
            get(move || async move {
                let receive = receive.lock().await.take().unwrap();
                Body::from_stream(futures::stream::unfold(receive, |mut receive| async move {
                    receive
                        .recv()
                        .await
                        .map(|chunk| (Ok::<_, std::convert::Infallible>(chunk), receive))
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        send.send("first").await.unwrap();
        let response = reqwest::Client::new().get(url).send().await.unwrap();

        tokio::time::pause();
        let mut read = Box::pin(read_peer_body_capped(response, 1024));
        assert!(futures::poll!(&mut read).is_pending());
        tokio::time::advance(Duration::from_secs(61)).await;
        send.send("second").await.unwrap();
        drop(send);

        let error = read.await.unwrap_err();
        assert!(error.to_string().contains("fell below 4096 bytes/s"));
        server.abort();
    }
}

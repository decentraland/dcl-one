//! The capped body reader. Edition-2021 clean: dcl-scene-cli compiles it by path.

use anyhow::{Result, anyhow};
use futures::StreamExt;
use tokio::time::{Duration, Instant};

const MIN_TRANSFER_BYTES_PER_SECOND: u64 = 4 * 1024;
const TRANSFER_GRACE_PERIOD: Duration = Duration::from_secs(60);

/// Checks cumulative throughput after the grace period. Count wire bytes before any
/// decompression, and pass only time spent reading so local sinks can apply backpressure.
#[derive(Default)]
pub struct TransferRateGuard {
    read_time: Duration,
}

impl TransferRateGuard {
    pub fn check(&mut self, bytes_read: u64, last_read: Duration) -> Result<()> {
        self.read_time += last_read;
        let seconds = self.read_time.as_secs_f64();
        if self.read_time > TRANSFER_GRACE_PERIOD
            && (bytes_read as f64) < seconds * MIN_TRANSFER_BYTES_PER_SECOND as f64
        {
            return Err(anyhow!(
                "transfer of {bytes_read} bytes over {seconds:.1}s fell below {MIN_TRANSFER_BYTES_PER_SECOND} bytes/s"
            ));
        }
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
            rate.check(bytes_read as u64, started.elapsed())?;
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
        for seconds in [20, 40, 60] {
            assert!(rate.check(seconds, Duration::from_secs(20)).is_ok());
        }
        let err = rate.check(61, Duration::from_secs(1)).unwrap_err();
        assert!(err.to_string().contains("fell below 4096 bytes/s"), "{err}");
    }

    #[test]
    fn a_transfer_at_the_rate_floor_can_continue() {
        let mut rate = TransferRateGuard::default();
        let time = Duration::from_secs(90);
        assert!(rate.check(4096 * time.as_secs(), time).is_ok());
    }
}

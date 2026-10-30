//! The capped body reader. Edition-2021 clean: dcl-scene-cli compiles it by path.

use anyhow::{Result, anyhow};
use futures::StreamExt;

/// The advertised `Content-Length` is checked first so an oversized body costs
/// nothing, but the running total is enforced too: a chunked or lying
/// upstream never gets to grow the buffer past the cap.
pub async fn read_body_capped(resp: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    if let Some(len) = resp.content_length().filter(|len| *len > max_bytes as u64) {
        return Err(anyhow!(
            "response body advertises {len} bytes, over the {max_bytes} byte cap"
        ));
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        // A body error names the URL; a configured upstream's path can carry
        // a gateway key (TRADE_RPC_URLS, ETH_RPC_URL), so it is stripped as
        // `SendError` strips it from a transport error.
        let chunk = chunk.map_err(|e| e.without_url())?;
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(anyhow!("response body exceeds the {max_bytes} byte cap"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

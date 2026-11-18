//! Fetching a presentation by URL without reaching private networks, as
//! upstream (`src/logic/network-validator/component.ts`,
//! `pinned-https.ts`, `src/adapters/file-provider/component.ts:61-162`):
//! HTTPS only; every address the host resolves to must be public, and the
//! connection is pinned to those addresses (no second lookup to rebind);
//! redirects are followed by hand, at most 5, each hop checked again; the
//! body stops at 100 MiB and the whole download at 60 s.
//!
//! Upstream resolves with `dns.resolve4`/`resolve6`, which skip the hosts
//! file; this resolves with the system resolver, which reads it, so a name
//! the hosts file maps to a private address is refused as private rather
//! than as unresolvable. Both answer 400.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use futures::StreamExt;

use super::files::{MAX_FILE_SIZE, download_filename};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_REDIRECTS: usize = 5;

/// A download failure, split as the handler maps them.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DownloadError {
    /// 400, the message as is: the URL is not HTTPS or reaches a private address.
    #[error("{0}")]
    InvalidUrl(String),
    /// 400 `Failed to download file: <message>`.
    #[error("{0}")]
    Failed(String),
    /// 413.
    #[error("File exceeds maximum size of 100MB")]
    TooLarge,
    /// 500: a connection or TLS failure, which upstream does not classify.
    #[error("{0}")]
    Network(String),
}

/// IPv4 private, loopback, link-local, CGNAT, benchmarking, multicast,
/// reserved, broadcast and the zero address.
pub fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    a == 127
        || a == 10
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 169 && b == 254)
        || (224..=239).contains(&a)
        || ip == Ipv4Addr::BROADCAST
        || ip == Ipv4Addr::UNSPECIFIED
        || (a == 100 && (64..=127).contains(&b))
        || (a == 198 && (b == 18 || b == 19))
        || a >= 240
}

fn embedded_v4(high: u16, low: u16) -> Ipv4Addr {
    let [a, b] = high.to_be_bytes();
    let [c, d] = low.to_be_bytes();
    Ipv4Addr::new(a, b, c, d)
}

/// IPv6 loopback, unspecified, link-local, unique local, and the forms that
/// embed an IPv4 address (mapped, compatible, 6to4, NAT64), judged by it.
pub fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    let g = ip.segments();
    if ip == Ipv6Addr::LOCALHOST || ip == Ipv6Addr::UNSPECIFIED {
        return true;
    }
    if g[..5].iter().all(|&s| s == 0) && g[5] == 0xffff {
        return is_private_ipv4(embedded_v4(g[6], g[7]));
    }
    if g[..6].iter().all(|&s| s == 0) && (g[6] != 0 || g[7] > 1) {
        return is_private_ipv4(embedded_v4(g[6], g[7]));
    }
    if g[0] == 0x2002 {
        return is_private_ipv4(embedded_v4(g[1], g[2]));
    }
    if g[0] == 0x0064 && g[1] == 0xff9b && g[2..6].iter().all(|&s| s == 0) {
        return is_private_ipv4(embedded_v4(g[6], g[7]));
    }
    (g[0] & 0xffc0) == 0xfe80 || (g[0] & 0xfe00) == 0xfc00
}

pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_ipv4(v4),
        IpAddr::V6(v6) => is_private_ipv6(v6),
    }
}

/// A host's addresses, every one public, or why not.
pub async fn resolve_public(host: &str) -> Result<Vec<IpAddr>, String> {
    let literal = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addresses: Vec<IpAddr> = match literal.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => tokio::net::lookup_host((host, 443))
            .await
            .map(|found| found.map(|a| a.ip()).collect())
            .unwrap_or_default(),
    };
    if let Some(private) = addresses.iter().find(|ip| is_private_ip(**ip)) {
        return Err(format!("URL resolves to private IP: {host} → {private}"));
    }
    if addresses.is_empty() {
        return Err(format!("Could not resolve any addresses for {host}"));
    }
    Ok(addresses)
}

/// `resolveAndValidateUrl`: an absolute HTTPS URL whose host resolves only
/// to public addresses.
pub async fn validate_url(raw: &str) -> Result<(url::Url, Vec<IpAddr>), String> {
    let parsed = url::Url::parse(raw).map_err(|_| format!("Invalid URL: {raw}"))?;
    if parsed.scheme() != "https" {
        return Err(format!(
            "URL must use HTTPS, got {}: for {raw}",
            parsed.scheme()
        ));
    }
    let host = parsed.host_str().unwrap_or_default().to_string();
    let addresses = resolve_public(&host).await?;
    Ok((parsed, addresses))
}

/// The downloaded file and its name.
#[derive(Debug)]
pub struct Downloaded {
    pub bytes: Vec<u8>,
    pub filename: String,
}

/// One pinned hop: a client that dials only `addresses` for `url`'s host.
fn pinned_client(url: &url::Url, addresses: &[IpAddr]) -> Result<reqwest::Client, String> {
    let host = url.host_str().unwrap_or_default();
    let port = url.port_or_known_default().unwrap_or(443);
    let sockets: Vec<SocketAddr> = addresses
        .iter()
        .map(|ip| SocketAddr::new(*ip, port))
        .collect();
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .no_proxy()
        .resolve_to_addrs(host, &sockets)
        .build()
        .map_err(|e| e.to_string())
}

async fn download_hops(start: &str) -> Result<Downloaded, DownloadError> {
    let mut current = start.to_string();
    for _ in 0..MAX_REDIRECTS {
        let (url, addresses) = validate_url(&current)
            .await
            .map_err(DownloadError::InvalidUrl)?;
        let client = pinned_client(&url, &addresses).map_err(DownloadError::Network)?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| DownloadError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        if (300..400).contains(&status)
            && let Some(location) = response.headers().get(reqwest::header::LOCATION)
        {
            let location = location.to_str().unwrap_or_default();
            current = url
                .join(location)
                .map_err(|_| DownloadError::InvalidUrl(format!("Invalid URL: {location}")))?
                .to_string();
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(DownloadError::Failed(format!(
                "HTTP {status} downloading {current}"
            )));
        }
        let disposition = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;
            if bytes.len() + chunk.len() > MAX_FILE_SIZE {
                return Err(DownloadError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        let filename = download_filename(disposition.as_deref(), &url);
        return Ok(Downloaded { bytes, filename });
    }
    Err(DownloadError::Failed(format!(
        "Too many redirects (>{MAX_REDIRECTS})"
    )))
}

/// Downloads `url` within the limits above.
pub async fn download(url: &str) -> Result<Downloaded, DownloadError> {
    match tokio::time::timeout(DOWNLOAD_TIMEOUT, download_hops(url)).await {
        Ok(result) => result,
        Err(_) => Err(DownloadError::Failed(format!(
            "Download timed out after {}s",
            DOWNLOAD_TIMEOUT.as_secs()
        ))),
    }
}

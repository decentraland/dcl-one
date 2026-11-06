//! The caller's address behind the gateway, which overwrites `X-Real-IP` and
//! `X-Forwarded-For` with the peer it accepted (`infra/gateway/gateway.py`).

use axum::http::HeaderMap;

/// `X-Real-IP`, else the last `X-Forwarded-For` entry; empty when neither is set.
pub fn client_ip(headers: &HeaderMap) -> String {
    if let Some(value) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = value.trim();
        if !ip.is_empty() {
            return normalize_ip(ip);
        }
    }
    if let Some(value) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(last) = value.rsplit(',').next()
    {
        let ip = last.trim();
        if !ip.is_empty() {
            return normalize_ip(ip);
        }
    }
    String::new()
}

/// The TCP peer, or the single `X-Real-IP` it supplies when it is a trusted proxy
/// (`HTTP_TRUSTED_PROXY_CIDRS`); `None` without a peer or with a bad header.
pub fn peer_client_ip(
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<String> {
    let peer = peer?.ip().to_canonical();
    let ip = if trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
    {
        let mut values = headers.get_all("x-real-ip").iter();
        let candidate = values.next()?.to_str().ok()?;
        if values.next().is_some() {
            return None;
        }
        candidate.parse::<std::net::IpAddr>().ok()?.to_canonical()
    } else {
        peer
    };
    Some(ip.to_string())
}

pub fn normalize_ip(ip: &str) -> String {
    ip.strip_prefix("::ffff:").unwrap_or(ip).trim().to_string()
}

/// Bits of an IPv6 address that name one client, as for content downloads
/// (`CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`): a host routed a /64 is one source.
pub const IPV6_SOURCE_PREFIX_BITS: u32 = 64;

/// The per-source key of a client address: IPv4 as is, IPv6 by its /64 network.
pub fn source_key(client_ip: &str) -> String {
    use std::net::{IpAddr, Ipv6Addr};
    let Ok(IpAddr::V6(v6)) = client_ip.parse::<IpAddr>() else {
        return client_ip.to_string();
    };
    if let Some(v4) = v6.to_ipv4_mapped() {
        return v4.to_string();
    }
    let mask = u128::MAX << (128 - IPV6_SOURCE_PREFIX_BITS);
    let network = Ipv6Addr::from(u128::from(v6) & mask);
    format!("{network}/{IPV6_SOURCE_PREFIX_BITS}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderName;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        map
    }

    #[test]
    fn client_ip_ignores_client_settable_spoof_headers() {
        let map = headers(&[
            ("true-client-ip", "6.6.6.6"),
            ("cf-connecting-ip", "7.7.7.7"),
            ("x-real-ip", "203.0.113.9"),
        ]);
        assert_eq!(client_ip(&map), "203.0.113.9");
    }

    #[test]
    fn client_ip_takes_proxy_appended_forwarded_entry() {
        let map = headers(&[
            ("true-client-ip", "6.6.6.6"),
            ("x-forwarded-for", "6.6.6.6, 203.0.113.9"),
        ]);
        assert_eq!(client_ip(&map), "203.0.113.9");
    }

    #[test]
    fn client_ip_empty_when_only_spoofable_headers_present() {
        let map = headers(&[
            ("true-client-ip", "6.6.6.6"),
            ("cf-connecting-ip", "7.7.7.7"),
        ]);
        assert_eq!(client_ip(&map), "");
    }
}

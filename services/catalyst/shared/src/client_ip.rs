//! The caller's address behind the gateway, which overwrites `X-Real-IP` and
//! `X-Forwarded-For` with the peer it accepted (`infra/gateway/gateway.py`).

use std::net::IpAddr;

use axum::http::HeaderMap;

/// Whether the socket peer is a direct backend proxy listed in
/// `HTTP_TRUSTED_PROXY_CIDRS`: only such a peer may name the client in a
/// forwarding header. The routes are reachable without the gateway too, so a
/// header from any other peer is the client's own choice
/// (docs/adr/2026-11-04-referral-trusted-proxy-ingress.md).
pub fn is_trusted_proxy(peer: IpAddr, trusted_proxies: &[ipnet::IpNet]) -> bool {
    let peer = peer.to_canonical();
    trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
}

/// The one valid `X-Real-IP` a trusted proxy overwrote, canonical; `None`
/// when it is missing, repeated or not exactly an address (no padding, port
/// or brackets).
pub fn single_real_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let mut values = headers.get_all("x-real-ip").iter();
    let candidate = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(candidate.parse::<IpAddr>().ok()?.to_canonical())
}

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
    let ip = if is_trusted_proxy(peer, trusted_proxies) {
        single_real_ip(headers)?
    } else {
        peer
    };
    Some(ip.to_string())
}

/// [`peer_client_ip`] only when it names one client: a trusted proxy's
/// `X-Real-IP`, or a peer that is not internal (an unlisted gateway's own
/// address would put every client in one bucket).
pub fn distinct_client_ip(
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<String> {
    let ip = peer?.ip().to_canonical();
    if is_trusted_proxy(ip, trusted_proxies) {
        return peer_client_ip(headers, peer, trusted_proxies);
    }
    (!is_internal(ip)).then(|| ip.to_string())
}

/// Loopback, unspecified, private (RFC 1918, CGNAT, IPv6 ULA) or link-local.
pub fn is_internal(ip: std::net::IpAddr) -> bool {
    match ip.to_canonical() {
        std::net::IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || (a == 100 && b & 0xc0 == 64)
        }
        std::net::IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80
        }
    }
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
    fn distinct_client_ip_skips_internal_peers_unless_trusted() {
        let peer = |ip: &str| Some(std::net::SocketAddr::new(ip.parse().unwrap(), 1));
        let real = headers(&[("x-real-ip", "203.0.113.9")]);
        for internal in [
            "10.0.0.5",
            "172.18.0.2",
            "192.168.1.1",
            "127.0.0.1",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:10.0.0.5",
        ] {
            assert_eq!(
                distinct_client_ip(&real, peer(internal), &[]),
                None,
                "{internal}"
            );
        }
        assert_eq!(
            distinct_client_ip(&real, peer("198.51.100.4"), &[]).as_deref(),
            Some("198.51.100.4")
        );
        assert_eq!(distinct_client_ip(&real, None, &[]), None);
        let trusted = ["10.0.0.0/8".parse().unwrap()];
        assert_eq!(
            distinct_client_ip(&real, peer("10.0.0.5"), &trusted).as_deref(),
            Some("203.0.113.9")
        );
        assert_eq!(
            distinct_client_ip(&HeaderMap::new(), peer("10.0.0.5"), &trusted),
            None
        );
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

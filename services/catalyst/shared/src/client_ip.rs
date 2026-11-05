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

pub fn normalize_ip(ip: &str) -> String {
    ip.strip_prefix("::ffff:").unwrap_or(ip).trim().to_string()
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

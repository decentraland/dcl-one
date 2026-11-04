//! The `GET`/`HEAD /contents/{hash}` rate limit, upstream's
//! (`decentraland/catalyst` `src/controllers/content-get-rate-limit.ts`,
//! `sync-peer-exemption.ts`, `client-source.ts`): three fixed-window buckets
//! per client, a burst of `CONTENT_GET_RATE_LIMIT_MAX` per
//! `CONTENT_GET_RATE_LIMIT_WINDOW_SECONDS`, then `CONTENT_GET_HOURLY_QUOTA_MAX`
//! per hour, then `CONTENT_GET_DAILY_QUOTA_MAX` per day. The burst runs first,
//! so a request it rejects does not drain the quotas. A client is its IPv4
//! address or its IPv6 network of `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`
//! bits (a host routed a /64 cannot rotate addresses for a fresh budget).
//! Peers listed in `TRUSTED_SYNC_PEER_IPS` skip all three, so a peer
//! bootstrapping from this node is never throttled; the gateway exempts the
//! same addresses from its own limits (`GATEWAY_TRUSTED_SYNC_PEER_IPS`).
//!
//! Over a budget the answer is upstream's 429 `{"error":"Too many requests"}`
//! with `Retry-After` (seconds until the rejecting window ends) and the
//! `RateLimit-Limit`/`-Remaining`/`-Reset` triplet of that bucket; a request
//! that passes carries the burst bucket's triplet. The triplet's exact values
//! come from `@dcl/rate-limiter-component`, which the pinned tree does not
//! vendor, so they are this module's reading of it, not a capture.
//!
//! The counters are per process, as upstream's in-memory store is: N `serve`
//! replicas behind the gateway admit N times each budget, and a restart
//! resets them. Each bucket tracks at most [`MAX_SOURCES`] clients; a new
//! client past that evicts the oldest window.

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use parking_lot::Mutex;

use super::SharedState;
use super::errors::json_error;

/// Clients each bucket tracks at once.
const MAX_SOURCES: usize = 100_000;

/// The limit's settings, upstream's environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub burst_max: u32,
    pub burst_window: Duration,
    pub hourly_max: u32,
    pub daily_max: u32,
    pub ipv6_prefix_length: u8,
    /// `TRUSTED_SYNC_PEER_IPS`, the raw comma list.
    pub trusted_sync_peer_ips: String,
    /// `TRUSTED_CLIENT_IP_HEADER`.
    pub trusted_client_ip_header: Option<String>,
}

impl Default for Config {
    /// Upstream's defaults (`src/Environment.ts:106-125`).
    fn default() -> Self {
        Self {
            burst_max: 1000,
            burst_window: Duration::from_secs(60),
            hourly_max: 3000,
            daily_max: 20000,
            ipv6_prefix_length: 64,
            trusted_sync_peer_ips: String::new(),
            trusted_client_ip_header: None,
        }
    }
}

struct Window {
    started: Instant,
    count: u32,
}

/// One fixed-window budget per client key.
struct Bucket {
    max: u32,
    window: Duration,
    state: Mutex<Windows>,
}

#[derive(Default)]
struct Windows {
    by_key: HashMap<String, Window>,
    by_age: VecDeque<String>,
}

/// What a bucket answered: its limit, what is left and the seconds until its
/// window ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Verdict {
    allowed: bool,
    limit: u32,
    remaining: u32,
    reset_secs: u64,
}

impl Bucket {
    fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            state: Mutex::new(Windows::default()),
        }
    }

    fn consume(&self, key: &str, now: Instant) -> Verdict {
        let mut state = self.state.lock();
        while let Some(oldest) = state.by_age.front() {
            match state.by_key.get(oldest) {
                Some(w) if now.saturating_duration_since(w.started) < self.window => break,
                _ => {
                    let oldest = state.by_age.pop_front().expect("front exists");
                    state.by_key.remove(&oldest);
                }
            }
        }
        if !state.by_key.contains_key(key) {
            if state.by_key.len() >= MAX_SOURCES
                && let Some(oldest) = state.by_age.pop_front()
            {
                state.by_key.remove(&oldest);
            }
            state.by_key.insert(
                key.to_string(),
                Window {
                    started: now,
                    count: 0,
                },
            );
            state.by_age.push_back(key.to_string());
        }
        let w = state.by_key.get_mut(key).expect("inserted above");
        let left = self
            .window
            .saturating_sub(now.saturating_duration_since(w.started));
        let reset_secs = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        let allowed = w.count < self.max;
        if allowed {
            w.count += 1;
        }
        Verdict {
            allowed,
            limit: self.max,
            remaining: self.max - w.count,
            reset_secs: reset_secs.max(1),
        }
    }
}

pub struct ContentGetLimit {
    buckets: [Bucket; 3],
    ipv6_prefix_length: u8,
    trusted_sync_peers: HashSet<IpAddr>,
    client_ip_header: Option<HeaderName>,
}

impl ContentGetLimit {
    pub fn new(config: &Config) -> Self {
        let client_ip_header = config
            .trusted_client_ip_header
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
            .and_then(|h| match HeaderName::try_from(h) {
                Ok(name) => Some(name),
                Err(_) => {
                    tracing::warn!(
                        header = h,
                        "TRUSTED_CLIENT_IP_HEADER is not a header name; ignored"
                    );
                    None
                }
            });
        Self {
            buckets: [
                Bucket::new(config.burst_max, config.burst_window),
                Bucket::new(config.hourly_max, Duration::from_secs(3600)),
                Bucket::new(config.daily_max, Duration::from_secs(86400)),
            ],
            ipv6_prefix_length: config.ipv6_prefix_length.clamp(1, 128),
            trusted_sync_peers: parse_trusted_sync_peers(&config.trusted_sync_peer_ips),
            client_ip_header,
        }
    }

    /// The client a request is counted as: the rightmost entry of
    /// `TRUSTED_CLIENT_IP_HEADER` when set (one trusted proxy), else the
    /// gateway-stamped address, else the socket; canonical (an IPv4-mapped
    /// IPv6 address is its IPv4 address). `None` when none parses.
    fn client(&self, headers: &HeaderMap, socket: Option<IpAddr>) -> Option<IpAddr> {
        let from_header = match &self.client_ip_header {
            Some(name) => headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit(',').next())
                .and_then(parse_ip),
            None => parse_ip(&crate::shared::client_ip::client_ip(headers)),
        };
        from_header.or(socket.map(|ip| ip.to_canonical()))
    }

    /// Runs the three buckets in order for `client`: the first rejection, or
    /// the burst bucket's verdict when all pass. `None` for an exempt peer.
    fn check(&self, client: IpAddr, now: Instant) -> Option<Verdict> {
        if self.trusted_sync_peers.contains(&client) {
            return None;
        }
        let key = client_key(client, self.ipv6_prefix_length);
        let mut first = None;
        for bucket in &self.buckets {
            let verdict = bucket.consume(&key, now);
            if !verdict.allowed {
                return Some(verdict);
            }
            first.get_or_insert(verdict);
        }
        first
    }
}

fn parse_ip(raw: &str) -> Option<IpAddr> {
    let raw = raw.trim();
    let raw = raw.split('%').next().unwrap_or(raw);
    raw.parse::<IpAddr>().ok().map(|ip| ip.to_canonical())
}

/// The canonical addresses of `TRUSTED_SYNC_PEER_IPS`; an entry that is not
/// an IP address is dropped with a warning (a typo must not stop the node).
fn parse_trusted_sync_peers(raw: &str) -> HashSet<IpAddr> {
    let mut peers = HashSet::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        match parse_ip(entry) {
            Some(ip) => {
                peers.insert(ip);
            }
            None => tracing::warn!(
                entry,
                "Ignoring invalid entry in TRUSTED_SYNC_PEER_IPS: not an IP address"
            ),
        }
    }
    peers
}

/// IPv4 by address, IPv6 by its network of `prefix` bits.
fn client_key(ip: IpAddr, prefix: u8) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let mask = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            format!("{}/{prefix}", Ipv6Addr::from(u128::from(v6) & mask))
        }
    }
}

fn rate_limit_headers(headers: &mut HeaderMap, verdict: &Verdict) {
    for (name, value) in [
        ("ratelimit-limit", verdict.limit.to_string()),
        ("ratelimit-remaining", verdict.remaining.to_string()),
        ("ratelimit-reset", verdict.reset_secs.to_string()),
    ] {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    }
}

/// The layer on `GET`/`HEAD /contents/{hash}`.
pub async fn limit(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let socket = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip());
    let limiter = &state.content_get_limit;
    let Some(client) = limiter.client(request.headers(), socket) else {
        return next.run(request).await;
    };
    match limiter.check(client, Instant::now()) {
        None => next.run(request).await,
        Some(verdict) if verdict.allowed => {
            let mut response = next.run(request).await;
            rate_limit_headers(response.headers_mut(), &verdict);
            response
        }
        Some(verdict) => {
            let mut response = json_error(StatusCode::TOO_MANY_REQUESTS, "Too many requests");
            let headers = response.headers_mut();
            rate_limit_headers(headers, &verdict);
            if let Ok(value) = HeaderValue::from_str(&verdict.reset_secs.to_string()) {
                headers.insert("retry-after", value);
            }
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(burst: u32, hourly: u32, daily: u32, peers: &str) -> ContentGetLimit {
        ContentGetLimit::new(&Config {
            burst_max: burst,
            hourly_max: hourly,
            daily_max: daily,
            trusted_sync_peer_ips: peers.into(),
            ..Config::default()
        })
    }

    fn statuses(l: &ContentGetLimit, ips: &[&str], now: Instant) -> Vec<bool> {
        ips.iter()
            .map(|ip| {
                l.check(parse_ip(ip).unwrap(), now)
                    .is_none_or(|v| v.allowed)
            })
            .collect()
    }

    /// Upstream `content-get-rate-limit.spec.ts:427-466`.
    #[test]
    fn ipv6_clients_are_keyed_by_network() {
        let key = |ip: &str, p| client_key(parse_ip(ip).unwrap(), p);
        assert_eq!(
            key("2a03:b0c0:3:f0:0:3:410:3000", 64),
            key("2a03:b0c0:3:f0:0:2:6d50:c000", 64)
        );
        assert_eq!(
            key("2a03:b0c0:3:f0:0:3:410:3000", 64),
            "2a03:b0c0:3:f0::/64"
        );
        assert_eq!(key("2001:db8::1:0:0:1f", 124), "2001:db8::1:0:0:10/124");
        assert_eq!(key("fe80::1%eth0", 64), "fe80::/64");
        assert_eq!(key("64:ff9b::1.2.3.4", 128), "64:ff9b::102:304/128");
        assert_eq!(key("::ffff:203.0.113.7", 64), "203.0.113.7");
        assert_eq!(key("203.0.113.7", 64), "203.0.113.7");
    }

    #[test]
    fn the_hourly_quota_rejects_under_the_burst() {
        let l = limiter(100, 3, 100, "");
        let now = Instant::now();
        let ip = "198.51.100.9";
        assert_eq!(
            statuses(&l, &[ip, ip, ip, ip], now),
            [true, true, true, false]
        );
    }

    #[test]
    fn the_daily_quota_rejects_under_the_hourly_one() {
        let l = limiter(100, 100, 2, "");
        let now = Instant::now();
        let ip = "198.51.100.9";
        assert_eq!(statuses(&l, &[ip, ip, ip], now), [true, true, false]);
    }

    #[test]
    fn a_burst_rejection_does_not_drain_the_quotas() {
        let l = limiter(1, 2, 100, "");
        let now = Instant::now();
        let ip = "198.51.100.9";
        assert_eq!(statuses(&l, &[ip, ip, ip], now), [true, false, false]);
        // The burst window ended; one hourly unit is still left.
        let later = now + Duration::from_secs(61);
        assert_eq!(statuses(&l, &[ip, ip], later), [true, false]);
        let v = l
            .check(parse_ip(ip).unwrap(), later + Duration::from_secs(61))
            .unwrap();
        assert!(!v.allowed, "the hourly quota is spent: {v:?}");
        assert_eq!(v.limit, 2);
        assert!(v.reset_secs > 3000 && v.reset_secs <= 3600, "{v:?}");
    }

    #[test]
    fn ipv6_rotation_inside_one_network_shares_a_budget() {
        let l = limiter(100, 3, 100, "");
        let now = Instant::now();
        let ips = [
            "2a03:b0c0:3:f0::1",
            "2a03:b0c0:3:f0::2",
            "2a03:b0c0:3:f0:0:3:410:3000",
            "2a03:b0c0:3:f0:ffff::9",
        ];
        assert_eq!(statuses(&l, &ips, now), [true, true, true, false]);
        let ips = ["2a03:b0c0:3:f1::1", "198.51.100.10"];
        assert_eq!(
            statuses(&l, &ips, now),
            [true, true],
            "other clients keep theirs"
        );
    }

    #[test]
    fn listed_sync_peers_are_never_limited_and_bad_entries_are_dropped() {
        let l = limiter(1, 1, 1, "203.0.113.50, not-an-ip, ::ffff:203.0.113.51");
        let now = Instant::now();
        let peer = "203.0.113.50";
        assert_eq!(statuses(&l, &[peer; 5], now), [true; 5]);
        assert_eq!(
            statuses(&l, &["203.0.113.51"; 3], now),
            [true; 3],
            "canonical form"
        );
        assert_eq!(statuses(&l, &["198.51.100.9"; 2], now), [true, false]);
        assert_eq!(l.trusted_sync_peers.len(), 2);
    }

    #[test]
    fn the_client_comes_from_the_trusted_header_then_the_gateway_then_the_socket() {
        let socket = Some("192.0.2.1".parse().unwrap());
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.2".parse().unwrap());
        headers.insert("cf-connecting-ip", "6.6.6.6, 198.51.100.3".parse().unwrap());
        let gateway = limiter(1, 1, 1, "");
        assert_eq!(gateway.client(&headers, socket), parse_ip("198.51.100.2"));
        assert_eq!(gateway.client(&HeaderMap::new(), socket), socket);
        let trusted = ContentGetLimit::new(&Config {
            trusted_client_ip_header: Some("CF-Connecting-IP".into()),
            ..Config::default()
        });
        assert_eq!(trusted.client(&headers, socket), parse_ip("198.51.100.3"));
        assert_eq!(trusted.client(&HeaderMap::new(), None), None);
    }

    /// Through the router: the burst 429 body and headers, the exempt peer,
    /// and the triplet on a request that passes; both mounts share a budget.
    #[tokio::test]
    async fn the_router_answers_upstreams_429() {
        use crate::tests::support::{TEST_CID, TestNode};
        use axum::body::Body;
        use tower::ServiceExt;

        let mut settings = crate::Settings::for_tests();
        settings.content_get_limit = Config {
            burst_max: 2,
            trusted_sync_peer_ips: "203.0.113.50".into(),
            ..Config::default()
        };
        let node = TestNode::without_db_settings("content-limit", settings).await;
        node.store(TEST_CID, bytes::Bytes::from_static(b"x")).await;
        let send = |method: &str, path: String, ip: &str| {
            let request = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("x-real-ip", ip)
                .body(Body::empty())
                .unwrap();
            node.router().oneshot(request)
        };
        let client = "198.51.100.9";
        let first = send("GET", format!("/content/contents/{TEST_CID}"), client)
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()["ratelimit-limit"], "2");
        assert_eq!(first.headers()["ratelimit-remaining"], "1");
        let second = send("HEAD", format!("/contents/{TEST_CID}"), client)
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let third = send("GET", format!("/contents/{TEST_CID}"), client)
            .await
            .unwrap();
        assert_eq!(third.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after: u64 = third.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=60).contains(&retry_after));
        assert_eq!(third.headers()["ratelimit-remaining"], "0");
        let body = axum::body::to_bytes(third.into_body(), 1024).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"error": "Too many requests"})
        );
        for _ in 0..4 {
            let peer = send("GET", format!("/contents/{TEST_CID}"), "203.0.113.50")
                .await
                .unwrap();
            assert_eq!(peer.status(), StatusCode::OK);
            assert!(peer.headers().get("ratelimit-limit").is_none());
        }
        let other = send(
            "GET",
            format!("/contents/{TEST_CID}/active-entities"),
            client,
        )
        .await
        .unwrap();
        assert_ne!(
            other.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "only /contents/{{hash}} is limited"
        );
    }
}

//! The process's outbound HTTP: one hardened [`reqwest::Client`] built through
//! [`HttpClientCfg`] and wrapped in the one [`Outbound`] handle `serve` hands
//! to every reader (peer sync and the RPC validator take its `raw` client,
//! the mirrors and upstream readers send through the guard), the SSRF guard
//! ([`is_safe_http_url`] on the string, [`resolve_and_pin`] on the socket),
//! the retry ladder ([`retry_with_backoff`]) and the capped body reader.

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

#[cfg(feature = "db")]
pub mod api_error;
mod body;
pub mod jsonrpc;

pub use body::read_body_capped;

pub const USER_AGENT: &str = concat!("catalyst/", env!("CARGO_PKG_VERSION"));

/// Redirect following is opt-in: for user-supplied URLs each hop has to be
/// re-checked against the SSRF guards, which reqwest's own redirect policy
/// cannot do. Configured upstream endpoints (peers, mirror sources) may
/// follow a bounded number.
#[derive(Clone, Debug)]
pub struct HttpClientCfg {
    /// Whole-request budget; `None` for readers that stream large bodies
    /// (snapshots) and bound them by `read_timeout` and the body cap instead.
    pub total_timeout: Option<Duration>,
    pub connect_timeout: Duration,
    /// Longest pause between two bytes of a response.
    pub read_timeout: Option<Duration>,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub max_redirects: Option<usize>,
    pub tcp_nodelay: bool,
    pub user_agent: Option<String>,
}

impl Default for HttpClientCfg {
    fn default() -> Self {
        Self {
            total_timeout: Some(Duration::from_secs(30)),
            connect_timeout: Duration::from_secs(10),
            read_timeout: None,
            pool_idle_timeout: Duration::from_secs(90),
            pool_max_idle_per_host: 16,
            max_redirects: None,
            tcp_nodelay: true,
            user_agent: None,
        }
    }
}

impl HttpClientCfg {
    /// The client `serve` builds once for every upstream reader, with the old
    /// sync client's limits: no whole-request budget (snapshot files are
    /// gigabytes), 8 s connect, 25 s read, `CONNECTIONS_MAX_IDLE` idle
    /// connections per host, at most two redirects.
    pub fn serve(connections_max_idle: usize) -> Self {
        Self {
            total_timeout: None,
            connect_timeout: Duration::from_secs(8),
            read_timeout: Some(Duration::from_secs(25)),
            pool_max_idle_per_host: connections_max_idle,
            max_redirects: Some(2),
            ..Self::default()
        }
    }

    pub fn with_total_timeout(mut self, timeout: Duration) -> Self {
        self.total_timeout = Some(timeout);
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn with_pool(mut self, idle_timeout: Duration, max_idle_per_host: usize) -> Self {
        self.pool_idle_timeout = idle_timeout;
        self.pool_max_idle_per_host = max_idle_per_host;
        self
    }

    pub fn following_redirects(mut self, max: usize) -> Self {
        self.max_redirects = Some(max);
        self
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    pub fn builder(&self) -> reqwest::ClientBuilder {
        let redirect = match self.max_redirects {
            Some(max) => guarded_redirect_policy(max),
            None => reqwest::redirect::Policy::none(),
        };
        let mut builder = reqwest::Client::builder()
            .user_agent(self.user_agent.as_deref().unwrap_or(USER_AGENT))
            .connect_timeout(self.connect_timeout)
            .pool_idle_timeout(self.pool_idle_timeout)
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .tcp_nodelay(self.tcp_nodelay)
            .dns_resolver(ReservedNamesResolver::default())
            .redirect(redirect);
        if let Some(total) = self.total_timeout {
            builder = builder.timeout(total);
        }
        if let Some(read) = self.read_timeout {
            builder = builder.read_timeout(read);
        }
        builder
    }

    pub fn build(&self) -> reqwest::Result<reqwest::Client> {
        self.builder().build()
    }
}

/// What RFC 6761 fixes for a name before any resolver is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservedName {
    /// `invalid` and every `.invalid` name: NXDOMAIN.
    Invalid,
    /// `localhost` and every `.localhost` name: the loopback.
    Loopback,
}

pub fn reserved_name(host: &str) -> Option<ReservedName> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host == "invalid" || host.ends_with(".invalid") {
        Some(ReservedName::Invalid)
    } else if host == "localhost" || host.ends_with(".localhost") {
        Some(ReservedName::Loopback)
    } else {
        None
    }
}

type SystemLookup = dyn Fn(String) -> reqwest::dns::Resolving + Send + Sync;

/// The resolver every [`HttpClientCfg`] client asks. A `.invalid` name fails
/// as NXDOMAIN and a `localhost`/`.localhost` one is the loopback without a
/// query leaving the process (an island's placeholders and its own names never
/// reach a DNS server); every other name goes to `system`, the
/// `getaddrinfo` lookup reqwest's default resolver makes.
#[derive(Clone)]
pub struct ReservedNamesResolver {
    system: Arc<SystemLookup>,
}

impl Default for ReservedNamesResolver {
    fn default() -> Self {
        Self::over(system_lookup)
    }
}

impl ReservedNamesResolver {
    pub fn over(
        system: impl Fn(String) -> reqwest::dns::Resolving + Send + Sync + 'static,
    ) -> Self {
        Self {
            system: Arc::new(system),
        }
    }
}

fn system_lookup(host: String) -> reqwest::dns::Resolving {
    Box::pin(async move {
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
        Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
    })
}

impl reqwest::dns::Resolve for ReservedNamesResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        match reserved_name(&host) {
            Some(ReservedName::Invalid) => Box::pin(async move {
                Err(format!("{host}: NXDOMAIN, an .invalid name never resolves (RFC 6761)").into())
            }),
            Some(ReservedName::Loopback) => Box::pin(async {
                let loopback = [
                    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0),
                    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 0),
                ];
                Ok(Box::new(loopback.into_iter()) as reqwest::dns::Addrs)
            }),
            None => (self.system)(host),
        }
    }
}

/// Follows at most `max` hops, and only to a public http(s) host once the
/// chain started at one: a public upstream (or a MITM on a plaintext one)
/// cannot steer a fetch onto the loopback or the metadata address with a
/// `Location` header. A chain that started on a private host (a development
/// peer on the loopback) keeps following private hops.
fn guarded_redirect_policy(max: usize) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() > max {
            return attempt.error(format!("more than {max} redirects"));
        }
        let started_public = attempt
            .previous()
            .first()
            .is_none_or(|u| is_safe_http_url(u.as_str()));
        if started_public && !is_safe_http_url(attempt.url().as_str()) {
            let host = attempt.url().host_str().unwrap_or("?").to_string();
            return attempt.error(format!(
                "redirect to {host} refused: not a public http(s) host"
            ));
        }
        attempt.follow()
    })
}

pub fn try_http_client(cfg: &HttpClientCfg) -> reqwest::Result<reqwest::Client> {
    cfg.build()
}

/// Falls back to a default client so a TLS-backend hiccup degrades the
/// timeouts rather than taking the service down at startup.
pub fn http_client(name: &str, cfg: &HttpClientCfg) -> reqwest::Client {
    match try_http_client(cfg) {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(client = name, %err, "http client build failed; using defaults");
            reqwest::Client::new()
        }
    }
}

const INTERNAL_HOST_SUFFIXES: [&str; 8] = [
    ".localhost",
    ".local",
    ".internal",
    ".intranet",
    ".lan",
    ".home",
    ".corp",
    ".home.arpa",
];

/// True for anything a server must not be talked into fetching: reserved and
/// private IP ranges, single-label names, and the reserved internal DNS
/// suffixes.
///
/// Two check classes are merged because neither alone is sufficient. The
/// `IpAddr` range checks catch every literal form the standard parser accepts
/// (including IPv4-mapped IPv6), but reject nothing that is not a literal; the
/// string checks catch names that never reach a resolver (`router`,
/// `nas.local`) and the URL-spec IPv4 forms (`0x7f000001`, `0177.0.0.1`,
/// `127.1`) that `IpAddr` refuses to parse.
pub fn is_private_or_internal_host(host: &str) -> bool {
    let host = host.trim();
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let normalized = unbracketed.trim_end_matches('.').to_ascii_lowercase();

    if normalized.is_empty() {
        return true;
    }
    if let Ok(ip) = normalized.parse::<IpAddr>() {
        return is_blocked_ip(ip);
    }
    if let Some(ip) = spec_normalized_ip(&normalized) {
        return is_blocked_ip(ip);
    }
    is_internal_hostname(&normalized)
}

/// True only for an `http`/`https` URL whose host is publicly routable.
///
/// A clicked link reaches an unrestricted URL opener on the viewer's machine
/// and a server-side fetch reaches the machine's own network, so anything that
/// is not plainly safe fails closed. Whitespace anywhere in the target is
/// rejected outright rather than percent-encoded away, since it is only ever
/// there to smuggle a second URL past a parser.
pub fn is_safe_http_url(url: &str) -> bool {
    let trimmed = url.trim_matches(is_js_whitespace);
    if trimmed.chars().any(is_js_whitespace) {
        return false;
    }
    let Ok(parsed) = url::Url::parse(trimmed) else {
        return false;
    };
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return false;
    }
    match parsed.host_str() {
        Some(host) => !is_private_or_internal_host(host),
        None => false,
    }
}

/// Resolves `url`'s host and returns the address to pin the connection to, or
/// `None` if the host is not publicly routable.
///
/// Pinning is what closes the DNS-rebinding window: the guard's verdict and
/// the socket the request actually opens come from the same lookup, so a name
/// that answers differently a millisecond later cannot be dialled. Every
/// answer must be public -- one blocked address in the set rejects the host.
pub async fn resolve_and_pin(url: &reqwest::Url) -> Option<SocketAddr> {
    let host = url.host_str()?;
    let port = url.port_or_known_default().unwrap_or(80);
    let bare = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);

    if let Ok(ip) = bare.parse::<IpAddr>() {
        return (!is_blocked_ip(ip)).then_some(SocketAddr::new(ip, port));
    }
    if is_private_or_internal_host(host) || reserved_name(bare).is_some() {
        return None;
    }

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((bare, port)).await.ok()?.collect();
    if addrs.is_empty() || addrs.iter().any(|a| is_blocked_ip(a.ip())) {
        return None;
    }
    Some(addrs[0])
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || v4.octets()[0] == 0
                || {
                    let o = v4.octets();
                    o[0] == 100 && (o[1] & 0xc0) == 0x40
                }
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn spec_normalized_ip(host: &str) -> Option<IpAddr> {
    let parsed = url::Url::parse(&format!("http://{host}/")).ok()?;
    let normalized = parsed.host_str()?;
    if normalized == host {
        return None;
    }
    normalized
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(normalized)
        .parse::<IpAddr>()
        .ok()
}

fn is_internal_hostname(host: &str) -> bool {
    if let Some([a, b, _, _]) = parse_dotted_quad(host) {
        return a == 0
            || a == 127
            || a == 10
            || (a == 169 && b == 254)
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 100 && (64..=127).contains(&b));
    }

    if host.contains(':') {
        return host == "::1"
            || host == "::"
            || ["fe8", "fe9", "fea", "feb"]
                .iter()
                .any(|p| host.starts_with(p))
            || host.starts_with("fc")
            || host.starts_with("fd")
            || host.starts_with("::ffff:");
    }

    if !host.contains('.') {
        return true;
    }
    INTERNAL_HOST_SUFFIXES
        .iter()
        .any(|suffix| host.ends_with(suffix))
}

fn parse_dotted_quad(host: &str) -> Option<[u32; 4]> {
    let mut octets = [0u32; 4];
    let mut parts = host.split('.');
    for octet in &mut octets {
        let p = parts.next()?;
        if p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *octet = p.parse().ok()?;
    }
    parts.next().is_none().then_some(octets)
}

fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

const RATE_LIMIT_ATTEMPTS: u32 = 5;
const RATE_LIMIT_BASE_MS: u64 = 2000;
const MAX_SLEEP: Duration = Duration::from_secs(30);

pub enum RetryDecision<T> {
    Done(T),
    Retry {
        after: Option<Duration>,
        rate_limited: bool,
    },
}

/// Rate limiting gets its own, longer ladder and its own attempt budget: a
/// 429 means the upstream is fine and we are early, which deserves more
/// patience than a transient failure. `Ok(None)` means the budget ran out;
/// `Err` only ever comes from `op` itself.
pub async fn retry_with_backoff<T, F, Fut>(
    label: &str,
    attempts: u32,
    base_ms: u64,
    mut op: F,
) -> Result<Option<T>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<RetryDecision<T>>>,
{
    let max_attempts = attempts.max(RATE_LIMIT_ATTEMPTS);
    let mut backoff_ms = base_ms;
    let mut rl_backoff_ms = RATE_LIMIT_BASE_MS;
    let mut plain_failures = 0u32;
    for attempt in 1..=max_attempts {
        match op().await? {
            RetryDecision::Done(v) => return Ok(Some(v)),
            RetryDecision::Retry {
                after,
                rate_limited,
            } => {
                if !rate_limited {
                    plain_failures += 1;
                    if plain_failures >= attempts {
                        return Ok(None);
                    }
                }
                if attempt == max_attempts {
                    return Ok(None);
                }
                let ladder = if rate_limited {
                    let d = Duration::from_millis(rl_backoff_ms);
                    rl_backoff_ms = rl_backoff_ms.saturating_mul(2);
                    d
                } else {
                    let d = Duration::from_millis(backoff_ms);
                    backoff_ms = backoff_ms.saturating_mul(2);
                    d
                };
                let wait = after.unwrap_or(ladder).min(MAX_SLEEP);
                tracing::warn!(
                    retry = label,
                    attempt,
                    of = if rate_limited { max_attempts } else { attempts },
                    rate_limited,
                    wait_ms = wait.as_millis() as u64,
                    "retrying after backoff"
                );
                tokio::time::sleep(wait).await;
            }
        }
    }
    Ok(None)
}

/// Parses a `Retry-After` header, in either the delta-seconds or the
/// HTTP-date form.
///
/// An unparseable value still yields a short wait rather than `None`: the
/// header's presence is itself the signal to slow down.
pub fn parse_retry_after(header: &str) -> Option<Duration> {
    let trimmed = header.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let delta = date.with_timezone(&chrono::Utc) - chrono::Utc::now();
        return Some(delta.to_std().unwrap_or(Duration::ZERO));
    }
    Some(Duration::from_secs(5))
}

pub fn is_transient_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// How long a pinned address is reused before the host is resolved again.
const PIN_TTL: Duration = Duration::from_secs(60);
const PIN_CACHE_MAX: usize = 256;

/// The handle every upstream reader holds (the mirrors, the explorer-boot
/// readers, the lambdas' subgraph/RPC/trades readers). One shared pooled
/// client underneath ([`Outbound::raw`], which peer sync keeps using), plus
/// the guard that the string check alone cannot give: the host of every
/// request is resolved through [`resolve_and_pin`] and the connection is
/// opened to that address (a per-host client built with
/// `ClientBuilder::resolve`, cached for [`PIN_TTL`]), so a public name that
/// answers with a private address, or answers differently a moment later, is
/// never dialled. Redirects are re-issued by hand through the same guard
/// (`max_redirects` hops), never followed blindly.
///
/// `allow_private` is for tests against a loopback stub: it skips the guard
/// and the pinning and sends through the shared client. Never set from
/// configuration.
#[derive(Clone)]
pub struct Outbound {
    cfg: Arc<HttpClientCfg>,
    shared: reqwest::Client,
    /// Redirects off: the manual loop in [`Outbound::execute`] follows them.
    direct: reqwest::Client,
    pinned: Arc<Mutex<HashMap<String, Pinned>>>,
    max_redirects: usize,
    allow_private: bool,
    #[cfg(test)]
    test_pins: Arc<Mutex<HashMap<String, SocketAddr>>>,
}

struct Pinned {
    addr: SocketAddr,
    resolved_at: Instant,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// The guard refused the target before any byte left the process.
    #[error("{0}")]
    Refused(String),
    /// Transport or protocol failure; the URL is stripped from the text
    /// (`reqwest::Error::without_url`) so a gateway key in a configured
    /// upstream path never reaches a log line or a status route.
    #[error("{0}")]
    Http(reqwest::Error),
}

impl From<reqwest::Error> for SendError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e.without_url())
    }
}

impl SendError {
    pub fn is_connect(&self) -> bool {
        matches!(self, Self::Http(e) if e.is_connect())
    }

    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Http(e) if e.is_timeout())
    }

    pub fn is_request(&self) -> bool {
        matches!(self, Self::Http(e) if e.is_request())
    }

    /// True for what a retry ladder should retry: a refused or unresolvable
    /// target is not one of them.
    pub fn is_transient(&self) -> bool {
        self.is_connect() || self.is_timeout() || self.is_request()
    }
}

impl std::fmt::Debug for Outbound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Outbound")
            .field("max_redirects", &self.max_redirects)
            .field("allow_private", &self.allow_private)
            .finish()
    }
}

impl Outbound {
    pub fn new(cfg: HttpClientCfg) -> reqwest::Result<Self> {
        let shared = cfg.build()?;
        let direct = HttpClientCfg {
            max_redirects: None,
            ..cfg.clone()
        }
        .build()?;
        Ok(Self {
            max_redirects: cfg.max_redirects.unwrap_or(0),
            cfg: Arc::new(cfg),
            shared,
            direct,
            pinned: Arc::new(Mutex::new(HashMap::new())),
            allow_private: false,
            #[cfg(test)]
            test_pins: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Tests only: `host` resolves to `addr` without DNS, so a loopback stub
    /// can stand in for a public name and the pinning path runs for real.
    #[cfg(test)]
    fn with_test_pin(self, host: &str, addr: SocketAddr) -> Self {
        self.test_pins
            .lock()
            .unwrap()
            .insert(host.to_ascii_lowercase(), addr);
        self
    }

    /// Default limits and the loopback admitted: for tests with a stub upstream.
    pub fn for_tests() -> Self {
        Self::new(HttpClientCfg::default())
            .expect("the default client builds")
            .allowing_private_hosts()
    }

    /// Tests only: send to private hosts through the shared client, unpinned.
    pub fn allowing_private_hosts(mut self) -> Self {
        self.allow_private = true;
        self
    }

    pub fn allows_private_hosts(&self) -> bool {
        self.allow_private
    }

    /// The shared pooled client, unguarded: for peer sync, whose sources are
    /// operator-configured content servers (a development peer may be on the
    /// loopback), and the RPC validator.
    pub fn raw(&self) -> &reqwest::Client {
        &self.shared
    }

    pub fn request(&self, method: reqwest::Method, url: impl reqwest::IntoUrl) -> GuardedRequest {
        GuardedRequest {
            outbound: self.clone(),
            builder: self.shared.request(method, url),
        }
    }

    pub fn get(&self, url: impl reqwest::IntoUrl) -> GuardedRequest {
        self.request(reqwest::Method::GET, url)
    }

    pub fn post(&self, url: impl reqwest::IntoUrl) -> GuardedRequest {
        self.request(reqwest::Method::POST, url)
    }

    /// Sends `req` through the guard, following up to `max_redirects` hops
    /// with the guard re-run on each. A body that cannot be cloned (a
    /// stream) is sent once and its redirect, if any, returned as is.
    pub async fn execute(&self, mut req: reqwest::Request) -> Result<reqwest::Response, SendError> {
        if self.allow_private {
            return Ok(self.shared.execute(req).await?);
        }
        for hop in 0..=self.max_redirects {
            let client = self.client_for(req.url()).await?;
            let resp = match req.try_clone() {
                Some(copy) => client.execute(copy).await?,
                None => return Ok(client.execute(req).await?),
            };
            let status = resp.status();
            if !status.is_redirection() || hop == self.max_redirects {
                return Ok(resp);
            }
            let Some(location) = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
            else {
                return Ok(resp);
            };
            let next = req.url().join(location).map_err(|e| {
                SendError::Refused(format!("redirect target {location:?} is not a URL: {e}"))
            })?;
            let method = req.method().clone();
            match status.as_u16() {
                303 => {
                    *req.method_mut() = reqwest::Method::GET;
                    *req.body_mut() = None;
                }
                301 | 302 if method == reqwest::Method::POST => {
                    *req.method_mut() = reqwest::Method::GET;
                    *req.body_mut() = None;
                }
                301 | 302 | 307 | 308 => {}
                _ => return Ok(resp),
            }
            if next.host_str() != req.url().host_str() {
                let headers = req.headers_mut();
                headers.remove(reqwest::header::AUTHORIZATION);
                headers.remove(reqwest::header::COOKIE);
            }
            tracing::debug!(from = %req.url(), to = %next, status = status.as_u16(), "following a redirect through the guard");
            *req.url_mut() = next;
        }
        unreachable!("the loop returns on its last hop")
    }

    /// The client to dial `url` with: the guard's verdict and the socket come
    /// from the same lookup. A literal public address needs no pin.
    async fn client_for(&self, url: &reqwest::Url) -> Result<reqwest::Client, SendError> {
        let Some(host) = url.host_str() else {
            return Err(SendError::Refused("URL has no host".into()));
        };
        if !matches!(url.scheme(), "http" | "https") {
            return Err(SendError::Refused(format!(
                "scheme {}:// is not http(s)",
                url.scheme()
            )));
        }
        if !is_safe_http_url(url.as_str()) {
            return Err(SendError::Refused(format!(
                "{host} refused by the SSRF guard (private, internal or single-label host)"
            )));
        }
        let bare = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        if bare.parse::<IpAddr>().is_ok() {
            return Ok(self.direct.clone());
        }
        let key = host.to_ascii_lowercase();
        if let Some(p) = self.pinned.lock().unwrap().get(&key)
            && p.resolved_at.elapsed() < PIN_TTL
        {
            return Ok(p.client.clone());
        }
        #[cfg(test)]
        let test_pin = self.test_pins.lock().unwrap().get(&key).copied();
        #[cfg(not(test))]
        let test_pin: Option<SocketAddr> = None;
        let addr = match test_pin {
            Some(addr) => addr,
            None => resolve_and_pin(url).await.ok_or_else(|| {
                SendError::Refused(format!(
                    "{host} does not resolve to a public address (or does not resolve)"
                ))
            })?,
        };
        let mut pinned = self.pinned.lock().unwrap();
        if let Some(p) = pinned.get_mut(&key)
            && p.addr.ip() == addr.ip()
        {
            p.resolved_at = Instant::now();
            return Ok(p.client.clone());
        }
        let client = HttpClientCfg {
            max_redirects: None,
            ..(*self.cfg).clone()
        }
        .builder()
        .resolve(host, addr)
        .build()?;
        if pinned.len() >= PIN_CACHE_MAX {
            pinned.retain(|_, p| p.resolved_at.elapsed() < PIN_TTL);
            if pinned.len() >= PIN_CACHE_MAX
                && let Some(oldest) = pinned
                    .iter()
                    .min_by_key(|(_, p)| p.resolved_at)
                    .map(|(k, _)| k.clone())
            {
                pinned.remove(&oldest);
            }
        }
        tracing::debug!(host, %addr, "pinned an upstream host");
        pinned.insert(
            key,
            Pinned {
                addr,
                resolved_at: Instant::now(),
                client: client.clone(),
            },
        );
        Ok(client)
    }

    #[cfg(test)]
    fn pinned_hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self.pinned.lock().unwrap().keys().cloned().collect();
        hosts.sort();
        hosts
    }
}

/// A request under construction against an [`Outbound`]: the builder
/// methods the readers use, then [`GuardedRequest::send`], which is the only
/// way to put it on the wire.
pub struct GuardedRequest {
    outbound: Outbound,
    builder: reqwest::RequestBuilder,
}

impl GuardedRequest {
    fn map(mut self, f: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder) -> Self {
        self.builder = f(self.builder);
        self
    }

    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        reqwest::header::HeaderName: TryFrom<K>,
        <reqwest::header::HeaderName as TryFrom<K>>::Error: Into<axum::http::Error>,
        reqwest::header::HeaderValue: TryFrom<V>,
        <reqwest::header::HeaderValue as TryFrom<V>>::Error: Into<axum::http::Error>,
    {
        self.map(|b| b.header(key, value))
    }

    pub fn headers(self, headers: reqwest::header::HeaderMap) -> Self {
        self.map(|b| b.headers(headers))
    }

    pub fn timeout(self, timeout: Duration) -> Self {
        self.map(|b| b.timeout(timeout))
    }

    pub fn json<T: serde::Serialize + ?Sized>(self, json: &T) -> Self {
        self.map(|b| b.json(json))
    }

    pub fn body<T: Into<reqwest::Body>>(self, body: T) -> Self {
        self.map(|b| b.body(body))
    }

    pub async fn send(self) -> Result<reqwest::Response, SendError> {
        let req = self.builder.build()?;
        self.outbound.execute(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_is_the_package_version() {
        assert_eq!(
            USER_AGENT,
            format!("catalyst/{}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[tokio::test]
    async fn reserved_names_never_reach_the_system_resolver() {
        use reqwest::dns::Resolve;
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = asked.clone();
        let resolver = ReservedNamesResolver::over(move |host| {
            seen.lock().unwrap().push(host);
            Box::pin(async {
                Ok(
                    Box::new(std::iter::once(SocketAddr::from(([192, 0, 2, 1], 0))))
                        as reqwest::dns::Addrs,
                )
            })
        });
        for host in ["fixtures.invalid", "Peer.Example.INVALID.", "invalid"] {
            let answer = resolver.resolve(host.parse().unwrap()).await;
            assert!(answer.is_err(), "{host} resolved");
        }
        for host in ["localhost", "catalyst.localhost", "Node.LOCALHOST."] {
            let addrs: Vec<SocketAddr> = resolver
                .resolve(host.parse().unwrap())
                .await
                .unwrap()
                .collect();
            assert!(
                !addrs.is_empty() && addrs.iter().all(|a| a.ip().is_loopback()),
                "{host}: {addrs:?}"
            );
        }
        assert!(
            asked.lock().unwrap().is_empty(),
            "asked the system resolver for {:?}",
            asked.lock().unwrap()
        );
        let addrs: Vec<SocketAddr> = resolver
            .resolve("peer.example.org".parse().unwrap())
            .await
            .unwrap()
            .collect();
        assert_eq!(addrs, [SocketAddr::from(([192, 0, 2, 1], 0))]);
        assert_eq!(*asked.lock().unwrap(), ["peer.example.org"]);
    }

    #[tokio::test]
    async fn every_built_client_answers_reserved_names_itself() {
        let client = HttpClientCfg::default()
            .with_total_timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let err = client
            .get("http://catalyst-test.invalid/")
            .send()
            .await
            .unwrap_err();
        let mut chain = Vec::new();
        let mut source: Option<&dyn std::error::Error> = Some(&err);
        while let Some(e) = source {
            chain.push(e.to_string());
            source = e.source();
        }
        assert!(chain.iter().any(|e| e.contains("RFC 6761")), "{chain:?}");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let resp = client
            .get(format!("http://catalyst-test.localhost:{port}/"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);
    }

    #[tokio::test]
    async fn the_guard_pins_no_reserved_name() {
        for url in [
            "http://catalyst-test.invalid/",
            "https://catalyst-test.localhost/",
        ] {
            let parsed = reqwest::Url::parse(url).unwrap();
            assert!(resolve_and_pin(&parsed).await.is_none(), "{url}");
        }
    }

    #[test]
    fn reserved_name_reads_rfc_6761_suffixes_only() {
        assert_eq!(reserved_name("a.invalid"), Some(ReservedName::Invalid));
        assert_eq!(reserved_name("A.INVALID."), Some(ReservedName::Invalid));
        assert_eq!(reserved_name("localhost"), Some(ReservedName::Loopback));
        assert_eq!(reserved_name("x.localhost."), Some(ReservedName::Loopback));
        for public in [
            "invalid.example.org",
            "notinvalid",
            "localhost.example.org",
            "mylocalhost",
        ] {
            assert_eq!(reserved_name(public), None, "{public}");
        }
    }

    #[test]
    fn defaults_do_not_follow_redirects() {
        assert!(HttpClientCfg::default().max_redirects.is_none());
    }

    #[test]
    fn builders_produce_a_client() {
        assert!(try_http_client(&HttpClientCfg::default()).is_ok());
        let cfg = HttpClientCfg::default()
            .with_total_timeout(Duration::from_secs(5))
            .with_connect_timeout(Duration::from_secs(2))
            .with_pool(Duration::from_secs(30), 4)
            .following_redirects(3)
            .with_user_agent("catalyst-test/1");
        assert_eq!(cfg.max_redirects, Some(3));
        assert!(try_http_client(&cfg).is_ok());
        let _ = http_client("test", &cfg);
    }

    #[test]
    fn the_serve_client_keeps_the_old_sync_limits() {
        let cfg = HttpClientCfg::serve(25);
        assert_eq!(
            cfg.total_timeout, None,
            "snapshot downloads have no whole-request budget"
        );
        assert_eq!(cfg.connect_timeout, Duration::from_secs(8));
        assert_eq!(cfg.read_timeout, Some(Duration::from_secs(25)));
        assert_eq!(cfg.pool_max_idle_per_host, 25);
        assert_eq!(cfg.max_redirects, Some(2));
        assert!(cfg.tcp_nodelay);
        assert!(cfg.build().is_ok());
    }

    #[test]
    fn reserved_ranges_and_internal_names_are_blocked() {
        for host in [
            "0.0.0.0",
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.2.1",
            "::1",
            "::",
            "fe80::1",
            "fd00::1",
            "fc00::1",
            "[::1]",
            "[::ffff:127.0.0.1]",
            "localhost",
            "router",
            "svc.internal",
            "office.intranet",
            "box.home",
            "vpn.corp",
            "gw.home.arpa",
            "app.localhost",
            "printer.lan",
            "nas.local",
            "localhost.",
            "nas.local.",
            "router.",
            "127.0.0.1.",
            "LOCALHOST",
            "NAS.Local",
        ] {
            assert!(is_private_or_internal_host(host), "{host} must be blocked");
        }
    }

    #[test]
    fn public_hosts_are_allowed() {
        for host in [
            "8.8.8.8",
            "example.com",
            "example.com.",
            "decentraland.org..",
            "172.15.0.1",
            "172.32.0.1",
            "100.63.0.1",
            "100.128.0.1",
            "events.decentraland.org",
            "2600::1",
            "[2600::1]",
        ] {
            assert!(
                !is_private_or_internal_host(host),
                "{host} must be reachable"
            );
        }
    }

    #[test]
    fn obfuscated_ip_literals_are_blocked() {
        for host in [
            "2130706433",
            "0x7f000001",
            "0177.0.0.1",
            "127.1",
            "0x7f.1",
            "017700000001",
        ] {
            assert!(
                is_private_or_internal_host(host),
                "{host} must resolve to loopback and be blocked"
            );
        }
    }

    #[test]
    fn safe_urls_pass() {
        for url in [
            "https://decentraland.org/",
            "http://example.com/path?q=1",
            "https://example.com./",
            "https://8.8.8.8/",
            "https://[2600::1]/",
        ] {
            assert!(is_safe_http_url(url), "{url} must be safe");
        }
    }

    #[test]
    fn unsafe_schemes_and_hosts_are_rejected() {
        for url in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "file:///etc/passwd",
            "smb://attacker/share",
            "ftp://example.com/",
            "//example.com/",
            "example.com",
            "",
            "http://169.254.169.254/latest/meta-data/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0177.0.0.1/",
            "http://192.168.1.1/",
            "http://localhost:8080/",
            "http://router/",
            "http://printer.lan/",
            "http://nas.local/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://localhost./",
            "http://nas.local./",
            "http://router./",
            "http://10.0.0.1/",
            "http://100.100.0.1/",
        ] {
            assert!(!is_safe_http_url(url), "{url} must be rejected");
        }
    }

    #[test]
    fn whitespace_smuggling_is_rejected() {
        for url in [
            "https://example.com/ javascript:alert(1)",
            "https://exa\tmple.com/",
            "https://example.com/\nhttp://127.0.0.1/",
            "https://example.com/\u{2028}x",
            "java\nscript:alert(1)",
        ] {
            assert!(!is_safe_http_url(url), "{url} must be rejected");
        }
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_not_rejected() {
        assert!(is_safe_http_url("  https://decentraland.org/  "));
        assert!(is_safe_http_url("\n\thttps://decentraland.org/\r\n"));
    }

    #[tokio::test]
    async fn pinning_rejects_literal_private_addresses() {
        for url in [
            "http://127.0.0.1/x",
            "http://169.254.169.254/latest/",
            "http://[::1]/x",
            "http://[::ffff:10.0.0.1]/x",
            "http://router/x",
            "http://svc.internal/x",
        ] {
            let parsed = reqwest::Url::parse(url).unwrap();
            assert!(
                resolve_and_pin(&parsed).await.is_none(),
                "{url} must not be pinned"
            );
        }
    }

    #[tokio::test]
    async fn pinning_accepts_a_public_literal_and_keeps_the_port() {
        let parsed = reqwest::Url::parse("http://8.8.8.8:8080/x").unwrap();
        let pinned = resolve_and_pin(&parsed).await.expect("public literal pins");
        assert_eq!(pinned, "8.8.8.8:8080".parse::<SocketAddr>().unwrap());

        let parsed = reqwest::Url::parse("https://8.8.4.4/x").unwrap();
        let pinned = resolve_and_pin(&parsed).await.expect("public literal pins");
        assert_eq!(pinned.port(), 443, "https must pin the default port");
    }

    #[tokio::test]
    async fn pinning_accepts_a_public_ipv6_literal() {
        let parsed = reqwest::Url::parse("http://[2600::1]:81/x").unwrap();
        let pinned = resolve_and_pin(&parsed).await.expect("public literal pins");
        assert_eq!(pinned, "[2600::1]:81".parse::<SocketAddr>().unwrap());
    }

    /// A loopback stub standing in for public names: `/ok` answers, the
    /// `/to-*` routes redirect (to a literal loopback address, to another
    /// public name, to itself).
    async fn redirect_stub() -> SocketAddr {
        use axum::response::IntoResponse;
        use axum::routing::get;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        let redirect = |to: String| async move {
            (axum::http::StatusCode::FOUND, [("location", to)]).into_response()
        };
        let app = axum::Router::new()
            .route("/ok", get(|| async { "fine" }))
            .route(
                "/to-private",
                get(move || redirect(format!("http://127.0.0.1:{port}/ok"))),
            )
            .route(
                "/to-public",
                get(move || redirect(format!("http://other.example.test:{port}/ok"))),
            )
            .route(
                "/to-self",
                get(move || redirect(format!("http://public.example.test:{port}/to-self"))),
            )
            .route(
                "/to-ftp",
                get(move || redirect("ftp://public.example.test/x".to_string())),
            );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        addr
    }

    fn strict(addr: SocketAddr) -> Outbound {
        Outbound::new(HttpClientCfg::default().following_redirects(2))
            .unwrap()
            .with_test_pin("public.example.test", addr)
            .with_test_pin("other.example.test", addr)
    }

    #[tokio::test]
    async fn the_guard_refuses_private_hosts_before_any_byte_leaves() {
        let out = Outbound::new(HttpClientCfg::default()).unwrap();
        for url in [
            "http://127.0.0.1:9/x",
            "http://router/x",
            "https://169.254.169.254/latest/",
            "ftp://example.com/",
        ] {
            let err = out.get(url).send().await.unwrap_err();
            assert!(matches!(err, SendError::Refused(_)), "{url}: {err}");
            assert!(!err.is_transient(), "a refusal is never retried");
        }
        assert!(out.pinned_hosts().is_empty());
    }

    #[tokio::test]
    async fn a_pinned_public_name_is_dialled_at_its_pinned_address() {
        let addr = redirect_stub().await;
        let out = strict(addr);
        let resp = out
            .get(format!("http://public.example.test:{}/ok", addr.port()))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "fine");
        assert_eq!(out.pinned_hosts(), vec!["public.example.test"]);
        // The second call reuses the pinned client within the TTL.
        out.get(format!("http://public.example.test:{}/ok", addr.port()))
            .send()
            .await
            .unwrap();
        assert_eq!(out.pinned_hosts(), vec!["public.example.test"]);
    }

    #[tokio::test]
    async fn a_redirect_to_a_private_address_is_refused_and_not_followed() {
        let addr = redirect_stub().await;
        let out = strict(addr);
        let err = out
            .get(format!(
                "http://public.example.test:{}/to-private",
                addr.port()
            ))
            .send()
            .await
            .unwrap_err();
        assert!(
            matches!(&err, SendError::Refused(m) if m.contains("127.0.0.1")),
            "{err}"
        );
        let err = out
            .get(format!("http://public.example.test:{}/to-ftp", addr.port()))
            .send()
            .await
            .unwrap_err();
        assert!(matches!(err, SendError::Refused(_)), "{err}");
    }

    #[tokio::test]
    async fn a_redirect_to_another_public_name_is_followed_through_the_guard() {
        let addr = redirect_stub().await;
        let out = strict(addr);
        let resp = out
            .get(format!(
                "http://public.example.test:{}/to-public",
                addr.port()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.text().await.unwrap(), "fine");
        assert_eq!(
            out.pinned_hosts(),
            vec!["other.example.test", "public.example.test"],
            "both hops were pinned"
        );
    }

    #[tokio::test]
    async fn redirects_stop_at_the_hop_limit_with_the_last_answer() {
        let addr = redirect_stub().await;
        let out = strict(addr);
        let resp = out
            .get(format!(
                "http://public.example.test:{}/to-self",
                addr.port()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            302,
            "the 3xx comes back for the caller to treat as a failure"
        );
    }

    #[tokio::test]
    async fn the_shared_client_refuses_a_public_to_private_redirect() {
        let addr = redirect_stub().await;
        let client = HttpClientCfg::default()
            .following_redirects(2)
            .builder()
            .resolve("public.example.test", addr)
            .build()
            .unwrap();
        let err = client
            .get(format!(
                "http://public.example.test:{}/to-private",
                addr.port()
            ))
            .send()
            .await
            .unwrap_err();
        assert!(err.is_redirect(), "{err}");
        // A chain that started on the loopback keeps following the loopback.
        let resp = client
            .get(format!("http://127.0.0.1:{}/to-private", addr.port()))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test]
    async fn for_tests_sends_to_the_loopback_unpinned() {
        let addr = redirect_stub().await;
        let resp = Outbound::for_tests()
            .get(format!("http://{addr}/ok"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[test]
    fn parse_retry_after_integer_seconds() {
        assert_eq!(parse_retry_after("3"), Some(Duration::from_secs(3)));
        assert_eq!(parse_retry_after("0"), Some(Duration::from_secs(0)));
    }

    #[test]
    fn parse_retry_after_http_date_in_future() {
        let future = (chrono::Utc::now() + chrono::Duration::seconds(10)).to_rfc2822();
        let d = parse_retry_after(&future).expect("future date should parse");
        assert!(
            d > Duration::from_secs(8) && d <= Duration::from_secs(10),
            "expected ~10s, got {d:?}"
        );
    }

    #[test]
    fn parse_retry_after_http_date_in_past() {
        let past = (chrono::Utc::now() - chrono::Duration::seconds(60)).to_rfc2822();
        assert_eq!(parse_retry_after(&past), Some(Duration::ZERO));
    }

    #[test]
    fn parse_retry_after_garbage_falls_back() {
        assert_eq!(parse_retry_after("garbage"), Some(Duration::from_secs(5)));
    }

    #[test]
    fn parse_retry_after_empty_is_none() {
        assert_eq!(parse_retry_after(""), None);
        assert_eq!(parse_retry_after("   "), None);
    }

    #[test]
    fn transient_statuses() {
        assert!(is_transient_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_transient_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(is_transient_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(!is_transient_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_transient_status(reqwest::StatusCode::OK));
    }

    async fn drive_schedule(
        attempts: u32,
        base_ms: u64,
        after: Option<Duration>,
        rate_limited: bool,
    ) -> Vec<Duration> {
        let times: Mutex<Vec<Duration>> = Mutex::new(Vec::new());
        let times_ref = &times;
        let start = tokio::time::Instant::now();
        let result: Option<()> =
            retry_with_backoff("test", attempts, base_ms, move || async move {
                times_ref.lock().unwrap().push(start.elapsed());
                Ok(RetryDecision::Retry {
                    after,
                    rate_limited,
                })
            })
            .await
            .unwrap();
        assert!(result.is_none());
        times.into_inner().unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limited_ladder_is_2_4_8_16_over_5_attempts() {
        let times = drive_schedule(3, 1000, None, true).await;
        let expected: Vec<Duration> = [0u64, 2, 6, 14, 30]
            .iter()
            .map(|s| Duration::from_secs(*s))
            .collect();
        assert_eq!(times, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn plain_transient_keeps_caller_schedule() {
        let times = drive_schedule(3, 1000, None, false).await;
        let expected: Vec<Duration> = [0u64, 1, 3]
            .iter()
            .map(|s| Duration::from_secs(*s))
            .collect();
        assert_eq!(times, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn rate_limited_honours_retry_after_hint() {
        let times = drive_schedule(3, 1000, Some(Duration::from_secs(7)), true).await;
        assert_eq!(times.len(), 5);
        assert_eq!(times[1], Duration::from_secs(7));
        assert_eq!(times[2], Duration::from_secs(14));
    }

    #[tokio::test(start_paused = true)]
    async fn sleeps_are_clamped_to_30s() {
        let times = drive_schedule(3, 1000, Some(Duration::from_secs(120)), true).await;
        assert_eq!(times[1], Duration::from_secs(30));
    }

    #[tokio::test(start_paused = true)]
    async fn done_short_circuits() {
        let result =
            retry_with_backoff("test", 3, 1000, || async { Ok(RetryDecision::Done(42u32)) })
                .await
                .unwrap();
        assert_eq!(result, Some(42));
    }

    #[tokio::test(start_paused = true)]
    async fn operation_errors_propagate() {
        let r: Result<Option<u32>> = retry_with_backoff("test", 3, 1000, || async {
            Err(anyhow::anyhow!("hard failure"))
        })
        .await;
        assert_eq!(r.unwrap_err().to_string(), "hard failure");
    }

    fn sized(body: &'static str) -> reqwest::Response {
        reqwest::Response::from(axum::http::Response::new(body.to_string()))
    }

    fn chunked(chunks: Vec<&'static str>) -> reqwest::Response {
        let stream = futures::stream::iter(
            chunks
                .into_iter()
                .map(|c| Ok::<_, std::io::Error>(bytes::Bytes::from_static(c.as_bytes()))),
        );
        reqwest::Response::from(axum::http::Response::new(reqwest::Body::wrap_stream(
            stream,
        )))
    }

    #[tokio::test]
    async fn advertised_length_over_the_cap_fails_before_reading() {
        let err = read_body_capped(sized("hello world"), 4)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("advertises 11 bytes"), "{err}");
    }

    #[tokio::test]
    async fn streamed_body_over_the_cap_fails_mid_read() {
        let err = read_body_capped(chunked(vec!["aaaa", "bbbb", "cccc"]), 6)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("exceeds the 6 byte cap"), "{err}");
    }

    #[tokio::test]
    async fn streamed_body_under_the_cap_is_concatenated() {
        let out = read_body_capped(chunked(vec!["aa", "bb", "cc"]), 6)
            .await
            .unwrap();
        assert_eq!(out, b"aabbcc");
    }

    #[tokio::test]
    async fn a_body_exactly_at_the_cap_is_accepted() {
        let out = read_body_capped(sized("hello"), 5).await.unwrap();
        assert_eq!(out, b"hello");
    }
}

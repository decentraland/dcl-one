//! The explore router's CORS, as the old places and events services answered
//! it through the `cors` package (decentraland-gatsby `Route/middleware/withCors.js`,
//! `Route/types.js:4-67`):
//!
//! - `/api/*`: an allow-list of origins (`places/src/server.ts:64-72` plus the
//!   per-router list `Place/routes/index.ts:24-38` that every later router
//!   shares; `events/src/server.ts:49-58`). A listed `Origin` is echoed in
//!   `Access-Control-Allow-Origin`; a non-listed one gets no allow-origin at
//!   all; `Vary: Origin` always. A preflight short-circuits to 204 with
//!   `Access-Control-Allow-Methods: GET,HEAD,PUT,PATCH,POST,DELETE` and
//!   `Access-Control-Allow-Headers: *` (`allowedHeaders: "*"`). No credentials
//!   header: the `cors` default. The matched origin also gets the gatsby
//!   default `Access-Control-Expose-Headers`.
//! - The public events reads (`Event/routes/index.ts:68-96`,
//!   `EventCategory/routes.ts:10-15`: `withCors({cors: "*"})`) answer
//!   `Access-Control-Allow-Origin: *` on the actual request whatever the
//!   origin; their preflight is still the app-level allow-list.
//! - `/places/place` and `/places/world` (the social cards,
//!   `places/src/server.ts:90`) carry no CORS headers.
//! - Every other path (`/v1/*`, `/federation/*`, the sitemaps, the posters,
//!   `/openapi.json`) keeps the node's shared reflecting CORS
//!   (`shared::cors`): upstream never had them.
//!
//! The lists are configurable per node ([`Policy`]); the defaults are
//! upstream's regular expressions, anchored. Upstream left the router list
//! and two app patterns unanchored (`RegExp.test` matches anywhere), so
//! `https://decentraland.org.evil.com` was echoed; no legitimate client
//! relied on that, and the anchors are the one deliberate deviation of this
//! port. A configured pattern must be anchored too, or it reopens the hole.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use regex::Regex;

use crate::SharedState;

const ALLOW_METHODS: &str = "GET,HEAD,PUT,PATCH,POST,DELETE";
const ALLOW_HEADERS: &str = "*";
/// decentraland-gatsby `Route/types.js:59-67`.
const EXPOSE_HEADERS: &str =
    "ETag,Cache-Control,Content-Language,Content-Type,Expires,Last-Modified,Pragma";

/// `places/src/server.ts:65-70`: the app-level list every `/api` request meets.
pub const PLACES_APP_ORIGINS: &[&str] = &[
    r"^http://localhost:[0-9]{1,10}$",
    r"^https://(.{1,50}\.)?decentraland\.(zone|today|org)$",
    r"^https://dcl-preview\.vercel\.app$",
    r"^https://([a-zA-Z0-9\-_])+-decentraland1\.vercel\.app$",
];

/// `places/src/entities/Place/routes/index.ts:25-37` (the World, Map and
/// Destination routers repeat it): added for the requests that reach the
/// place router and the routers behind it. Anchored here (see the module
/// doc).
pub const PLACES_ROUTER_ORIGINS: &[&str] = &[
    r"^https?://localhost(:\d{4,6})?$",
    r"^https?://127\.0\.0\.1(:\d{4,6})?$",
    r"^https?://192\.168\.\d{1,3}\.\d{1,3}(:\d{4,6})?$",
    r"^https://([a-zA-Z0-9\-_]+\.)*dcl\.gg$",
    r"^https://([a-zA-Z0-9\-_]+\.)*decentraland\.systems$",
    r"^https://([a-zA-Z0-9\-_]+\.)*decentraland\.today$",
    r"^https://([a-zA-Z0-9\-_]+\.)*decentraland\.zone$",
    r"^https://([a-zA-Z0-9\-_]+\.)*decentraland\.org$",
    r"^https://decentraland\.github\.io$",
    r"^https://([a-zA-Z0-9\-_]+\.)*pages\.dev$",
    r"^https://([a-zA-Z0-9\-_])+-decentraland1\.vercel\.app$",
];

/// `events/src/server.ts:50-56` (the two literal origins as anchored patterns).
pub const EVENTS_APP_ORIGINS: &[&str] = &[
    r"^http://localhost:[0-9]{1,10}$",
    r"^https://(?:[a-zA-Z0-9_-]+\.)*decentraland\.(?:zone|today|org|systems)$",
    r"^https://[a-zA-Z0-9_-]+-decentraland1\.vercel\.app$",
    r"^https://mvfw\.org$",
    r"^https://dcl-metrics\.com$",
];

/// An origin allow-list: the `cors` package's `origin: RegExp[]`
/// (`RegExp.test`); every pattern, configured ones included, is anchored
/// with `^` and `$`.
#[derive(Debug, Clone)]
pub struct OriginList(Vec<Regex>);

impl OriginList {
    /// Compiles JavaScript-style patterns; `\d` becomes ASCII digits.
    pub fn new(patterns: &[&str]) -> Self {
        Self::try_new(patterns).unwrap_or_else(|e| panic!("CORS origin pattern: {e}"))
    }

    /// [`OriginList::new`] as a result: the error names the pattern that is
    /// not anchored at both ends or does not compile. The anchors are
    /// required because `RegExp.test` matches anywhere, so an unanchored
    /// `decentraland\.org` would echo `https://decentraland.org.evil.com`.
    pub fn try_new(patterns: &[&str]) -> Result<Self, String> {
        patterns
            .iter()
            .map(|p| {
                if !anchored(p) {
                    return Err(format!(
                        "{p:?}: an origin pattern must start with ^ and end with $"
                    ));
                }
                // The wrap anchors every alternative, not only the outer ones.
                Regex::new(&format!("^(?:{})$", ascii_digits(p))).map_err(|e| format!("{p:?}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    pub fn allows(&self, origin: &str) -> bool {
        self.0.iter().any(|re| re.is_match(origin))
    }
}

/// `^` first and an unescaped `$` last.
fn anchored(pattern: &str) -> bool {
    let Some(body) = pattern.strip_prefix('^').and_then(|p| p.strip_suffix('$')) else {
        return false;
    };
    body.chars().rev().take_while(|c| *c == '\\').count() % 2 == 0
}

/// `\d` as JavaScript reads it, ASCII digits; every other escape, an escaped
/// backslash included, stays as written.
fn ascii_digits(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('d') => out.push_str("[0-9]"),
            Some(escaped) => {
                out.push('\\');
                out.push(escaped);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// The node's explore CORS lists. [`Policy::upstream`] is what
/// places.decentraland.org and events.decentraland.org answer.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Preflights and the favorites, likes and categories routes (places).
    pub places_app: OriginList,
    /// The remaining places `/api` routes: the app list plus the router list.
    pub places_routes: OriginList,
    /// Every events `/api` route; the public reads add `*` on the actual request.
    pub events_app: OriginList,
}

impl Policy {
    pub fn upstream() -> Self {
        Self::upstream_with(&[]).expect("the upstream lists compile")
    }

    /// Upstream's lists with `extra` (`EXPLORE_CORS_ORIGINS`, JavaScript-style
    /// patterns) appended to each; a pattern that is not anchored or does not
    /// compile is the error.
    pub fn upstream_with(extra: &[String]) -> Result<Self, String> {
        let extra: Vec<&str> = extra
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .collect();
        let list = |defaults: &[&str]| {
            let mut patterns: Vec<&str> = defaults.to_vec();
            patterns.extend_from_slice(&extra);
            OriginList::try_new(&patterns)
        };
        let mut routes: Vec<&str> = PLACES_APP_ORIGINS.to_vec();
        routes.extend_from_slice(PLACES_ROUTER_ORIGINS);
        Ok(Self {
            places_app: list(PLACES_APP_ORIGINS)?,
            places_routes: list(&routes)?,
            events_app: list(EVENTS_APP_ORIGINS)?,
        })
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::upstream()
    }
}

/// Which upstream router a path under `/api` belonged to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The places app-level list only (`categoryRoute`, `userFavoriteRoute`,
    /// `userLikesRoute` run before `placeRoute`'s `use(withCors)`).
    PlacesApp,
    /// The places app list plus the router list.
    PlacesRoutes,
    /// An events route under the app list.
    Events,
    /// An events route that also answers `*` on the actual request.
    EventsPublic,
    /// No CORS at all (the social cards).
    None,
    /// Not an upstream path: the node's shared CORS applies.
    Shared,
}

/// `path` is the explore router's own path (the `/places` and `/events`
/// mounts strip their prefix before the layer sees it).
pub fn surface(method: &Method, path: &str) -> Surface {
    let path = path.trim_end_matches('/');
    if path == "/places/place" || path == "/places/world" {
        return Surface::None;
    }
    if !path.starts_with("/api/") && path != "/api" {
        return Surface::Shared;
    }
    if path.starts_with("/api/events")
        || path.starts_with("/api/schedules")
        || path.starts_with("/api/profiles")
        || path.starts_with("/api/profile/")
        || path.starts_with("/api/poster")
    {
        let public = match (method, path) {
            (&Method::GET, "/api/events" | "/api/events/attending" | "/api/events/categories") => {
                true
            }
            (&Method::POST, "/api/events/search") => true,
            (&Method::GET, p) => p
                .strip_prefix("/api/events/")
                .is_some_and(|rest| !rest.is_empty() && !rest.contains('/')),
            _ => false,
        };
        return if public {
            Surface::EventsPublic
        } else {
            Surface::Events
        };
    }
    if path == "/api/categories"
        || (path.starts_with("/api/places/")
            && (path.ends_with("/favorites") || path.ends_with("/likes")))
    {
        return Surface::PlacesApp;
    }
    Surface::PlacesRoutes
}

fn allowed(policy: &Policy, surface: Surface, origin: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return false;
    };
    match surface {
        Surface::PlacesApp => policy.places_app.allows(origin),
        Surface::PlacesRoutes => policy.places_routes.allows(origin),
        Surface::Events | Surface::EventsPublic => policy.events_app.allows(origin),
        Surface::None | Surface::Shared => false,
    }
}

/// The explore router's CORS layer. Paths upstream never served fall through
/// to [`crate::shared::cors::cors_middleware`].
pub async fn explore_cors(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let surface = surface(request.method(), request.uri().path());
    match surface {
        Surface::Shared => return crate::shared::cors::cors_middleware(request, next).await,
        Surface::None => return next.run(request).await,
        _ => {}
    }
    let policy: Arc<Policy> = state.cors.clone();
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let echo = allowed(&policy, surface, origin.as_deref());

    if request.method() == Method::OPTIONS {
        let mut resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .unwrap();
        let h = resp.headers_mut();
        if echo
            && let Some(origin) = origin
                .as_deref()
                .and_then(|o| HeaderValue::from_str(o).ok())
        {
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
        h.insert(header::VARY, HeaderValue::from_static("Origin"));
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static(ALLOW_METHODS),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static(ALLOW_HEADERS),
        );
        return resp;
    }

    let mut resp = next.run(request).await;
    let h = resp.headers_mut();
    h.insert(header::VARY, HeaderValue::from_static("Origin"));
    let allow_origin = if surface == Surface::EventsPublic {
        Some(HeaderValue::from_static("*"))
    } else if echo {
        origin
            .as_deref()
            .and_then(|o| HeaderValue::from_str(o).ok())
    } else {
        None
    };
    if let Some(value) = allow_origin {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        h.insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static(EXPOSE_HEADERS),
        );
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_lists_echo_listed_origins_only() {
        let policy = Policy::upstream();
        for ok in [
            "https://decentraland.org",
            "https://play.decentraland.org",
            "https://decentraland.zone",
            "http://localhost:5173",
            "https://dcl-preview.vercel.app",
            "https://feature-decentraland1.vercel.app",
        ] {
            assert!(policy.places_app.allows(ok), "{ok}");
            assert!(policy.places_routes.allows(ok), "{ok}");
        }
        for ok in [
            "https://decentraland.org",
            "https://events.decentraland.systems",
            "https://mvfw.org",
            "https://dcl-metrics.com",
            "http://localhost:8000",
        ] {
            assert!(policy.events_app.allows(ok), "{ok}");
        }
        // The router list is anchored too, so a listed domain used as a
        // prefix, a suffix or a path is refused on every places route.
        for bad in [
            "https://example.com",
            "https://decentraland.org.evil.com",
            "https://evil.com/https://decentraland.org",
            "https://dcl-preview.vercel.app.evil.com",
            "https://foo.pages.dev.evil.com",
            "http://localhost:3000.evil.com",
            "http://decentraland.org",
            "https://mvfw.org.example",
        ] {
            assert!(!policy.places_app.allows(bad), "{bad}");
            assert!(!policy.places_routes.allows(bad), "{bad}");
            assert!(!policy.events_app.allows(bad), "{bad}");
        }
        // The router list is broader: dcl.gg, pages.dev, github.io, LAN hosts.
        for ok in [
            "https://foo.pages.dev",
            "https://decentraland.github.io",
            "http://192.168.1.20:8080",
            "https://x.dcl.gg",
        ] {
            assert!(!policy.places_app.allows(ok), "{ok}");
            assert!(policy.places_routes.allows(ok), "{ok}");
            assert!(!policy.events_app.allows(ok), "{ok}");
        }
    }

    #[test]
    fn configured_origins_extend_every_upstream_list() {
        let policy = Policy::upstream_with(&[
            r"^https://explore\.example\.org$".to_string(),
            "  ".to_string(),
            r"^http://10\.0\.0\.\d+:\d+$".to_string(),
        ])
        .unwrap();
        for surface in [Surface::PlacesApp, Surface::PlacesRoutes, Surface::Events] {
            assert!(allowed(
                &policy,
                surface,
                Some("https://explore.example.org")
            ));
            assert!(allowed(&policy, surface, Some("http://10.0.0.7:8080")));
            assert!(allowed(&policy, surface, Some("https://decentraland.org")));
            assert!(!allowed(
                &policy,
                surface,
                Some("https://explore.example.org.evil")
            ));
            assert!(!allowed(&policy, surface, Some("https://example.com")));
        }
        assert!(
            Policy::upstream_with(&["^https://(unclosed$".to_string()])
                .unwrap_err()
                .contains("unclosed"),
            "an invalid pattern is named"
        );
    }

    #[test]
    fn configured_patterns_must_be_anchored() {
        for unanchored in [
            r"https://explore\.example\.org$",
            r"^https://explore\.example\.org",
            r"explore\.example\.org",
            r"^https://explore\.example\.org\$",
        ] {
            let err = Policy::upstream_with(&[unanchored.to_string()]).unwrap_err();
            assert!(err.contains(&format!("{unanchored:?}")), "{err}");
            assert!(err.contains("start with ^ and end with $"), "{err}");
        }
        let policy =
            Policy::upstream_with(&[r"^https://explore\.example\.org$".to_string()]).unwrap();
        assert!(policy.events_app.allows("https://explore.example.org"));
        assert!(!policy.events_app.allows("https://explore.example.org.evil"));
        let alternation =
            Policy::upstream_with(&[r"^https://a\.example$|https://b\.example$".to_string()])
                .unwrap();
        assert!(alternation.events_app.allows("https://b.example"));
        assert!(!alternation.events_app.allows("evil-https://b.example"));
    }

    #[test]
    fn digit_escapes_are_ascii_digits_and_escaped_backslashes_stay() {
        let digits = OriginList::new(&[r"^\d+$"]);
        assert!(digits.allows("0123456789"));
        assert!(
            !digits.allows("\u{661}\u{662}"),
            "Arabic-Indic digits are not ASCII"
        );
        assert!(!digits.allows("a"));
        let backslash_d = OriginList::new(&[r"^a\\d$"]);
        assert!(backslash_d.allows(r"a\d"));
        assert!(!backslash_d.allows("a5"));
        assert_eq!(ascii_digits(r"\d\\d\\\d\."), r"[0-9]\\d\\[0-9]\.");
    }

    #[test]
    fn surfaces_follow_the_upstream_mounts() {
        let get = Method::GET;
        assert_eq!(surface(&get, "/api/places"), Surface::PlacesRoutes);
        assert_eq!(surface(&get, "/api/destinations"), Surface::PlacesRoutes);
        assert_eq!(surface(&get, "/api/status"), Surface::PlacesRoutes);
        assert_eq!(surface(&get, "/api/nope"), Surface::PlacesRoutes);
        assert_eq!(surface(&get, "/api/categories"), Surface::PlacesApp);
        assert_eq!(
            surface(&Method::PATCH, "/api/places/x/favorites"),
            Surface::PlacesApp
        );
        assert_eq!(
            surface(&Method::PATCH, "/api/worlds/x/likes"),
            Surface::PlacesRoutes
        );
        assert_eq!(surface(&get, "/api/events"), Surface::EventsPublic);
        assert_eq!(
            surface(&get, "/api/events/categories"),
            Surface::EventsPublic
        );
        assert_eq!(
            surface(&get, "/api/events/attending"),
            Surface::EventsPublic
        );
        assert_eq!(surface(&get, "/api/events/some-id"), Surface::EventsPublic);
        assert_eq!(
            surface(&Method::POST, "/api/events/search"),
            Surface::EventsPublic
        );
        assert_eq!(surface(&Method::POST, "/api/events"), Surface::Events);
        assert_eq!(
            surface(&Method::PATCH, "/api/events/some-id"),
            Surface::Events
        );
        assert_eq!(
            surface(&get, "/api/events/some-id/attendees"),
            Surface::Events
        );
        assert_eq!(surface(&get, "/api/schedules"), Surface::Events);
        assert_eq!(surface(&Method::POST, "/api/poster"), Surface::Events);
        assert_eq!(surface(&get, "/places/place"), Surface::None);
        assert_eq!(surface(&get, "/places/world/"), Surface::None);
        assert_eq!(surface(&get, "/v1/destinations"), Surface::Shared);
        assert_eq!(surface(&get, "/events/sitemap.xml"), Surface::Shared);
        assert_eq!(surface(&get, "/openapi.json"), Surface::Shared);
    }
}

//! The places social cards: `GET /places/place?id=|position=` and
//! `/places/world?id=|name=` answer a bare HTML document whose title,
//! description, image and canonical URL are the place's or the world's
//! (Open Graph and Twitter tags), for the link-preview crawlers the places
//! site's `/place/` and `/world/` URLs send here.
//!
//! The document is the old service's byte for byte (places e660fbc2
//! `Social/routes.ts`, rendered by decentraland-gatsby 9.0.1
//! `entities/Gatsby/utils.js` `replaceHelmetMetadata`; captures
//! `places.social_place.*`, `places.social_world.*`): the `social.place`
//! copy of `intl/en.json` is the generic card, every value is HTML-escaped
//! once by the renderer and the title and description a second time by the
//! route (so a quote reads `&amp;#39;`, as in production), the description is
//! cut at its first blank line, and the canonical URL is built on
//! `PLACES_BASE_URL` (`Place/utils.ts:23-39`: pathname `/places/place/` with
//! `?position=x.y`, dot-glued, or `/places/world/?name=`). The generic card's
//! URL is the site root plus the request's own path and query
//! (`Social/routes.ts:66-70`). `id` is looked up only when it is a UUID
//! (`validator/lib/isUUID`), `position` through `toCanonicalPosition`. The
//! image goes through the catalogue's URL sanitizer at the sink, as upstream
//! did.

use axum::extract::{OriginalUri, Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};

use super::PlaceRow;
use crate::SharedState;
use crate::sanitize::sanitize_image_url;

/// `Place/utils.ts:25`'s default.
pub const DEFAULT_BASE_URL: &str = "https://decentraland.org/places";

/// `social.place` of `intl/en.json`, the generic card and the keys every
/// card carries (`Social/routes.ts:53-63`, `:67-70`), in their object order.
const GENERIC_TITLE: &str = "Decentraland Place";
const GENERIC_DESCRIPTION: &str = "This place is awesome";
const GENERIC_IMAGE: &str = "image";
const COPY_SITE: &str = "site";
const COPY_CARD: &str = "card";
const COPY_CREATOR: &str = "creator";

/// `html-escaper`'s `escape`: the five characters, in its spellings.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The renderer's `escape` (`Gatsby/utils.js:6-8`): `html-escaper` then
/// every run of newlines to one space.
fn helmet_escape(s: &str) -> String {
    let escaped = html_escape(s);
    let mut out = String::with_capacity(escaped.len());
    let mut in_newlines = false;
    for c in escaped.chars() {
        if c == '\n' {
            if !in_newlines {
                out.push(' ');
            }
            in_newlines = true;
        } else {
            in_newlines = false;
            out.push(c);
        }
    }
    out
}

/// The renderer's `description` rule (`Gatsby/utils.js:38-50`): trimmed,
/// cut at the first blank line, trimmed again.
fn first_paragraph(description: &str) -> &str {
    let d = description.trim();
    match d.find("\n\n") {
        Some(i) if i > 0 => d[..i].trim(),
        _ => d,
    }
}

/// What a card says: the route's values before the renderer escapes them.
struct Card<'a> {
    /// Already `html-escaper`-escaped by the route (`Social/routes.ts:55`).
    title: String,
    /// Already escaped by the route (`:56`); the renderer cuts and escapes again.
    description: String,
    image: &'a str,
    url: &'a str,
    /// `twitter:card`, on a found row only.
    large_image: bool,
}

impl Card<'_> {
    fn found<'a>(title: &str, description: &str, image: &'a str, url: &'a str) -> Card<'a> {
        Card {
            title: format!("{} | Decentraland Place", html_escape(title)),
            description: html_escape(description.trim()),
            image,
            url,
            large_image: true,
        }
    }

    fn generic(url: &str) -> Card<'_> {
        Card {
            title: GENERIC_TITLE.to_string(),
            description: GENERIC_DESCRIPTION.to_string(),
            image: GENERIC_IMAGE,
            url,
            large_image: false,
        }
    }
}

/// `replaceHelmetMetadata(SOCIAL_HTML_TEMPLATE, {...})` (`Gatsby/utils.js`):
/// the injected tags in the metadata object's key order, each stamped
/// `data-react-helmet="true"`, in the bare template.
fn render(card: &Card<'_>) -> String {
    let title = helmet_escape(&card.title);
    let description = helmet_escape(first_paragraph(&card.description));
    let image = helmet_escape(card.image);
    let url = helmet_escape(card.url);
    let mut html = String::from("<!DOCTYPE html><html><head>");
    html.push_str(&format!(
        "<title data-react-helmet=\"true\">{title}</title>"
    ));
    let meta = |html: &mut String, attr: &str, key: &str, value: &str| {
        html.push_str(&format!(
            "<meta {attr}=\"{key}\" content=\"{value}\" data-react-helmet=\"true\">"
        ));
    };
    meta(&mut html, "name", "twitter:title", &title);
    meta(&mut html, "property", "og:title", &title);
    meta(&mut html, "name", "description", &description);
    meta(&mut html, "name", "twitter:description", &description);
    meta(&mut html, "property", "og:description", &description);
    meta(&mut html, "name", "twitter:image", &image);
    meta(&mut html, "property", "og:image", &image);
    meta(&mut html, "name", "site", COPY_SITE);
    meta(&mut html, "name", "card", COPY_CARD);
    meta(&mut html, "name", "creator", COPY_CREATOR);
    meta(&mut html, "name", "twitter:url", &url);
    meta(&mut html, "property", "og:url", &url);
    if card.large_image {
        meta(&mut html, "name", "twitter:card", "summary_large_image");
    }
    html.push_str("</head><body></body></html>");
    html
}

/// `toCanonicalPosition` (`utils/position/toCanonicalPosition.ts`): the
/// string split on every run of `,.;|` (`split(/[,.;|]+/)`, so a leading or
/// trailing separator leaves an empty piece), exactly two pieces read as
/// `Number()` (an empty piece is 0), finite, inside the world limits, joined
/// with `glue`; `None` otherwise.
pub(super) fn canonical_position(position: &str, glue: &str) -> Option<String> {
    if position.is_empty() {
        return None;
    }
    let is_separator = |c: char| matches!(c, ',' | '.' | ';' | '|');
    let mut pieces: Vec<&str> = Vec::new();
    let mut start = 0;
    let mut in_run = false;
    for (i, c) in position.char_indices() {
        if is_separator(c) {
            if !in_run {
                pieces.push(&position[start..i]);
            }
            in_run = true;
        } else {
            if in_run {
                start = i;
            }
            in_run = false;
        }
    }
    pieces.push(if in_run { "" } else { &position[start..] });
    if pieces.len() != 2 {
        return None;
    }
    // `Number(piece)`: whitespace-trimmed, empty is 0; the pieces carry no
    // `.`, so an integral value is the only one a parcel can be.
    let number = |piece: &str| -> Option<i64> {
        let piece = piece.trim();
        if piece.is_empty() {
            return Some(0);
        }
        let n: f64 = piece.parse().ok()?;
        (n.is_finite() && n.fract() == 0.0 && n.abs() < 1e9).then_some(n as i64)
    };
    let (x, y) = (number(pieces[0])?, number(pieces[1])?);
    if !crate::shared::parcel::is_inside_world_limits(x, y) {
        return None;
    }
    Some(format!("{x}{glue}{y}"))
}

/// `validator/lib/isUUID` with its default `all` version: the hyphenated form, any case.
fn is_uuid(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// The configured base, else the default when it does not parse.
fn base_url(base: &str) -> url::Url {
    url::Url::parse(base.trim())
        .ok()
        .filter(|u| u.has_host())
        .unwrap_or_else(|| url::Url::parse(DEFAULT_BASE_URL).expect("the default base is a URL"))
}

/// `new URL(base)` with `pathname` set and one query pair, as `placeUrl`,
/// `worldUrl` and `whatsOnUrl` build theirs (`Place/utils.ts:23-48`).
pub(super) fn site_url(base: &str, pathname: &str, key: &str, value: &str) -> String {
    let mut u = base_url(base);
    u.set_path(pathname);
    u.set_fragment(None);
    u.query_pairs_mut().clear().append_pair(key, value);
    u.to_string()
}

/// `placeUrl` (`Place/utils.ts:23-30`): `/places/place/?position=x.y`.
fn place_url(base: &str, place: &PlaceRow) -> String {
    let position = canonical_position(&place.base_position, ".")
        .unwrap_or_else(|| place.base_position.clone());
    site_url(base, "/places/place/", "position", &position)
}

/// `worldUrl` (`Place/utils.ts:32-39`): `/places/world/?name=<world_name>`.
fn world_url(base: &str, name: &str) -> String {
    site_url(base, "/places/world/", "name", name)
}

/// The generic card's URL (`Social/routes.ts:66`): `siteUrl()` (the base
/// with pathname `/places/`) followed by the request's `originalUrl` less
/// its leading slash.
fn fallback_url(base: &str, original: &str) -> String {
    let mut u = base_url(base);
    u.set_path("/places/");
    u.set_query(None);
    u.set_fragment(None);
    format!("{}{}", u, original.strip_prefix('/').unwrap_or(original))
}

fn with_canonical(url: &str, html: String) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&format!("<{url}>; rel=canonical")) {
        headers.insert(header::LINK, v);
    }
    (headers, Html(html)).into_response()
}

/// The first place deployed at `position` (`Social/routes.ts:33-47`).
async fn place_at(state: &SharedState, position: &str) -> Option<PlaceRow> {
    let filters = super::PlaceListFilters {
        positions: vec![position.to_string()],
        limit: 1,
        only_places: true,
        ..Default::default()
    };
    state
        .places
        .list_page(&filters)
        .await
        .ok()
        .and_then(|(rows, _)| rows.into_iter().next())
}

fn card_for(state: &SharedState, row: &PlaceRow, url: &str) -> Response {
    let image =
        sanitize_image_url(row.image.as_deref(), state.places.content_origin()).unwrap_or_default();
    let card = Card::found(
        row.title.as_deref().unwrap_or(""),
        row.description.as_deref().unwrap_or(""),
        &image,
        url,
    );
    with_canonical(url, render(&card))
}

fn original_url(uri: &axum::http::Uri) -> &str {
    uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
}

pub async fn inject_place_metadata(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    // Any query shape reaches the fallback card, never a framework rejection.
    let first = |key: &str| q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    let id = first("id").unwrap_or_default();
    let position = first("position").unwrap_or_default();
    let base = state.settings.places_base_url.as_str();

    let place = if is_uuid(id) {
        state.places.find_by_id_for(id, None).await.ok().flatten()
    } else if let Some(canonical) = canonical_position(position, ",") {
        place_at(&state, &canonical).await
    } else {
        None
    };

    if let Some(place) = place {
        let url = place_url(base, &place);
        return card_for(&state, &place, &url);
    }
    let url = fallback_url(base, original_url(&uri));
    Html(render(&Card::generic(&url))).into_response()
}

pub async fn inject_world_metadata(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    Query(q): Query<Vec<(String, String)>>,
) -> Response {
    let first = |key: &str| q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    let world_id = first("id")
        .or_else(|| first("name"))
        .unwrap_or_default()
        .to_lowercase();
    let base = state.settings.places_base_url.as_str();

    let world = if !world_id.is_empty() {
        match state.places.find_world_by_id_for(&world_id, None).await {
            Ok(world) => world,
            Err(error) => {
                tracing::warn!(%error, world_id, "world lookup failed; serving the generic card");
                None
            }
        }
    } else {
        None
    };

    if let Some(world) = world {
        let name = world.world_name.clone().unwrap_or_default();
        let url = world_url(base, &name);
        return card_for(&state, &world, &url);
    }
    let url = fallback_url(base, original_url(&uri));
    Html(render(&Card::generic(&url))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(value: Option<&str>) -> Option<String> {
        sanitize_image_url(value, None)
    }

    const BREAKOUT: &str = "https://a\"><script>alert(1)</script><meta name=\"x";

    #[test]
    fn sanitize_image_url_keeps_valid_http_and_https_urls_normalized() {
        assert_eq!(
            img(Some("https://cdn.decentraland.org/thumb.png")),
            Some("https://cdn.decentraland.org/thumb.png".to_string())
        );
        assert_eq!(
            img(Some("http://cdn.decentraland.org/thumb.png")),
            Some("http://cdn.decentraland.org/thumb.png".to_string())
        );
        assert_eq!(
            img(Some("https://cdn.decentraland.org/t.png?w=100&h=50")),
            Some("https://cdn.decentraland.org/t.png?w=100&h=50".to_string())
        );
    }

    #[test]
    fn sanitize_image_url_rejects_html_breakout_values() {
        assert_eq!(img(Some(BREAKOUT)), None);
    }

    #[test]
    fn sanitize_image_url_rejects_non_http_schemes() {
        assert_eq!(img(Some("javascript:alert(document.domain)")), None);
        assert_eq!(img(Some("data:text/html,<script>alert(1)</script>")), None);
        assert_eq!(img(Some("file:///etc/passwd")), None);
    }

    #[test]
    fn sanitize_image_url_rejects_missing_or_unparseable_values() {
        assert_eq!(img(None), None);
        assert_eq!(img(Some("")), None);
        assert_eq!(img(Some("not a url")), None);
        assert_eq!(img(Some("/relative/path.png")), None);
    }

    #[test]
    fn sanitize_image_url_percent_encodes_breakout_chars_outside_the_host() {
        assert_eq!(
            img(Some("https://example.com/a\"b<c>d")),
            Some("https://example.com/a%22b%3Cc%3Ed".to_string())
        );
    }

    #[test]
    fn render_never_emits_a_sanitized_breakout_image_as_live_markup() {
        let image = img(Some(BREAKOUT)).unwrap_or_default();
        let html = render(&Card::found(
            "Genesis Plaza",
            "description",
            &image,
            "https://decentraland.org/places/place/?position=0.0",
        ));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains(BREAKOUT));
        assert!(html.contains("property=\"og:image\" content=\"\""));
    }

    #[test]
    fn render_escapes_image_and_url_attribute_values() {
        let html = render(&Card::found(
            "t",
            "d",
            "https://cdn.decentraland.org/t.png?w=100&h=50",
            "https://x\"><script>alert(1)</script>",
        ));
        assert!(html.contains("content=\"https://cdn.decentraland.org/t.png?w=100&amp;h=50\""));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(
            html.contains("content=\"https://x&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;\"")
        );
    }

    /// `utils/position/toCanonicalPosition.test.ts`, case for case, plus
    /// the world-limit edges `@dcl/schemas` draws.
    #[test]
    fn canonical_position_is_the_old_helpers() {
        assert_eq!(canonical_position("-9,-9", "."), Some("-9.-9".into()));
        assert_eq!(canonical_position("-9|-9", "."), Some("-9.-9".into()));
        assert_eq!(canonical_position("-9.-9", "."), Some("-9.-9".into()));
        assert_eq!(canonical_position("-9;-9", "."), Some("-9.-9".into()));
        assert_eq!(canonical_position("-9;-9", "|"), Some("-9|-9".into()));
        assert_eq!(canonical_position("20,-20", "."), Some("20.-20".into()));
        assert_eq!(canonical_position("0,0", ","), Some("0,0".into()));
        for bad in [
            "-9/-9", "", "0.1,2;3", "abc,de", "10000,1", "1", "1,2,3", "0,151", "-151,0",
            "164,100", "1e-1,2",
        ] {
            assert_eq!(canonical_position(bad, "."), None, "{bad:?}");
        }
        assert_eq!(canonical_position("163,100", "."), Some("163.100".into()));
        assert_eq!(
            canonical_position("151,151", "."),
            Some("151.151".into()),
            "the north-east block"
        );
        assert_eq!(
            canonical_position("150,,150", "."),
            Some("150.150".into()),
            "a run of separators is one"
        );
        // `Number("")` is 0: `split(/[,.;|]+/)` keeps the empty edge piece.
        assert_eq!(canonical_position(",1", "."), Some("0.1".into()));
        assert_eq!(canonical_position("1,", "."), Some("1.0".into()));
        assert_eq!(canonical_position(" 1e1 , 2", "."), Some("10.2".into()));
    }

    #[test]
    fn ids_are_looked_up_only_as_hyphenated_uuids() {
        assert!(is_uuid("3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d"));
        assert!(is_uuid("3CA25728-5B41-48F6-8E24-5CB0E3F2BB5D"));
        for bad in [
            "nope",
            "",
            "3ca257285b4148f68e245cb0e3f2bb5d",
            "3ca25728-5b41-48f6-8e24-5cb0e3f2bb5",
            "-7,7",
        ] {
            assert!(!is_uuid(bad), "{bad:?}");
        }
    }

    /// `Place/utils.ts:23-39`, `Social/routes.ts:66`: the canonical URLs
    /// of the captures `places.social_place.id` and `places.social_world.name`
    /// and the generic card's echo of the request.
    #[test]
    fn urls_follow_the_configured_base() {
        let base = "https://decentraland.org/places";
        assert_eq!(
            site_url(base, "/places/place/", "position", "-3.-2"),
            "https://decentraland.org/places/place/?position=-3.-2"
        );
        assert_eq!(
            world_url(base, "MetadyneLabs.dcl.eth"),
            "https://decentraland.org/places/world/?name=MetadyneLabs.dcl.eth"
        );
        assert_eq!(
            world_url(base, "my world\".dcl.eth"),
            "https://decentraland.org/places/world/?name=my+world%22.dcl.eth",
            "URLSearchParams form-encodes"
        );
        assert_eq!(
            fallback_url(base, "/places/place/"),
            "https://decentraland.org/places/places/place/"
        );
        assert_eq!(
            fallback_url(base, "/places/place/?id=nope"),
            "https://decentraland.org/places/places/place/?id=nope"
        );
        assert_eq!(
            fallback_url(
                "https://places.example.org/anything?x=1#f",
                "/places/world/"
            ),
            "https://places.example.org/places/places/world/",
            "the base's path, query and fragment are replaced as `new URL` + pathname does"
        );
        assert_eq!(
            site_url(
                "https://places.example.org:8443/x",
                "/whats-on",
                "world",
                "a.dcl.eth"
            ),
            "https://places.example.org:8443/whats-on?world=a.dcl.eth"
        );
        assert_eq!(
            site_url("not a url", "/places/place/", "position", "0.0"),
            "https://decentraland.org/places/place/?position=0.0",
            "an unparsable base falls back to the default"
        );
    }

    /// The capture `places.social_place.fallback` (2026-11-04), byte for byte.
    #[test]
    fn the_generic_card_is_the_capture() {
        let html = render(&Card::generic(
            "https://decentraland.org/places/places/place/",
        ));
        assert_eq!(
            html,
            "<!DOCTYPE html><html><head><title data-react-helmet=\"true\">Decentraland Place</title><meta name=\"twitter:title\" content=\"Decentraland Place\" data-react-helmet=\"true\"><meta property=\"og:title\" content=\"Decentraland Place\" data-react-helmet=\"true\"><meta name=\"description\" content=\"This place is awesome\" data-react-helmet=\"true\"><meta name=\"twitter:description\" content=\"This place is awesome\" data-react-helmet=\"true\"><meta property=\"og:description\" content=\"This place is awesome\" data-react-helmet=\"true\"><meta name=\"twitter:image\" content=\"image\" data-react-helmet=\"true\"><meta property=\"og:image\" content=\"image\" data-react-helmet=\"true\"><meta name=\"site\" content=\"site\" data-react-helmet=\"true\"><meta name=\"card\" content=\"card\" data-react-helmet=\"true\"><meta name=\"creator\" content=\"creator\" data-react-helmet=\"true\"><meta name=\"twitter:url\" content=\"https://decentraland.org/places/places/place/\" data-react-helmet=\"true\"><meta property=\"og:url\" content=\"https://decentraland.org/places/places/place/\" data-react-helmet=\"true\"></head><body></body></html>"
        );
    }

    /// The capture `places.social_place.id` (Genesis Plaza, 2026-11-04),
    /// byte for byte from the row's title, description and image: the
    /// apostrophe reads `&amp;#39;` because the route escaped before the
    /// renderer did.
    #[test]
    fn a_found_card_is_the_capture() {
        let html = render(&Card::found(
            "Genesis Plaza",
            "Decentraland's official spawn point and your #1 place to meet new people, hang out, and stay up to date with what's happening in our virtual world.",
            "https://peer-ec1.decentraland.org/content/contents/bafybeihjx4ugiye3ruqfcv54tu5p5keuy3x2l2u2o5otfczlb6dfp734bq",
            "https://decentraland.org/places/place/?position=-3.-2",
        ));
        assert_eq!(
            html,
            "<!DOCTYPE html><html><head><title data-react-helmet=\"true\">Genesis Plaza | Decentraland Place</title><meta name=\"twitter:title\" content=\"Genesis Plaza | Decentraland Place\" data-react-helmet=\"true\"><meta property=\"og:title\" content=\"Genesis Plaza | Decentraland Place\" data-react-helmet=\"true\"><meta name=\"description\" content=\"Decentraland&amp;#39;s official spawn point and your #1 place to meet new people, hang out, and stay up to date with what&amp;#39;s happening in our virtual world.\" data-react-helmet=\"true\"><meta name=\"twitter:description\" content=\"Decentraland&amp;#39;s official spawn point and your #1 place to meet new people, hang out, and stay up to date with what&amp;#39;s happening in our virtual world.\" data-react-helmet=\"true\"><meta property=\"og:description\" content=\"Decentraland&amp;#39;s official spawn point and your #1 place to meet new people, hang out, and stay up to date with what&amp;#39;s happening in our virtual world.\" data-react-helmet=\"true\"><meta name=\"twitter:image\" content=\"https://peer-ec1.decentraland.org/content/contents/bafybeihjx4ugiye3ruqfcv54tu5p5keuy3x2l2u2o5otfczlb6dfp734bq\" data-react-helmet=\"true\"><meta property=\"og:image\" content=\"https://peer-ec1.decentraland.org/content/contents/bafybeihjx4ugiye3ruqfcv54tu5p5keuy3x2l2u2o5otfczlb6dfp734bq\" data-react-helmet=\"true\"><meta name=\"site\" content=\"site\" data-react-helmet=\"true\"><meta name=\"card\" content=\"card\" data-react-helmet=\"true\"><meta name=\"creator\" content=\"creator\" data-react-helmet=\"true\"><meta name=\"twitter:url\" content=\"https://decentraland.org/places/place/?position=-3.-2\" data-react-helmet=\"true\"><meta property=\"og:url\" content=\"https://decentraland.org/places/place/?position=-3.-2\" data-react-helmet=\"true\"><meta name=\"twitter:card\" content=\"summary_large_image\" data-react-helmet=\"true\"></head><body></body></html>"
        );
    }

    #[test]
    fn the_description_is_cut_at_its_first_blank_line_and_newlines_collapse() {
        let html = render(&Card::found(
            "t",
            "  line one\nline two\n\n\nsecond paragraph  ",
            "",
            "https://decentraland.org/places/place/",
        ));
        assert!(html.contains("property=\"og:description\" content=\"line one line two\""));
        assert!(html.contains("name=\"twitter:description\" content=\"line one line two\""));
        assert!(html.contains("name=\"description\" content=\"line one line two\""));
        assert!(!html.contains("second paragraph"));
        let html = render(&Card::found("A <b>&amp;</b>", "x", "", "u"));
        assert!(
            html.contains("<title data-react-helmet=\"true\">A &amp;lt;b&amp;gt;&amp;amp;amp;&amp;lt;/b&amp;gt; | Decentraland Place</title>"),
            "{html}"
        );
    }

    /// The cards over the catalogue: by id, by the parcel it is deployed at,
    /// a world by name, and the generic card for an unknown or malformed one.
    #[tokio::test]
    async fn the_cards_describe_the_catalogue_row() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let place = uuid::Uuid::new_v4().to_string();
        let world = format!(
            "card{}.dcl.eth",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let cased = format!("Card{}", &world[4..]);
        let (x, y) = (rand::random::<u16>() % 150, rand::random::<u16>() % 149 + 1);
        let position = format!("{x},-{y}");
        sqlx::query("INSERT INTO explore.place (id, base_position, title, description, raw) VALUES ($1, $2, 'Plaza <b>', 'First\n\nsecond', $3)")
            .bind(&place)
            .bind(&position)
            .bind(serde_json::json!({ "positions": [position], "image": "https://cdn.example.org/p.png" }))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '0,0', 'World', $2)")
            .bind(&world)
            .bind(serde_json::json!({ "world": true, "world_name": cased }))
            .execute(&pool)
            .await
            .unwrap();
        let app = crate::router(std::sync::Arc::new(crate::ExploreState::new(
            pool,
            crate::Settings {
                places_base_url: "https://places.example.org/ignored".into(),
                ..crate::Settings::for_tests()
            },
            None,
        )));
        let get = |uri: String| {
            let app = app.clone();
            async move {
                let resp = app
                    .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let link = resp
                    .headers()
                    .get("link")
                    .map(|v| v.to_str().unwrap().to_string());
                let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
                    .await
                    .unwrap();
                (link, String::from_utf8(bytes.to_vec()).unwrap())
            }
        };
        let canonical = format!("https://places.example.org/places/place/?position={x}.-{y}");
        for uri in [
            format!("/places/place?id={place}"),
            format!("/places/place?position={position}"),
            format!("/places/place?id=nope&position={x}|-{y}"),
            format!("/places/place?position={x};-{y}"),
        ] {
            let (link, html) = get(uri.clone()).await;
            assert!(
                html.contains(
                    "<title data-react-helmet=\"true\">Plaza &amp;lt;b&amp;gt; | Decentraland Place</title>"
                ),
                "{uri}: {html}"
            );
            assert!(
                html.contains("name=\"description\" content=\"First\" data-react-helmet=\"true\""),
                "the first paragraph only: {html}"
            );
            assert!(html.contains("content=\"https://cdn.example.org/p.png\""));
            assert!(
                html.contains(&format!("property=\"og:url\" content=\"{canonical}\"")),
                "{uri}: {html}"
            );
            assert_eq!(
                link.unwrap(),
                format!("<{canonical}>; rel=canonical"),
                "{uri}"
            );
        }
        let (link, html) = get(format!("/places/world?name={}", world.to_uppercase())).await;
        assert!(html.contains("World | Decentraland Place"), "{html}");
        assert_eq!(
            link.unwrap(),
            format!("<https://places.example.org/places/world/?name={cased}>; rel=canonical"),
            "the cased name, as upstream's worldUrl"
        );
        for (uri, echoed) in [
            ("/places/place?id=nowhere", "/places/place?id=nowhere"),
            (
                &format!("/places/place?id={}", uuid::Uuid::new_v4()) as &str,
                "",
            ),
            (
                "/places/place?position=9999,1",
                "/places/place?position=9999,1",
            ),
            (
                "/places/world?name=no-such.dcl.eth",
                "/places/world?name=no-such.dcl.eth",
            ),
            ("/places/world", "/places/world"),
        ] {
            let (link, html) = get(uri.to_string()).await;
            assert!(link.is_none(), "{uri}");
            assert!(
                html.contains("<title data-react-helmet=\"true\">Decentraland Place</title>"),
                "{uri}: {html}"
            );
            assert!(html.contains("content=\"This place is awesome\""));
            if !echoed.is_empty() {
                assert!(
                    html.contains(&format!(
                        "property=\"og:url\" content=\"https://places.example.org/places{echoed}\""
                    )),
                    "{uri}: {html}"
                );
            }
            assert!(!html.contains("twitter:card"), "{uri}");
        }
    }
}

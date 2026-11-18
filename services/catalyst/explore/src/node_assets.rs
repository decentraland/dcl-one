//! Node paths for assets referenced by the upstream catalogues.

use url::{Position, Url};

use crate::Settings;

use crate::api::posters::{HOST_PREFIX, valid_file};

const POSTER_BUCKET: &str = "events-assets-099ac00.decentraland.org";
pub const DEFAULT_EVENT_IMAGE: &str = "/assets/event-default.webp";

fn poster_path(path: &str) -> Option<(&str, &str)> {
    let (kind, file) = path.strip_prefix('/')?.split_once('/')?;
    (matches!(kind, "poster" | "poster-vertical") && valid_file(file)).then_some((kind, file))
}

pub(crate) fn mounted_poster_url(path: &str, domain: Option<&str>) -> String {
    match domain {
        Some(domain) => format!("https://{domain}/{HOST_PREFIX}{path}"),
        None => format!("/{HOST_PREFIX}{path}"),
    }
}

/// Projects upload keys and copied posters onto the same mounted routes.
pub(crate) fn event_image_url(
    image: &str,
    domain: Option<&str>,
    events_asset_base: &str,
) -> String {
    if poster_path(image).is_some() {
        return mounted_poster_url(image, domain);
    }
    to_node(&absolute_default_image(image, events_asset_base), domain)
}

/// Expand the bundled default for consumers that render a stored event.
pub fn absolute_default_image(image: &str, events_asset_base: &str) -> String {
    if image == DEFAULT_EVENT_IMAGE {
        format!("{}{image}", events_asset_base.trim_end_matches('/'))
    } else {
        image.to_owned()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PosterInput<'a> {
    Default,
    Local(&'a str),
    Upstream(&'a str),
    Mounted(&'a str),
}

pub(crate) fn upstream_poster_url(path: &str) -> String {
    format!("https://{POSTER_BUCKET}{path}")
}

/// Parses authoring inputs using the same poster keys and mount as the read projection.
/// A mounted key needs the stored value or the poster index to identify its origin.
pub(crate) fn event_poster_input<'a>(
    image: &'a str,
    settings: &Settings,
    expected_kind: &str,
) -> Option<PosterInput<'a>> {
    let image = image.trim();
    let valid_kind = |path| poster_path(path).is_some_and(|(kind, _)| kind == expected_kind);
    if image == DEFAULT_EVENT_IMAGE {
        return Some(PosterInput::Default);
    }
    if let Some(path) = image
        .strip_prefix('/')
        .and_then(|path| path.strip_prefix(HOST_PREFIX))
    {
        return valid_kind(path).then_some(PosterInput::Mounted(path));
    }
    if valid_kind(image) {
        return Some(PosterInput::Local(image));
    }
    if let Some((url, path)) = parse_upstream_url(image) {
        if url.host_str() == Some(POSTER_BUCKET) {
            return valid_kind(path).then_some(PosterInput::Upstream(path));
        }
        if let Some(path) = path
            .strip_prefix('/')
            .and_then(|path| path.strip_prefix(HOST_PREFIX))
        {
            return (url.host_str() == settings.asset_rewrite_domain.as_deref()
                && valid_kind(path))
            .then_some(PosterInput::Mounted(path));
        }
    }
    let (url, path) = legacy_poster_url(image)?;
    for base in [
        &settings.events_asset_base_url,
        &settings.content_public_url,
    ] {
        let Ok(base) = Url::parse(base) else { continue };
        if base.origin() != url.origin() {
            continue;
        }
        let prefix = base.path().trim_end_matches('/');
        if let Some(path) = path.strip_prefix(prefix) {
            if path == DEFAULT_EVENT_IMAGE {
                return Some(PosterInput::Default);
            }
            if valid_kind(path) {
                return Some(PosterInput::Local(path));
            }
        }
        if path == DEFAULT_EVENT_IMAGE {
            return Some(PosterInput::Default);
        }
        if valid_kind(path) {
            return Some(PosterInput::Local(path));
        }
    }
    None
}

fn legacy_poster_url(image: &str) -> Option<(Url, &str)> {
    let (_, rest) = image.split_once("://")?;
    let (authority, path) = rest.split_at(rest.find('/')?);
    let url = Url::parse(image).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || image.bytes().any(|byte| byte.is_ascii_control())
        || !authority.eq_ignore_ascii_case(&url[Position::BeforeHost..Position::AfterPort])
        || path != url.path()
        || (url.port().is_some()
            && !matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    {
        return None;
    }
    Some((url, path))
}

pub(crate) fn is_uploaded_poster_key(file: &str) -> bool {
    file.split_once('.').is_some_and(|(hash, _)| {
        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// Maps a recognised upstream asset to its path on the node's public domain.
pub fn node_asset_url(url: &str, domain: &str) -> Option<String> {
    let (parsed, path_and_query) = parse_upstream_url(url)?;
    let host = parsed.host_str()?;
    let path = parsed.path();

    if host == "api.decentraland.org" && matches!(path, "/v1/map.png" | "/v2/map.png") {
        return Some(format!("https://{domain}/api{path_and_query}"));
    }
    if parsed.query().is_some() {
        return None;
    }

    match host {
        POSTER_BUCKET if poster_path(path).is_some() => {
            Some(mounted_poster_url(path, Some(domain)))
        }
        host if is_peer_host(host)
            && path
                .strip_prefix("/content/contents/")
                .is_some_and(is_asset_segment) =>
        {
            Some(format!("https://{domain}{path}"))
        }
        "worlds-content-server.decentraland.org"
            if path
                .strip_prefix("/contents/")
                .is_some_and(is_asset_segment) =>
        {
            Some(format!("https://{domain}/worlds-content-server{path}"))
        }
        _ => None,
    }
}

/// Leaves the stored URL intact when there is no domain or the asset is unrecognised.
pub fn to_node(url: &str, domain: Option<&str>) -> String {
    domain
        .and_then(|domain| node_asset_url(url, domain))
        .unwrap_or_else(|| url.to_owned())
}

/// Maps a production jump link to the node while retaining its query spelling.
pub fn node_page_url(url: &str, domain: &str) -> Option<String> {
    let (parsed, path_and_query) = parse_upstream_url(url)?;
    if parsed.host_str()? != "decentraland.org"
        || !matches!(parsed.path(), "/jump" | "/jump/events")
        || parsed.query().is_none()
    {
        return None;
    }

    Some(format!("https://{domain}{path_and_query}"))
}

fn parse_upstream_url(url: &str) -> Option<(Url, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") || url.bytes().any(|byte| byte.is_ascii_control()) {
        return None;
    }

    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let (authority, path_and_query) = rest.split_at(rest.find('/')?);
    let path = path_and_query
        .split_once('?')
        .map_or(path_and_query, |(path, _)| path);

    // Check the raw authority and path because Url normalizes default ports and dot segments.
    if !authority.eq_ignore_ascii_case(host) || path != parsed.path() || parsed.fragment().is_some()
    {
        return None;
    }

    Some((parsed, path_and_query))
}

fn is_peer_host(host: &str) -> bool {
    if host == "peer.decentraland.org" {
        return true;
    }

    let Some(region) = host
        .strip_prefix("peer-")
        .and_then(|host| host.strip_suffix(".decentraland.org"))
    else {
        return false;
    };
    let Some((digit, letters)) = region.as_bytes().split_last() else {
        return false;
    };
    digit.is_ascii_digit() && !letters.is_empty() && letters.iter().all(u8::is_ascii_alphabetic)
}

fn is_asset_segment(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::{node_asset_url, node_page_url, to_node};

    const D: &str = "node.example";

    #[test]
    fn poster_projection_uses_inputs_the_upload_policy_recognizes() {
        use super::{DEFAULT_EVENT_IMAGE, PosterInput, event_image_url, event_poster_input};
        use crate::Settings;

        for domain in [None, Some(D.to_owned())] {
            let settings = Settings {
                asset_rewrite_domain: domain.clone(),
                events_asset_base_url: format!("https://{D}/events"),
                ..Settings::for_tests()
            };
            for kind in ["poster", "poster-vertical"] {
                let names = [
                    "a.png".to_owned(),
                    format!("{}.webp", "a".repeat(64)),
                    "Z-1_a.B.webp".to_owned(),
                ];
                for name in names {
                    let path = format!("/{kind}/{name}");
                    let upstream = super::upstream_poster_url(&path);
                    let mounted = super::mounted_poster_url(&path, domain.as_deref());
                    let project = |image: &str| {
                        event_image_url(image, domain.as_deref(), &settings.events_asset_base_url)
                    };
                    assert_eq!(project(&path), mounted);
                    let (copied_url, copied_input) = if domain.is_some() {
                        (&mounted, PosterInput::Mounted(path.as_str()))
                    } else {
                        (&upstream, PosterInput::Upstream(path.as_str()))
                    };
                    assert_eq!(&project(&upstream), copied_url);
                    assert_eq!(
                        event_poster_input(copied_url, &settings, kind),
                        Some(copied_input)
                    );
                    assert_eq!(
                        event_poster_input(&mounted, &settings, kind),
                        Some(PosterInput::Mounted(&path))
                    );
                }
                let default = event_image_url(
                    DEFAULT_EVENT_IMAGE,
                    domain.as_deref(),
                    &settings.events_asset_base_url,
                );
                assert_eq!(
                    event_poster_input(&default, &settings, kind),
                    Some(PosterInput::Default)
                );
            }
        }
    }

    #[test]
    fn a_third_party_image_is_preserved_on_read_and_refused_by_upload_policy() {
        let settings = crate::Settings {
            asset_rewrite_domain: Some(D.into()),
            ..crate::Settings::for_tests()
        };
        let image = "https://images.example.org/cover.png";
        let projected = super::event_image_url(image, Some(D), &settings.events_asset_base_url);
        assert_eq!(projected, image);
        assert!(super::event_poster_input(image, &settings, "poster").is_none());
    }

    #[test]
    fn poster_inputs_refuse_misleading_origins_and_invalid_paths() {
        use super::event_poster_input;
        use crate::Settings;

        let settings = Settings {
            asset_rewrite_domain: Some(D.to_owned()),
            events_asset_base_url: format!("https://{D}/events"),
            ..Settings::for_tests()
        };
        for input in [
            "//node.example/events-assets-099ac00/poster/a.png",
            "https://another.example/events-assets-099ac00/poster/a.png",
            "https://node.example.evil.test/events-assets-099ac00/poster/a.png",
            "https://node.example@evil.test/events-assets-099ac00/poster/a.png",
            "https://user@node.example/events-assets-099ac00/poster/a.png",
            "https://node.example:443/events-assets-099ac00/poster/a.png",
            "https://node.example:8443/events-assets-099ac00/poster/a.png",
            "https://node.example/unused/../events-assets-099ac00/poster/a.png",
            "https://node.example/events-assets-099ac00/poster/%2e%2e/a.png",
            "https://node.example/events-assets-099ac00/poster/%61.png",
            "https://node.example/events-assets-099ac00/poster/a.png?x=1",
            "https://node.example/events-assets-099ac00/poster/a.png#x",
            "/events-assets-099ac00/poster-vertical/a.png",
            "/events-assets-099ac00/poster/_a.png",
            "/events-assets-099ac00/poster/.a.png",
            "/events-assets-099ac00/poster/../a.png",
            "https://events-assets-099ac00.decentraland.org.evil.test/poster/a.png",
            "https://node.example:443/events/poster/a.png",
            "https://node.example/events/../poster/a.png",
        ] {
            assert!(
                event_poster_input(input, &settings, "poster").is_none(),
                "{input}"
            );
        }
    }

    #[test]
    fn maps_each_known_upstream_to_its_node_path() {
        let longest_poster = format!("{}.webp", "a".repeat(195));
        let longest_upstream =
            format!("https://events-assets-099ac00.decentraland.org/poster/{longest_poster}");
        let longest_node =
            format!("https://node.example/events-assets-099ac00/poster/{longest_poster}");
        for (upstream, node) in [
            (longest_upstream.as_str(), longest_node.as_str()),
            (
                "https://events-assets-099ac00.decentraland.org/poster/abc.webp",
                "https://node.example/events-assets-099ac00/poster/abc.webp",
            ),
            (
                "https://events-assets-099ac00.decentraland.org/poster-vertical/v.webp",
                "https://node.example/events-assets-099ac00/poster-vertical/v.webp",
            ),
            (
                "https://peer.decentraland.org/content/contents/bafybeihjx4",
                "https://node.example/content/contents/bafybeihjx4",
            ),
            (
                "https://peer-ec1.decentraland.org/content/contents/bafybeihjx4",
                "https://node.example/content/contents/bafybeihjx4",
            ),
            (
                "https://worlds-content-server.decentraland.org/contents/1b79f38a",
                "https://node.example/worlds-content-server/contents/1b79f38a",
            ),
            (
                "https://api.decentraland.org/v1/map.png",
                "https://node.example/api/v1/map.png",
            ),
            (
                "https://api.decentraland.org/v2/map.png?center=1%2c1&name=a+b&value=%27",
                "https://node.example/api/v2/map.png?center=1%2c1&name=a+b&value=%27",
            ),
            (
                "https://api.decentraland.org/v2/map.png?center=-9,-9&size=10",
                "https://node.example/api/v2/map.png?center=-9,-9&size=10",
            ),
        ] {
            assert_eq!(
                node_asset_url(upstream, D).as_deref(),
                Some(node),
                "{upstream}"
            );
        }
    }

    #[test]
    fn leaves_everything_else_untouched() {
        let long_poster = "a".repeat(196);
        let long_url = format!(
            "https://events-assets-099ac00.decentraland.org/poster-vertical/{long_poster}.webp"
        );
        for untouched in [
            "https://events-assets-099ac00.decentraland.org/poster/_hidden.webp",
            "https://events-assets-099ac00.decentraland.org/poster-vertical/.hidden.webp",
            long_url.as_str(),
            "http://peer.decentraland.org/content/contents/x",
            "https://peer.decentraland.org.evil.example/content/contents/x",
            "https://events-assets-099ac00.decentraland.org.evil.example/poster/a.webp",
            "https://events-assets-other.decentraland.org/poster/a.webp",
            "https://peer.decentraland.org/content/contents/x?sig=1",
            "https://peer.decentraland.org/content/contents/x#fragment",
            "https://peer.decentraland.org/content/contents/../x",
            "https://peer.decentraland.org/content/contents/a/../x",
            "https://peer.decentraland.org/content/contents/%2e/x",
            "https://peer.decentraland.org/content/contents/%78",
            "https://peer-1.decentraland.org/content/contents/x",
            "https://peer-eu12.decentraland.org/content/contents/x",
            "https://peer.decentraland.org:443/content/contents/x",
            "https://@peer.decentraland.org/content/contents/x",
            "https://worlds-content-server.decentraland.org/contents/a/b",
            "https://events-assets-099ac00.decentraland.org/poster/a.webp?sig=1",
            "https://peer.decentraland.org/content/contents/a/b",
            "https://peer.decentraland.org/content/contents/..",
            "https://peer.decentraland.org/lambdas/profiles",
            "https://worlds-content-server.decentraland.org/world/x/about",
            "https://events-assets-099ac00.decentraland.org/private/a.webp",
            "https://user:pw@peer.decentraland.org/content/contents/x",
            "https://peer.decentraland.org:8443/content/contents/x",
            "https://api.decentraland.org/v1/parcels/1/2/map.png",
            "https://cdn.example/poster/a.webp",
            "/poster/a.webp",
            "",
        ] {
            assert_eq!(node_asset_url(untouched, D), None, "{untouched}");
            assert_eq!(to_node(untouched, Some(D)), untouched);
        }
    }

    #[test]
    fn node_page_url_maps_only_upstream_jump_links_with_queries() {
        for (upstream, node) in [
            (
                "https://decentraland.org/jump/events?position=0%2C0&realm=x.dcl.eth",
                "https://node.example/jump/events?position=0%2C0&realm=x.dcl.eth",
            ),
            (
                "https://decentraland.org/jump?position=1%2c1&name=a+b",
                "https://node.example/jump?position=1%2c1&name=a+b",
            ),
        ] {
            assert_eq!(node_page_url(upstream, D).as_deref(), Some(node));
        }

        for url in [
            "https://decentraland.org/events/event/?id=1",
            "https://play.decentraland.org/?position=1,1",
            "https://decentraland.org.evil.example/jump/events?position=1,1",
            "http://decentraland.org/jump/events?position=1,1",
            "https://example.com/party",
            "https://decentraland.org/jump/events",
            "https://decentraland.org/jump/events?position=1,1#scroll",
            "https://decentraland.org:443/jump?position=1,1",
            "https://decentraland.org/jump/other/../events?position=1,1",
        ] {
            assert_eq!(node_page_url(url, D), None, "{url}");
        }
    }

    #[test]
    fn no_domain_keeps_the_upstream_url() {
        let u = "https://peer.decentraland.org/content/contents/x";
        assert_eq!(to_node(u, None), u);
    }
}

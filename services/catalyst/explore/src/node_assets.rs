//! Node paths for assets referenced by the upstream catalogues.

use url::{Position, Url};

use crate::Settings;

use crate::api::posters::{HOST_PREFIX, valid_file};

const POSTER_BUCKET: &str = "events-assets-099ac00.decentraland.org";
pub const DEFAULT_EVENT_IMAGE: &str = "/assets/event-default.webp";

pub(crate) fn poster_path(path: &str) -> Option<(&str, &str)> {
    let (kind, file) = path.strip_prefix('/')?.split_once('/')?;
    (matches!(kind, "poster" | "poster-vertical") && valid_file(file)).then_some((kind, file))
}

pub(crate) fn mounted_poster_url(path: &str, base: Option<&str>) -> String {
    format!("{}/{HOST_PREFIX}{path}", base.unwrap_or_default())
}

/// Projects upload keys and copied posters onto the same mounted routes.
pub(crate) fn event_image_url(
    image: &str,
    rewrite_base: Option<&str>,
    configured_public_base: Option<&str>,
    events_asset_base: &str,
) -> String {
    if poster_path(image).is_some() {
        return mounted_poster_url(image, configured_public_base);
    }
    to_node(
        &absolute_default_image(image, events_asset_base),
        rewrite_base,
    )
}

/// Expand the bundled default for consumers that render a stored event.
fn absolute_default_image(image: &str, events_asset_base: &str) -> String {
    if image == DEFAULT_EVENT_IMAGE {
        format!("{}{image}", events_asset_base.trim_end_matches('/'))
    } else {
        image.to_owned()
    }
}

/// A notice's image: posters as the read projection serves them, other node paths on the asset mount.
pub fn notice_image_url(
    image: &str,
    configured_public_base: Option<&str>,
    events_asset_base: &str,
) -> String {
    let assets = events_asset_base.trim_end_matches('/');
    if poster_path(image).is_some() {
        return mounted_poster_url(image, configured_public_base);
    }
    if image.starts_with('/') {
        format!("{assets}{image}")
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

fn mounted_path(path: &str) -> Option<&str> {
    path.strip_prefix('/')?.strip_prefix(HOST_PREFIX)
}

/// Parses authoring inputs using the same poster keys and mount as the read projection.
pub(crate) fn event_poster_input<'a>(
    image: &'a str,
    settings: &Settings,
    expected_kind: &str,
) -> Option<PosterInput<'a>> {
    let image = image.trim();
    let valid_kind = |path| poster_path(path).is_some_and(|(kind, _)| kind == expected_kind);
    let legacy = |path| {
        if path == DEFAULT_EVENT_IMAGE {
            Some(PosterInput::Default)
        } else {
            valid_kind(path).then_some(PosterInput::Local(path))
        }
    };
    if let Some(path) = mounted_path(image) {
        return valid_kind(path).then_some(PosterInput::Mounted(path));
    }
    if let Some(input) = legacy(image) {
        return Some(input);
    }
    let base = settings
        .configured_public_base
        .as_deref()
        .and_then(|base| Url::parse(base).ok());
    if let Some(base) = &base
        && let Some(path) = image.strip_prefix(base.path().trim_end_matches('/'))
        && let Some(path) = mounted_path(path)
    {
        return valid_kind(path).then_some(PosterInput::Mounted(path));
    }

    let (url, path) = parse_asset_url(image)?;
    if url.query().is_some() {
        return None;
    }
    if url.scheme() == "https" && url.port().is_none() && url.host_str() == Some(POSTER_BUCKET) {
        return valid_kind(path).then_some(PosterInput::Upstream(path));
    }
    if let Some(base) = base
        && base.origin() == url.origin()
        && let Some(path) = path.strip_prefix(base.path().trim_end_matches('/'))
        && let Some(path) = mounted_path(path)
    {
        return valid_kind(path).then_some(PosterInput::Mounted(path));
    }
    [
        (&settings.events_asset_base_url, "/events"),
        (&settings.content_public_url, "/content"),
    ]
    .into_iter()
    .filter_map(|(base, mount)| Some((Url::parse(base).ok()?, mount)))
    .filter(|(base, _)| base.origin() == url.origin())
    .find_map(|(base, mount)| {
        let prefix = base.path().trim_end_matches('/');
        if settings.asset_rewrite_base.is_none() {
            let public_prefix = prefix.strip_suffix(mount).unwrap_or(prefix);
            let copy = mounted_path(path)
                .or_else(|| path.strip_prefix(public_prefix).and_then(mounted_path));
            if let Some(path) = copy {
                return valid_kind(path).then_some(PosterInput::Mounted(path));
            }
        }
        path.strip_prefix(prefix)
            .and_then(legacy)
            .or_else(|| legacy(path))
    })
}

/// Maps a recognised upstream asset to its path on the node's public domain.
pub fn node_asset_url(url: &str, base: &str) -> Option<String> {
    let (parsed, _) = parse_upstream_url(url)?;
    let host = parsed.host_str()?;
    let path = parsed.path();

    if host == "api.decentraland.org" && matches!(path, "/v1/map.png" | "/v2/map.png") {
        return Some(format!(
            "{base}/api{}",
            &parsed[Position::BeforePath..Position::AfterQuery]
        ));
    }
    if parsed.query().is_some() {
        return None;
    }

    match host {
        POSTER_BUCKET if poster_path(path).is_some() => Some(mounted_poster_url(path, Some(base))),
        host if is_peer_host(host)
            && path
                .strip_prefix("/content/contents/")
                .is_some_and(is_asset_segment) =>
        {
            Some(format!("{base}{path}"))
        }
        "worlds-content-server.decentraland.org"
            if path
                .strip_prefix("/contents/")
                .is_some_and(is_asset_segment) =>
        {
            Some(format!("{base}/worlds-content-server{path}"))
        }
        _ => None,
    }
}

/// Leaves the stored URL intact when there is no domain or the asset is unrecognised.
pub fn to_node(url: &str, base: Option<&str>) -> String {
    base.and_then(|base| node_asset_url(url, base))
        .unwrap_or_else(|| url.to_owned())
}

/// Maps a production jump link to the node while retaining its query spelling.
pub fn node_page_url(url: &str, base: &str) -> Option<String> {
    let (parsed, path_and_query) = parse_upstream_url(url)?;
    if parsed.host_str()? != "decentraland.org"
        || !matches!(parsed.path(), "/jump" | "/jump/events")
        || parsed.query().is_none()
    {
        return None;
    }

    Some(format!("{base}{path_and_query}"))
}

fn parse_upstream_url(url: &str) -> Option<(Url, &str)> {
    let (parsed, path_and_query) = parse_asset_url(url)?;
    (parsed.scheme() == "https" && parsed.port().is_none()).then_some((parsed, path_and_query))
}

fn parse_asset_url(url: &str) -> Option<(Url, &str)> {
    let (_, rest) = url.split_once("://")?;
    let (authority, path_and_query) = rest.split_at(rest.find('/')?);
    let parsed = Url::parse(url).ok()?;
    let path = path_and_query
        .split_once('?')
        .map_or(path_and_query, |(path, _)| path);

    // Url can normalize explicit default ports and dot segments.
    if !matches!(parsed.scheme(), "http" | "https")
        || url.bytes().any(|byte| byte.is_ascii_control())
        || !authority.eq_ignore_ascii_case(&parsed[Position::BeforeHost..Position::AfterPort])
        || path != parsed.path()
        || parsed.fragment().is_some()
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

    const D: &str = "https://node.example";

    #[test]
    fn poster_projection_uses_inputs_the_upload_policy_recognizes() {
        use super::{DEFAULT_EVENT_IMAGE, PosterInput, event_image_url, event_poster_input};
        use crate::Settings;

        for domain in [None, Some(D.to_owned())] {
            let settings = Settings {
                asset_rewrite_base: domain.clone(),
                configured_public_base: domain.clone(),
                events_asset_base_url: format!("{D}/events"),
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
                    let upstream = format!("https://{}{}", super::POSTER_BUCKET, path);
                    let mounted = super::mounted_poster_url(&path, domain.as_deref());
                    let project = |image: &str| {
                        event_image_url(
                            image,
                            domain.as_deref(),
                            domain.as_deref(),
                            &settings.events_asset_base_url,
                        )
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
    fn mounted_inputs_require_the_configured_origin_and_path() {
        use super::{PosterInput, event_poster_input, mounted_poster_url};
        let base = "https://node.example:8443/prefix";
        let settings = crate::Settings {
            asset_rewrite_base: Some(base.into()),
            configured_public_base: Some(base.into()),
            events_asset_base_url: format!("{base}/events"),
            ..crate::Settings::for_tests()
        };
        let key = "/poster/example.webp";
        let mounted = mounted_poster_url(key, Some(base));
        assert_eq!(
            event_poster_input(&mounted, &settings, "poster"),
            Some(PosterInput::Mounted(key))
        );
        assert_eq!(
            event_poster_input(
                "/prefix/events-assets-099ac00/poster/example.webp",
                &settings,
                "poster"
            ),
            Some(PosterInput::Mounted(key))
        );
        assert_eq!(
            event_poster_input(&format!("{base}/events{key}"), &settings, "poster"),
            Some(PosterInput::Local(key))
        );
        for invalid in [
            mounted.replace(":8443", ":8444"),
            mounted.replace(":8443", ""),
            mounted.replace("/prefix", "/prefix-sibling"),
            mounted.replace("/prefix", ""),
        ] {
            assert_eq!(
                event_poster_input(&invalid, &settings, "poster"),
                None,
                "{invalid}"
            );
        }
    }

    #[test]
    fn absolute_upload_links_work_on_configured_origins_without_rewriting() {
        use super::{PosterInput, event_poster_input};
        let settings = crate::Settings {
            events_asset_base_url: "http://127.0.0.1:8080/nested/events".into(),
            content_public_url: "https://content.example/nested/content/".into(),
            ..crate::Settings::for_tests()
        };
        let path = "/poster/own_file.png";
        for url in [
            "http://127.0.0.1:8080/events-assets-099ac00/poster/own_file.png",
            "http://127.0.0.1:8080/nested/events-assets-099ac00/poster/own_file.png",
            "https://content.example/nested/events-assets-099ac00/poster/own_file.png",
        ] {
            assert_eq!(
                event_poster_input(url, &settings, "poster"),
                Some(PosterInput::Mounted(path)),
                "{url}"
            );
        }
        for url in [
            "http://127.0.0.1:8081/events-assets-099ac00/poster/own_file.png",
            "https://content.example.other/nested/events-assets-099ac00/poster/own_file.png",
            "https://content.example/sibling/events-assets-099ac00/poster/own_file.png",
        ] {
            assert_eq!(event_poster_input(url, &settings, "poster"), None, "{url}");
        }
    }

    #[test]
    fn notice_posters_match_the_read_projection_under_a_prefixed_public_url() {
        let public_url = "https://node.example.org/prefix/";
        let rewrite = crate::Settings::asset_rewrite_base_for(Some(public_url));
        let public = crate::Settings::configured_public_base_for(Some(public_url));
        let assets = crate::Settings::events_asset_base_for(public_url);
        for image in ["/poster/a.webp", "/poster-vertical/a.webp"] {
            assert_eq!(
                super::notice_image_url(image, public.as_deref(), &assets),
                super::event_image_url(image, rewrite.as_deref(), public.as_deref(), &assets),
                "{image}"
            );
        }
        assert_eq!(
            super::notice_image_url(super::DEFAULT_EVENT_IMAGE, public.as_deref(), &assets),
            "https://node.example.org/prefix/events/assets/event-default.webp"
        );
    }

    #[test]
    fn a_third_party_image_is_preserved_on_read_and_refused_by_upload_policy() {
        let settings = crate::Settings {
            asset_rewrite_base: Some(D.into()),
            configured_public_base: Some(D.into()),
            ..crate::Settings::for_tests()
        };
        let image = "https://images.example.org/cover.png";
        let projected =
            super::event_image_url(image, Some(D), Some(D), &settings.events_asset_base_url);
        assert_eq!(projected, image);
        assert!(super::event_poster_input(image, &settings, "poster").is_none());
    }

    #[test]
    fn poster_inputs_refuse_misleading_origins_and_invalid_paths() {
        use super::event_poster_input;
        use crate::Settings;

        let settings = Settings {
            asset_rewrite_base: Some(D.to_owned()),
            configured_public_base: Some(D.to_owned()),
            events_asset_base_url: format!("{D}/events"),
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
                "https://api.decentraland.org/v1/map.png?name=hello world",
                "https://node.example/api/v1/map.png?name=hello%20world",
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

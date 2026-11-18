//! Node paths for assets referenced by the upstream catalogues.

use url::Url;

use crate::api::posters::HOST_PREFIX;

/// Maps a recognised upstream asset to its path on the node's public domain.
pub fn node_asset_url(url: &str, domain: &str) -> Option<String> {
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

    if host == "api.decentraland.org" && matches!(path, "/v1/map.png" | "/v2/map.png") {
        return Some(format!("https://{domain}/api{path_and_query}"));
    }
    if parsed.query().is_some() {
        return None;
    }

    let prefix = match host {
        host if host.strip_suffix(".decentraland.org") == Some(HOST_PREFIX)
            && ["/poster/", "/poster-vertical/"]
                .into_iter()
                .any(|directory| path.strip_prefix(directory).is_some_and(is_asset_segment)) =>
        {
            HOST_PREFIX
        }
        host if is_peer_host(host)
            && path
                .strip_prefix("/content/contents/")
                .is_some_and(is_asset_segment) =>
        {
            return Some(format!("https://{domain}{path}"));
        }
        "worlds-content-server.decentraland.org"
            if path
                .strip_prefix("/contents/")
                .is_some_and(is_asset_segment) =>
        {
            "worlds-content-server"
        }
        _ => return None,
    };

    Some(format!("https://{domain}/{prefix}{path}"))
}

/// Leaves the stored URL intact when there is no domain or the asset is unrecognised.
pub fn to_node(url: &str, domain: Option<&str>) -> String {
    domain
        .and_then(|domain| node_asset_url(url, domain))
        .unwrap_or_else(|| url.to_owned())
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
    use super::{node_asset_url, to_node};

    const D: &str = "node.example";

    #[test]
    fn maps_each_known_upstream_to_its_node_path() {
        for (upstream, node) in [
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
        for untouched in [
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
    fn no_domain_keeps_the_upstream_url() {
        let u = "https://peer.decentraland.org/content/contents/x";
        assert_eq!(to_node(u, None), u);
    }
}

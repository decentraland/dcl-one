//! Trusted native gateway configuration. The deployment's gateway virtual host
//! preserves service identities. Its apex serves the same APIs at canonical paths.
use system_api_types::services::Service;
use url::Url;

// These additional HUD services have deployed path prefixes matching their host labels.
const HUD_ALIASES: [&str; 4] = ["events", "badges", "notifications", "assets-cdn"];

#[derive(Debug, PartialEq)]
pub struct Gateway {
    prefix: String,
    virtual_host: bool,
}

impl Gateway {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let url =
            Url::parse(raw.trim()).map_err(|_| "--gateway expects an absolute HTTP(S) URL")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "--gateway expects an HTTP(S) base URL without credentials, query or fragment"
                    .into(),
            );
        }
        Ok(Self {
            virtual_host: url
                .host_str()
                .is_some_and(|host| host.starts_with("gateway.")),
            prefix: url.as_str().trim_end_matches('/').to_owned(),
        })
    }

    // Only the HTTP service identities shared with GatewayUrlsSource. For the
    // nginx gateway alias table, peer maps to the root and the local gatekeeper
    // maps to the ordinary gatekeeper. Reels has no /reels gateway alias.
    fn segment(&self, service: Service) -> Option<&'static str> {
        match service {
            Service::Catalyst if !self.virtual_host => Some(""),
            Service::PreviewGatekeeper if !self.virtual_host => Some("comms-gatekeeper"),
            Service::Reels => Some("camera-reel-service"),
            Service::RealmProvider
            | Service::Catalyst
            | Service::WorldsServer
            | Service::Places
            | Service::CommsGatekeeper
            | Service::PreviewGatekeeper
            | Service::AuthApi
            | Service::AssetBundleRegistry
            | Service::SocialApi
            | Service::MapApi => Some(service.composition().1),
            _ => None,
        }
    }

    pub fn service(&self, service: Service) -> Option<String> {
        let segment = self.segment(service)?;
        Some(if segment.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{segment}", self.prefix)
        })
    }

    pub fn hud_services(&self) -> impl Iterator<Item = (String, String)> + '_ {
        HUD_ALIASES
            .into_iter()
            .map(|alias| (alias.to_owned(), format!("{}/{alias}", self.prefix)))
    }

    pub fn profile_image_base(&self) -> String {
        format!("{}/profile-images", self.prefix)
    }

    /// The only URL supplied by a backend (as opposed to one we compose) that
    /// Unity's TransformUrl admits. Match the literal authority to also reject
    /// an explicit default port or empty userinfo erased by a URL parser.
    pub fn profile_image(&self, raw: &str, domain: &str) -> Option<String> {
        let (scheme, rest) = raw.split_once("://")?;
        let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        if !scheme.eq_ignore_ascii_case("https")
            || !rest[..host_end].eq_ignore_ascii_case(&format!("profile-images.{domain}"))
        {
            return None;
        }
        let image = Url::parse(raw).ok()?;
        if has_proxy_dot_segment(image.path()) {
            return None;
        }
        Some(format!(
            "{}{}",
            self.profile_image_base(),
            &image[url::Position::BeforePath..]
        ))
    }

    // nginx sets x-original-path to the path after the service segment only
    // on the gateway virtual host. An apex request signs its entire public path.
    pub fn original_path(&self, raw: &str) -> anyhow::Result<Option<String>> {
        if !self.virtual_host {
            return Ok(None);
        }
        let Ok(target) = Url::parse(raw) else {
            return Ok(None);
        };
        let origin = target.origin().ascii_serialization();
        let Some(mount) = self.prefix.strip_prefix(&origin) else {
            return Ok(None);
        };
        if !mount.is_empty() && !mount.starts_with('/') {
            return Ok(None);
        }

        // reqwest resolves dot segments before sending, while nginx also decodes
        // separators before routing. Never mint a signature for a normalized target.
        let (_, authority_and_path) = raw
            .split_once("://")
            .ok_or_else(|| anyhow::anyhow!("invalid gateway signing URL"))?;
        let public_target = authority_and_path
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        let raw_path = public_target.find('/').map_or("/", |i| &public_target[i..]);
        anyhow::ensure!(
            !public_target.contains('\\')
                && raw_path == target.path()
                && !has_proxy_dot_segment(raw_path),
            "ambiguous gateway signing path"
        );

        let Some(rest) = raw_path
            .strip_prefix(mount)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return Ok(None);
        };
        // nginx strips an alias only when a slash follows it. A bare alias keeps its
        // public path, even when its query contains a slash.
        let (segment, path) = rest.split_once('/').unwrap_or((rest, rest));
        if !HUD_ALIASES.contains(&segment)
            && !Service::all().any(|service| self.segment(service) == Some(segment))
        {
            return Ok(None);
        }
        Ok(Some(format!("/{path}")))
    }
}

// nginx decodes escaped separators before normalizing its proxy path.
fn has_proxy_dot_segment(path: &str) -> bool {
    path.to_ascii_lowercase()
        .replace("%2f", "/")
        .replace("%5c", "/")
        .replace("%2e", ".")
        .split('/')
        .any(|part| matches!(part, "." | ".."))
}

#[cfg(test)]
mod gateway_tests {
    use super::*;

    #[test]
    fn only_configured_service_routes_change_signing_paths() {
        let vhost = Gateway::parse("https://gateway.example.test/mount/").unwrap();
        let gate = Service::CommsGatekeeper;
        assert_eq!(
            vhost.service(gate).as_deref(),
            Some("https://gateway.example.test/mount/comms-gatekeeper")
        );
        assert_eq!(
            vhost
                .original_path("https://gateway.example.test/mount/comms-gatekeeper/a%2Fb?q=1")
                .unwrap()
                .as_deref(),
            Some("/a%2Fb")
        );
        for raw in [
            "https://other.example.test/mount/comms-gatekeeper/a",
            "https://gateway.example.test/mount2/comms-gatekeeper/a",
            "https://gateway.example.test/mount/unknown/a",
        ] {
            assert_eq!(vhost.original_path(raw).unwrap(), None);
        }
        for (target, expected) in [
            ("social-api", Some("/social-api")),
            ("social-api?x=a/b", Some("/social-api")),
            ("social-api/", Some("/")),
            ("social-api/?x=a/b", Some("/")),
            ("social-api/v1/search?x=a/b", Some("/v1/search")),
            (
                "comms-gatekeeper-local/custom/adapter",
                Some("/custom/adapter"),
            ),
        ] {
            let raw = format!("https://gateway.example.test/mount/{target}");
            assert_eq!(
                vhost.original_path(&raw).unwrap().as_deref(),
                expected,
                "{target}"
            );
        }
        assert_eq!(
            vhost
                .original_path("https://user@GATEWAY.example.test:443/mount/social-api/v1/search")
                .unwrap()
                .as_deref(),
            Some("/v1/search")
        );
        assert_eq!(vhost.service(Service::PulseServer), None);
        assert_eq!(vhost.service(Service::AuthPage), None);
        let direct = Gateway::parse("http://localhost:5222").unwrap();
        assert_eq!(
            direct.service(Service::PreviewGatekeeper).as_deref(),
            Some("http://localhost:5222/comms-gatekeeper")
        );
        assert_eq!(
            direct.service(Service::Catalyst).as_deref(),
            Some("http://localhost:5222")
        );
        assert_eq!(
            direct.service(Service::Reels).as_deref(),
            Some("http://localhost:5222/camera-reel-service")
        );
        assert_eq!(
            direct
                .original_path("http://localhost:5222/comms-gatekeeper/a")
                .unwrap(),
            None
        );
    }

    #[test]
    fn profile_images_are_the_only_backend_supplied_hosts_rewritten() {
        let gateway = Gateway::parse("https://gateway.example.test/prefix").unwrap();
        let image = "https://PROFILE-images.example.test/avatar%2Fface?q=x";
        assert_eq!(
            gateway.profile_image(image, "example.test").as_deref(),
            Some("https://gateway.example.test/prefix/profile-images/avatar%2Fface?q=x")
        );
        for raw in [
            "https://profile-images.example.test/bad%2f..%2f..%2fauth",
            "https://profile-images.example.test:443/i",
            "https://profile-images.example.test:444/i",
            "http://profile-images.example.test/i",
            "https://user@profile-images.example.test/i",
            "https://profile-images.example.test.evil/i",
            "https://cdn.example.test/i",
            "https://abgen-cdn.example.test/i",
            "https://profile-images.decentraland.zone/i",
            "https://gateway.example.test/prefix/profile-images/i",
        ] {
            assert_eq!(gateway.profile_image(raw, "example.test"), None, "{raw}");
        }
        for raw in [
            "ftp://host",
            "https://user@host",
            "https://host/?",
            "https://host/#",
        ] {
            assert!(Gateway::parse(raw).is_err(), "{raw}");
        }
    }
}

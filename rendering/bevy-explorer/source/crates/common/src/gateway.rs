//! Trusted native gateway configuration. The deployment's gateway virtual host
//! preserves service identities. Its apex serves the same APIs at canonical paths.
use system_api_types::services::{Service, ServiceValue};
use url::Url;

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

    // API vhosts use their service host label. Direct origins expose the peer at
    // the root, preview on the main gatekeeper, and community thumbnails through social.
    fn segment(&self, service: Service) -> Option<&'static str> {
        match service {
            Service::Catalyst if !self.virtual_host => Some(""),
            Service::PreviewGatekeeper if !self.virtual_host => Some("comms-gatekeeper"),
            Service::AssetsCdn if !self.virtual_host => Some("social-api"),
            // Site links and storage delegations use their own public destinations.
            Service::AuthPage | Service::Reels | Service::Storage => None,
            // Not through the gateway yet: its `/ab-cdn` route serves the asset-bundle CDN and
            // refuses `bevy/` paths, so abgen's CDN is reached directly until it has its own
            // route.
            Service::AbgenCdn => None,
            service if service.value() == ServiceValue::Http => Some(service.composition().1),
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
    pub fn original_path(
        &self,
        raw: &str,
        mut service_bases: impl Iterator<Item = String>,
    ) -> anyhow::Result<Option<String>> {
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
        // A resolved service beneath this mount can have a custom alias and a subpath.
        // A base at the mount itself has no service alias to strip.
        if !service_bases.any(|base| {
            let Some(route) = base
                .strip_prefix(&self.prefix)
                .and_then(|s| s.strip_prefix('/'))
            else {
                return false;
            };
            !route.is_empty()
                && rest
                    .strip_prefix(route)
                    .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        }) {
            return Ok(None);
        }
        // nginx strips only the first segment, and only when followed by a slash.
        let (_, path) = rest.split_once('/').unwrap_or((rest, rest));
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

    // Runs the actual HUD resolver and rendered infrastructure maps; no npm install is needed.
    // cargo test -p common gateway_routes_match_deployment -- --ignored
    #[test]
    #[ignore = "cross-language integration requires Python 3 and Node.js 24+"]
    fn gateway_routes_match_deployment() {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };

        let direct = Gateway::parse("https://example.test").unwrap();
        let vhost = Gateway::parse("https://gateway.example.test").unwrap();
        let mut rows = Vec::new();
        for service in Service::all() {
            let (scheme, sub, path) = service.composition();
            let virtual_base = vhost.service(service);
            let mut signing = Vec::new();
            if let Some(base) = &virtual_base {
                for suffix in ["", "/", "/child?query=x/y"] {
                    let target = format!("{base}{suffix}");
                    let service_bases = Service::all().filter_map(|s| vhost.service(s));
                    let signed = vhost.original_path(&target, service_bases).unwrap();
                    signing.push(serde_json::json!({
                        "target": target,
                        "signed": signed,
                    }));
                }
            }
            rows.push(serde_json::json!({
                "name": service.param(), "value": service.value(), "scheme": scheme,
                "sub": sub, "path": path, "web": service.has_web_param(),
                "direct": direct.service(service), "vhost": virtual_base,
                "fallback": crate::base_domain::service(service), "signing": signing,
            }));
        }
        let script = std::path::Path::new(env!("DCL_ONE_ROOT"))
            .join("infra/gateway/tests/explorer_routes.py");
        let python = std::env::var("PYTHON")
            .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
        let mut child = Command::new(python)
            .arg(script)
            .arg(env!("DCL_ONE_ROOT"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Python 3 is required for the gateway contract test");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&rows).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn signing_matches_explicit_bases_at_path_boundaries() {
        let gateway = Gateway::parse("https://gateway.example.test/mount").unwrap();
        let configured = [
            "https://gateway.example.test/mount",
            "https://gateway.example.test/mount/private-api/custom",
        ];
        for (path, signed) in [
            ("private-api/custom/items", Some("/custom/items")),
            ("private-api/custom", Some("/custom")),
            ("private-api/custom-other/items", None),
            ("private-api/items", None),
            ("v1/communities", None),
        ] {
            let url = format!("https://gateway.example.test/mount/{path}");
            assert_eq!(
                gateway
                    .original_path(&url, configured.into_iter().map(str::to_owned))
                    .unwrap()
                    .as_deref(),
                signed,
                "{path}"
            );
        }
    }

    #[test]
    fn only_configured_service_routes_change_signing_paths() {
        let vhost = Gateway::parse("https://gateway.example.test/mount/").unwrap();
        let sign =
            |raw: &str| vhost.original_path(raw, Service::all().filter_map(|s| vhost.service(s)));
        let gate = Service::CommsGatekeeper;
        assert_eq!(
            vhost.service(gate).as_deref(),
            Some("https://gateway.example.test/mount/comms-gatekeeper")
        );
        assert_eq!(
            sign("https://gateway.example.test/mount/comms-gatekeeper/a%2Fb?q=1")
                .unwrap()
                .as_deref(),
            Some("/a%2Fb")
        );
        for raw in [
            "https://other.example.test/mount/comms-gatekeeper/a",
            "https://gateway.example.test/mount2/comms-gatekeeper/a",
            "https://gateway.example.test/mount/unknown/a",
        ] {
            assert_eq!(sign(raw).unwrap(), None);
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
            assert_eq!(sign(&raw).unwrap().as_deref(), expected, "{target}");
        }
        assert_eq!(
            sign("https://user@GATEWAY.example.test:443/mount/social-api/v1/search")
                .unwrap()
                .as_deref(),
            Some("/v1/search")
        );
        let direct = Gateway::parse("http://localhost:5222").unwrap();
        for external in [
            Service::Storage,
            Service::Reels,
            Service::AuthPage,
            Service::EthereumRpc,
            Service::SocialRpc,
            Service::PulseServer,
        ] {
            assert_eq!(vhost.service(external), None);
            assert_eq!(direct.service(external), None);
        }
        for (service, alias) in [
            (Service::RealmProvider, "realm-provider-ea"),
            (Service::Catalyst, "peer"),
            (Service::WorldsServer, "worlds-content-server"),
            (Service::Places, "places"),
            (Service::CommsGatekeeper, "comms-gatekeeper"),
            (Service::PreviewGatekeeper, "comms-gatekeeper-local"),
            (Service::AuthApi, "auth-api"),
            (Service::SocialApi, "social-api"),
            (Service::MapApi, "api"),
            (Service::Opensea, "opensea"),
            (Service::CameraReel, "camera-reel-service"),
            (Service::Events, "events"),
            (Service::Badges, "badges"),
            (Service::Notifications, "notifications"),
            (Service::AssetsCdn, "assets-cdn"),
            (Service::MarketplaceApi, "marketplace-api"),
            (Service::Autotranslate, "autotranslate-server"),
            (Service::BuilderApi, "builder-api"),
        ] {
            let direct_path = match service {
                Service::Catalyst => String::new(),
                Service::PreviewGatekeeper => "/comms-gatekeeper".to_owned(),
                Service::AssetsCdn => "/social-api".to_owned(),
                _ => format!("/{alias}"),
            };
            assert_eq!(
                direct.service(service),
                Some(format!("http://localhost:5222{direct_path}"))
            );
            assert_eq!(
                vhost.service(service),
                Some(format!("https://gateway.example.test/mount/{alias}"))
            );
            let raw = format!("https://gateway.example.test/mount/{alias}/child");
            assert_eq!(sign(&raw).unwrap().as_deref(), Some("/child"));
        }
        assert_eq!(
            direct
                .original_path(
                    "http://localhost:5222/comms-gatekeeper/a",
                    std::iter::empty()
                )
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

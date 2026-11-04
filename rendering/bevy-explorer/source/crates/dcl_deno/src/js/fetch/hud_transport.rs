//! A signed native HUD request may reach the exact service selected by the operator.
//! Only the engine's service-URL reply to the super-user scene can install these bases.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use common::base_domain::{HudServices, Service};
use dcl_js::SuperUserScene;
use dcl_types::interface::crdt_context::CrdtContext;
use deno_core::anyhow::{bail, Result};
use deno_core::OpState;

struct Grant {
    method: String,
    url: String,
    identity: Vec<(String, String)>,
    created: Instant,
}

struct HudTransport {
    bases: Vec<(Service, String)>,
    pending: VecDeque<Grant>,
    client: Option<reqwest::Client>,
}

pub(crate) fn configure(state: &mut OpState, services: &HudServices) {
    if services.gateway_enabled
        && state.has::<SuperUserScene>()
        && !state.borrow::<CrdtContext>().is_server
        && !state.has::<HudTransport>()
    {
        state.put(HudTransport {
            bases: services.bases.clone(),
            pending: VecDeque::new(),
            client: None,
        });
    }
}

// Match the raw target before URL normalization. No escaped delimiters, dot components,
// credentials, backslashes or arbitrary same-origin routes can reach the private client.
fn eligible_url(bases: &[(Service, String)], method: &str, url: &str) -> Option<reqwest::Url> {
    if url.len() > 4096 || url.contains(['#', '\\']) {
        return None;
    }
    let (_, host_and_path) = url.split_once("://")?;
    let (authority, path) = host_and_path.split_at(host_and_path.find('/')?);
    if authority.is_empty()
        || authority.contains(['@', '?'])
        || url.bytes().any(|b| b.is_ascii_control())
    {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    let origin = parsed.origin().ascii_serialization();
    let path = path.split('?').next()?;
    let allowed = bases.iter().any(|(service, base)| {
        let Some(rest) = base
            .strip_prefix(&origin)
            .filter(|prefix| prefix.is_empty() || prefix.starts_with('/'))
            .and_then(|prefix| path.strip_prefix(prefix))
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return false;
        };
        let parts: Vec<_> = rest.split('/').collect();
        if parts.iter().any(|p| {
            p.is_empty()
                || matches!(*p, "." | "..")
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        }) {
            return false;
        }
        matches!(
            (service, method, parts.as_slice()),
            (Service::Notifications, "GET", ["notifications"])
                | (Service::Notifications, "PUT", ["notifications", "read"])
                | (Service::Events, "GET", ["api", "events"])
                | (Service::MarketplaceApi, "GET", ["v1", "items"])
                | (
                    Service::Badges,
                    "GET",
                    ["users", _, "badges"] | ["badges", _, "tiers"]
                )
                | (
                    Service::CameraReel,
                    "GET",
                    ["api", "users" | "communities", _, "images"]
                        | ["api", "images", _, "metadata"]
                )
                | (Service::CameraReel, "DELETE", ["api", "images", _])
                | (
                    Service::Catalyst,
                    "GET",
                    ["lambdas", "users", _, "names"]
                        | ["lambdas", "outfits", _]
                        | ["lambdas", "collections", "wearables" | "emotes"]
                        | ["explorer", _, "wearables" | "emotes"]
                )
                | (
                    Service::SocialApi,
                    "GET",
                    ["v2", "communities"]
                        | ["v2", "communities", _, "members" | "posts"]
                        | ["v1", "communities", _, "places"]
                )
                | (
                    Service::SocialApi,
                    "POST",
                    ["v1", "communities"] | ["v1", "communities", _, "members" | "requests"]
                )
                | (
                    Service::SocialApi,
                    "PATCH",
                    ["v1", "communities", _, "requests", _]
                )
                | (
                    Service::SocialApi,
                    "DELETE",
                    ["v1", "communities", _, "members", _]
                )
        )
    });
    allowed.then_some(parsed)
}

pub(crate) fn signing_target(state: &OpState, method: &str, raw: &str) -> Option<reqwest::Url> {
    eligible_url(&state.try_borrow::<HudTransport>()?.bases, method, raw)
}

pub(crate) fn signed(
    state: &mut OpState,
    method: &str,
    url: reqwest::Url,
    identity: &[(String, String)],
) {
    if !state.has::<SuperUserScene>() {
        return;
    }
    let Some(transport) = state.try_borrow_mut::<HudTransport>() else {
        return;
    };
    if identity.is_empty()
        || identity
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>()
            > 32768
    {
        return;
    }
    if transport.pending.len() == 64 {
        transport.pending.pop_front();
    }
    transport.pending.push_back(Grant {
        method: method.to_owned(),
        url: url.into(),
        identity: identity.to_vec(),
        created: Instant::now(),
    });
}

pub(super) fn take_client(
    state: &mut OpState,
    method: &str,
    url: &str,
    headers: &[(String, String)],
) -> Result<Option<reqwest::Client>> {
    if !state.has::<SuperUserScene>() {
        return Ok(None);
    }
    let Some(transport) = state.try_borrow_mut::<HudTransport>() else {
        return Ok(None);
    };
    transport
        .pending
        .retain(|g| g.created.elapsed() < Duration::from_secs(30));
    let Some(index) = transport.pending.iter().position(|g| {
        g.method.eq_ignore_ascii_case(method)
            && g.url == url
            && g.identity.iter().all(|(k, v)| {
                headers
                    .iter()
                    .any(|(n, val)| n.eq_ignore_ascii_case(k) && val == v)
            })
    }) else {
        return Ok(None);
    };
    let grant = transport.pending.remove(index).unwrap();
    for (name, value) in headers {
        if ["content-type", "accept", "accept-language"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
        {
            continue;
        }
        if !grant
            .identity
            .iter()
            .any(|(n, v)| name.eq_ignore_ascii_case(n) && value == v)
            || headers
                .iter()
                .filter(|(n, _)| n.eq_ignore_ascii_case(name))
                .count()
                != 1
        {
            bail!("unsupported header on native HUD request")
        }
    }
    if transport.client.is_none() {
        transport.client = Some(
            super::scene_client_builder(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        );
    }
    Ok(transport.client.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_methods_and_literal_paths_are_admitted() {
        let base = "https://example.test/mount/social-api";
        let bases = [(Service::SocialApi, base.to_owned())];
        assert!(eligible_url(
            &bases,
            "POST",
            &format!("{base}/v1/communities/group/members?x=1")
        )
        .is_some());
        assert!(eligible_url(
            &bases,
            "DELETE",
            &format!("{base}/v1/communities/group/members")
        )
        .is_none());
        let raw = "https://EXAMPLE.test:443/mount/social-api/v1/communities?search=two words";
        assert_eq!(
            eligible_url(&bases, "POST", raw).unwrap().as_str(),
            "https://example.test/mount/social-api/v1/communities?search=two%20words"
        );
        for url in [
            "http://example.test/mount/social-api/v1/communities",
            "https://user@example.test/mount/social-api/v1/communities",
            "https://@example.test/mount/social-api/v1/communities",
            "https://example.test?x=/mount/social-api/v1/communities",
            "https://example.test:8443/mount/social-api/v1/communities",
            "https://example.test.evil/mount/social-api/v1/communities",
            "https://example.test/mount/social-api-other/v1/communities",
            "https://example.test/mount/social-api/v1/communities#fragment",
            "https://example.test/mount/social-api/v1/communities/../members",
            "https://example.test/mount/social-api/v1/communities/a%2f..%2fadmin/members",
            "https://example.test/mount/social-api/v1/communities/a%252fadmin/members",
            "https://example.test/mount/social-api/v1/communities/a\\admin/members",
        ] {
            assert!(eligible_url(&bases, "POST", url).is_none(), "{url}");
        }
    }
}

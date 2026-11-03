//! A signed native HUD request may reach the exact service selected by the operator.
//! Only an engine bootstrap on the registered super-user stream can install these bases.
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use dcl_js::SuperUserScene;
use dcl_types::interface::crdt_context::CrdtContext;
use deno_core::anyhow::{bail, Result};
use deno_core::{OpState, ResourceId};
use serde::Deserialize;

pub(crate) struct BootstrapStream(pub ResourceId);

#[derive(Deserialize)]
struct Bootstrap {
    to: String,
    gateway: bool,
    bases: HashMap<String, String>,
}

struct Grant {
    method: String,
    url: String,
    identity: Vec<(String, String)>,
    created: Instant,
}

struct HudTransport {
    bases: HashMap<String, String>,
    pending: VecDeque<Grant>,
    client: Option<reqwest::Client>,
}

pub(crate) fn bootstrap(state: &mut OpState, rid: ResourceId, frame: &str) {
    if state.try_borrow::<BootstrapStream>().map(|stream| stream.0) != Some(rid) {
        return;
    }
    state.take::<BootstrapStream>(); // Only the first engine frame on this exact stream.
    state.try_take::<HudTransport>();
    if !state.has::<SuperUserScene>() || state.borrow::<CrdtContext>().is_server {
        return;
    }
    let Ok(message) = serde_json::from_str::<Bootstrap>(frame) else {
        return;
    };
    if message.to == "nativeHudServices" && message.gateway {
        state.put(HudTransport {
            bases: message.bases,
            pending: VecDeque::new(),
            client: None,
        });
    }
}

// Match the raw target before URL normalization. No escaped delimiters, dot components,
// credentials, backslashes or arbitrary same-origin routes can reach the private client.
fn allowed(bases: &HashMap<String, String>, method: &str, url: &str) -> bool {
    if url.len() > 4096 || url.contains(['#', '\\']) {
        return false;
    }
    let path = url.split('?').next().unwrap_or(url);
    bases.iter().any(|(service, base)| {
        let Some(rest) = path.strip_prefix(base).and_then(|s| s.strip_prefix('/')) else {
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
            (service.as_str(), method, parts.as_slice()),
            ("notifications", "GET", ["notifications"])
                | ("notifications", "PUT", ["notifications", "read"])
                | ("events", "GET", ["api", "events"])
                | (
                    "badges",
                    "GET",
                    ["users", _, "badges"] | ["badges", _, "tiers"]
                )
                | (
                    "reels",
                    "GET",
                    ["api", "users" | "communities", _, "images"]
                        | ["api", "images", _, "metadata"]
                )
                | ("reels", "DELETE", ["api", "images", _])
                | (
                    "catalyst",
                    "GET",
                    ["lambdas", "users", _, "names"]
                        | ["lambdas", "outfits", _]
                        | ["lambdas", "collections", "wearables" | "emotes"]
                        | ["explorer", _, "wearables" | "emotes"]
                )
                | (
                    "socialApi",
                    "GET",
                    ["v2", "communities"]
                        | ["v2", "communities", _, "members" | "posts"]
                        | ["v1", "communities", _, "places"]
                )
                | (
                    "socialApi",
                    "POST",
                    ["v1", "communities"] | ["v1", "communities", _, "members" | "requests"]
                )
                | (
                    "socialApi",
                    "PATCH",
                    ["v1", "communities", _, "requests", _]
                )
                | (
                    "socialApi",
                    "DELETE",
                    ["v1", "communities", _, "members", _]
                )
        )
    })
}

pub(crate) fn signed(state: &mut OpState, method: &str, url: &str, identity: &[(String, String)]) {
    if !state.has::<SuperUserScene>() {
        return;
    }
    let Some(transport) = state.try_borrow_mut::<HudTransport>() else {
        return;
    };
    let method = method.to_ascii_uppercase();
    if !allowed(&transport.bases, &method, url)
        || identity.is_empty()
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
        method,
        url: url.to_owned(),
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
                .timeout(Duration::from_secs(15))
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
        let bases = HashMap::from([("socialApi".to_owned(), base.to_owned())]);
        assert!(allowed(
            &bases,
            "POST",
            &format!("{base}/v1/communities/group/members?x=1")
        ));
        assert!(!allowed(
            &bases,
            "DELETE",
            &format!("{base}/v1/communities/group/members")
        ));
        for url in [
            "http://example.test/mount/social-api/v1/communities",
            "https://example.test:8443/mount/social-api/v1/communities",
            "https://example.test.evil/mount/social-api/v1/communities",
            "https://example.test/mount/social-api-other/v1/communities",
            "https://example.test/mount/social-api/v1/communities#fragment",
            "https://example.test/mount/social-api/v1/communities/../members",
            "https://example.test/mount/social-api/v1/communities/a%2f..%2fadmin/members",
            "https://example.test/mount/social-api/v1/communities/a%252fadmin/members",
            "https://example.test/mount/social-api/v1/communities/a\\admin/members",
        ] {
            assert!(!allowed(&bases, "POST", url), "{url}");
        }
    }
}

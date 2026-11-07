//! The `dapps` rollout flags this node enforces: decentraland.org's feature-flags file, the one sites and the
//! Shop's pages read (apps/ui3 discovery/lib/remoteFlags.ts), asked as decentraland.org (the service answers by
//! Referer). The fetched document is held in memory only: `explore_api.feature_flags` also has a `dapps` row,
//! which Social writes asking as its own service, and the two answers may differ. On top of it go this node's
//! local overrides (`explore_api.feature_flags_local`, app `dapps`). Before the first answer, and when the service
//! is not configured, only the overrides count: every other flag is off and the features it opens stay closed.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::MarketplaceState;
use crate::explore::api::feature_flags;
use crate::jobs::{Jobs, PeriodicCfg};

/// The application the Shop's flags are read under: a key `shop-x` is `dapps-shop-x` in the file.
pub const APP: &str = "dapps";

/// The Marketplace inside the Shop (apps/ui3 market/access.tsx): resales, bids, LAND, NAME resale, listing an
/// owned token, favorites lists. Off, this node refuses them: a resale or a rental is not quoted, an NFT order or
/// a bid is not accepted, a list is not written, a LAND rental is not listed, and the federation log takes no bid
/// or order. The refusal holds on every mount of these routes, `/marketplace-api` included: scoping it to the
/// Shop's paths would leave the same writes open one path over.
pub const MARKETPLACE_FEATURES: &str = "shop-marketplace-features";

const TIMEOUT: Duration = Duration::from_secs(5);

/// Where the flags come from (`SHOP_FEATURE_FLAGS_*`, as sites' `FEATURE_FLAGS_URL` and `FEATURE_FLAGS_ORIGIN`).
#[derive(Clone, Debug)]
pub struct Source {
    /// The service's base URL; empty reads only the local overrides.
    pub url: String,
    /// The site the flags are asked for (an origin such as `https://decentraland.org`).
    pub origin: String,
    pub refresh: Duration,
}

impl Default for Source {
    fn default() -> Self {
        Self {
            url: String::new(),
            origin: String::new(),
            refresh: Duration::from_secs(60),
        }
    }
}

/// The flags as this node enforces them, and the last document the service answered.
pub struct DappsFlags {
    snapshot: feature_flags::Snapshot,
    fetched: Mutex<Option<Value>>,
}

impl DappsFlags {
    /// Every flag off until the first read. Tests (and `test-support` builds) start with the Marketplace on, so a
    /// test of a resale, a bid or a list exercises it; a test of the rollout switches it off.
    pub(super) fn new() -> Self {
        let on = cfg!(any(test, feature = "test-support"));
        Self {
            snapshot: feature_flags::Snapshot::from_document(document(on)),
            fetched: Mutex::new(None),
        }
    }

    fn enabled(&self, feature: &str) -> bool {
        self.snapshot.enabled(APP, feature)
    }
}

/// The refusal's message, the same on every route that answers it.
pub(crate) fn refusal() -> String {
    format!("the Marketplace's features are off on this node ({APP}-{MARKETPLACE_FEATURES})")
}

fn document(marketplace: bool) -> Value {
    let flags = if marketplace {
        json!({ format!("{APP}-{MARKETPLACE_FEATURES}"): true })
    } else {
        json!({})
    };
    json!({ "flags": flags, "variants": {} })
}

impl MarketplaceState {
    pub fn with_feature_flags(mut self, source: Source) -> Self {
        self.flags_source = source;
        self
    }

    /// Whether this node offers the Marketplace's features (see [`MARKETPLACE_FEATURES`]).
    pub fn marketplace_features(&self) -> bool {
        self.dapps_flags.enabled(MARKETPLACE_FEATURES)
    }

    /// Refuses one of the Marketplace's calls while its rollout is off: a 403 that names the flag.
    pub(crate) fn require_marketplace(&self) -> Result<(), super::wire::ApiError> {
        if self.marketplace_features() {
            return Ok(());
        }
        Err(super::wire::ApiError::http(403, refusal()))
    }

    /// Reads the service (when configured) and applies the local overrides on top. A failed read keeps the last
    /// document the service answered; before any answer only the overrides count.
    pub async fn refresh_feature_flags(&self) -> anyhow::Result<()> {
        if !self.flags_source.url.is_empty() {
            match self.fetch_feature_flags().await {
                Ok(doc) => *self.dapps_flags.fetched.lock() = Some(doc),
                Err(error) => {
                    tracing::warn!(%error, "dapps feature flags: keeping the last answer")
                }
            }
        }
        let base = self
            .dapps_flags
            .fetched
            .lock()
            .clone()
            .unwrap_or_else(|| document(false));
        let doc = feature_flags::with_local_overrides(&self.pool, APP, base)
            .await
            .context("reading the dapps feature flag overrides")?;
        self.dapps_flags.snapshot.replace(doc);
        Ok(())
    }

    async fn fetch_feature_flags(&self) -> anyhow::Result<Value> {
        let source = &self.flags_source;
        let url = format!("{}/{APP}.json", source.url.trim_end_matches('/'));
        // The Shop's outbound guard (as its Stripe, captcha and RPC URLs): no private, internal or single-label host.
        if !crate::http::is_safe_http_url(&url) {
            anyhow::bail!(
                "{url}: refused by the SSRF guard (private, internal or single-label host)"
            );
        }
        let target = reqwest::Url::parse(&url).context("SHOP_FEATURE_FLAGS_URL is not a URL")?;
        let referer = format!("{}/", source.origin.trim_end_matches('/'));
        feature_flags::fetch_document(&self.http, target, &referer, TIMEOUT).await
    }
}

#[cfg(any(test, feature = "test-support"))]
impl MarketplaceState {
    pub fn set_marketplace_features(&self, on: bool) {
        self.dapps_flags.snapshot.replace(document(on));
    }
}

/// The refresh job: at start, then every `SHOP_FEATURE_FLAGS_REFRESH_SECS`.
pub(super) fn spawn_job(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let state = state.clone();
    let interval = state.flags_source.refresh.max(Duration::from_secs(10));
    // The service answers by Referer: the site asked as decides the answer, so it is logged once.
    if state.flags_source.url.is_empty() {
        tracing::info!(
            "dapps feature flags: no service configured, only this node's overrides count"
        );
    } else {
        tracing::info!(url = %state.flags_source.url, origin = %state.flags_source.origin, "dapps feature flags");
    }
    jobs.spawn_periodic(
        "shop-dapps-flags",
        PeriodicCfg::every(interval),
        move |_ctx| {
            let state = state.clone();
            async move { state.refresh_feature_flags().await }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marketplace_key_is_the_one_the_pages_read() {
        // apps/ui3 discovery/lib/remoteFlags.ts: REMOTE_FLAG.marketplaceFeatures, read as `dapps-` + key.
        assert_eq!(
            format!("{APP}-{MARKETPLACE_FEATURES}"),
            "dapps-shop-marketplace-features"
        );
    }

    #[test]
    fn a_document_names_the_marketplace_only_when_it_is_on() {
        let on = feature_flags::Snapshot::from_document(document(true));
        let off = feature_flags::Snapshot::from_document(document(false));
        assert!(on.enabled(APP, MARKETPLACE_FEATURES));
        assert!(!off.enabled(APP, MARKETPLACE_FEATURES));
    }
}

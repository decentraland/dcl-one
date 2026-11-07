//! The `dapps` rollout flags this node enforces: decentraland.org's feature-flags file, the one sites and the
//! Shop's pages read (apps/ui3 discovery/lib/remoteFlags.ts). One snapshot, refreshed on a timer from the service
//! when `SHOP_FEATURE_FLAGS_URL` names one, and always reloaded from the stored document with its local
//! overrides (`explore_api.feature_flags`, `explore_api.feature_flags_local`). Before the first answer, and
//! whenever the file cannot be read, every flag is off: the features they open stay closed.

use std::sync::Arc;
use std::time::Duration;

use crate::explore::api::feature_flags;
use anyhow::Context as _;
use serde_json::Value;

use super::MarketplaceState;
use crate::jobs::{Jobs, PeriodicCfg};

/// The application the Shop's flags are read under: a key `shop-x` is `dapps-shop-x` in the file.
pub const APP: &str = "dapps";

/// The Marketplace inside the Shop (apps/ui3 market/access.tsx): resales, bids, LAND, NAME resale, listing an
/// owned token, favorites lists. Off, this node refuses them: a resale is not quoted, an NFT order or a bid is
/// not accepted, and a list is not written.
pub const MARKETPLACE_FEATURES: &str = "shop-marketplace-features";

const BODY_CAP: usize = 4 << 20;
const TIMEOUT: Duration = Duration::from_secs(5);

/// Where the flags come from (`SHOP_FEATURE_FLAGS_*`, as sites' `FEATURE_FLAGS_URL` and `FEATURE_FLAGS_ORIGIN`).
#[derive(Clone, Debug)]
pub struct Source {
    /// The service's base URL; empty reads only the stored document and its local overrides.
    pub url: String,
    /// The site the flags are asked for: the service answers by the request's Referer.
    pub origin: String,
    pub refresh: Duration,
}

impl Default for Source {
    fn default() -> Self {
        Self {
            url: String::new(),
            origin: "https://decentraland.org".into(),
            refresh: Duration::from_secs(60),
        }
    }
}

impl MarketplaceState {
    pub fn with_feature_flags(mut self, source: Source) -> Self {
        self.flags_source = source;
        self
    }

    /// Whether this node offers the Marketplace's features (see [`MARKETPLACE_FEATURES`]).
    pub fn marketplace_features(&self) -> bool {
        self.dapps_flags.enabled(APP, MARKETPLACE_FEATURES)
    }

    /// Refuses one of the Marketplace's calls while its rollout is off: a 403 that names the flag.
    pub(crate) fn require_marketplace(&self) -> Result<(), super::wire::ApiError> {
        if self.marketplace_features() {
            return Ok(());
        }
        Err(super::wire::ApiError::http(
            403,
            format!(
                "the Marketplace's features are off on this node ({APP}-{MARKETPLACE_FEATURES})"
            ),
        ))
    }

    /// Reads the service (when configured) into the stored document, then reloads the snapshot from it. A
    /// failed read keeps the stored document: the last answer stands, and before any answer every flag is off.
    pub async fn refresh_feature_flags(&self) -> anyhow::Result<()> {
        if !self.flags_source.url.is_empty() {
            if let Err(error) = self.fetch_feature_flags().await {
                tracing::warn!(%error, "dapps feature flags: keeping the stored document");
            }
        }
        self.dapps_flags
            .reload(&self.pool, APP)
            .await
            .context("reloading the dapps feature flags")
    }

    async fn fetch_feature_flags(&self) -> anyhow::Result<()> {
        let source = &self.flags_source;
        let url = format!("{}/{APP}.json", source.url.trim_end_matches('/'));
        let response = self
            .http
            .get(url.as_str())
            .header(
                "referer",
                format!("{}/", source.origin.trim_end_matches('/')),
            )
            .timeout(TIMEOUT)
            .send()
            .await
            .context("fetching the dapps feature flags")?;
        if !response.status().is_success() {
            anyhow::bail!("the dapps feature flags answered {}", response.status());
        }
        let body = crate::http::read_body_capped(response, BODY_CAP)
            .await
            .context("reading the dapps feature flags")?;
        let doc: Value =
            serde_json::from_slice(&body).context("the dapps feature flags are not JSON")?;
        feature_flags::validate(&doc)?;
        feature_flags::store(&self.pool, APP, &doc, None)
            .await
            .context("storing the dapps feature flags")?;
        Ok(())
    }
}

/// Every flag off until the first read. Tests (and `test-support` builds) start with the Marketplace on, so a test
/// of a resale, a bid or a list exercises it; a test of the rollout switches it off ([`set_marketplace_features`]).
pub(super) fn initial_snapshot() -> feature_flags::Snapshot {
    #[cfg(any(test, feature = "test-support"))]
    {
        feature_flags::Snapshot::from_document(serde_json::json!({
            "flags": { format!("{APP}-{MARKETPLACE_FEATURES}"): true },
            "variants": {}
        }))
    }
    #[cfg(not(any(test, feature = "test-support")))]
    {
        feature_flags::Snapshot::empty()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl MarketplaceState {
    pub fn set_marketplace_features(&self, on: bool) {
        self.dapps_flags.set_document(serde_json::json!({
            "flags": { format!("{APP}-{MARKETPLACE_FEATURES}"): on },
            "variants": {}
        }));
    }
}

/// The refresh job: at start, then every `SHOP_FEATURE_FLAGS_REFRESH_SECS`.
pub(super) fn spawn_job(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let state = state.clone();
    let interval = state.flags_source.refresh.max(Duration::from_secs(10));
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
}

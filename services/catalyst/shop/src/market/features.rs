//! The `dapps` rollout flags this node enforces: decentraland.org's feature-flags file, the one sites and the
//! Shop's pages read (apps/ui3 discovery/lib/remoteFlags.ts), asked as decentraland.org (the service answers by
//! Referer). The fetched document is held in memory only: `explore_api.feature_flags` also has a `dapps` row,
//! which Social writes asking as its own service, and the two answers may differ. On top of it go this node's
//! local overrides (`explore_api.feature_flags_local`, app `dapps`). Before the first answer, and when the service
//! is not configured, only the overrides count: every other flag is off and the features it opens stay closed. A
//! failed read keeps the last answer for a while ([`STALE_AFTER_REFRESHES`] refreshes, ten minutes at least), so a
//! blip does not switch features off; past that it is dropped and only the overrides count again, so a flag
//! switched off upstream reaches a node that cannot hear the service.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context as _;
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::MarketplaceState;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::feature_flags;

/// The application the Shop's flags are read under: a key `shop-x` is `dapps-shop-x` in the file.
pub const APP: &str = "dapps";

/// The Marketplace inside the Shop (apps/ui3 market/access.tsx): resales, bids, LAND, NAME resale, listing an
/// owned token, favorites lists. Off, this node refuses them: a resale or a rental is not quoted, an NFT order or
/// a bid is not accepted, a list is not written, a LAND rental is not listed, and the federation log takes no bid
/// or order. The refusal holds on every mount of these routes, `/marketplace-api` included: scoping it to the
/// Shop's paths would leave the same writes open one path over.
pub const MARKETPLACE_FEATURES: &str = "shop-marketplace-features";

/// Credits and MANA paying one line together (the Shop's Credits + MANA split): a Credits part (`creditsCents`) of one
/// line of one unit, the rest pulled from the buyer's MANA in the same `useCredits`. Off, this node takes no Credits
/// part on either credits authorize route, a replayed operation included, and its quotes mark no offer `splittable`.
/// A whole price, and every credit already issued (its status, settlement, expiry and cancellation), are untouched.
pub const CREDITS_MANA_SPLIT: &str = "shop-credits-mana-split";

/// The outfit authoring allowlist (`shop::outfits`): the variant payload of `dapps-shop-outfit-creators`, the flag
/// the Shop's pages read to show the studio.
pub const OUTFIT_CREATORS: &str = "shop-outfit-creators";

const TIMEOUT: Duration = Duration::from_secs(5);

/// How many refresh periods a failed read keeps the last answer for.
pub const STALE_AFTER_REFRESHES: u32 = 10;
const STALE_AT_LEAST: Duration = Duration::from_secs(600);

/// Whether an answer read `age` ago still stands while the service cannot be read.
fn still_stands(age: Duration, refresh: Duration) -> bool {
    age <= (refresh * STALE_AFTER_REFRESHES).max(STALE_AT_LEAST)
}

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
    fetched: Mutex<Option<(Value, Instant)>>,
    /// The local overrides as last read, applied while they cannot be read again; `None` until the first read.
    overrides: Mutex<Option<Vec<(String, bool)>>>,
}

impl DappsFlags {
    /// Every flag off until the first read. Tests (and `test-support` builds) start with the Marketplace on, so a
    /// test of a resale, a bid or a list exercises it; a test of the rollout switches it off.
    pub(super) fn new() -> Self {
        let on = cfg!(any(test, feature = "test-support"));
        Self {
            snapshot: feature_flags::Snapshot::from_document(document(on), false),
            fetched: Mutex::new(None),
            overrides: Mutex::new(None),
        }
    }

    fn enabled(&self, feature: &str) -> bool {
        self.snapshot.enabled(APP, feature)
    }

    /// Whether this node takes a Credits part (see [`CREDITS_MANA_SPLIT`]).
    pub fn credits_mana_split(&self) -> bool {
        self.enabled(CREDITS_MANA_SPLIT)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl DappsFlags {
    /// Flags as a node holds them after reading `flags` (`dapps-` keys): for a test of a service that reads them.
    pub fn for_tests(flags: &[(&str, bool)]) -> Arc<Self> {
        let held = Self::new();
        held.snapshot
            .replace_sourced(json!({ "flags": {}, "variants": {} }), false);
        for (feature, on) in flags {
            held.set_for_tests(feature, *on);
        }
        Arc::new(held)
    }

    /// Turns one `dapps` flag (`shop-x`) on or off, keeping the rest of the document.
    pub fn set_for_tests(&self, feature: &str, on: bool) {
        let (doc, sourced) = self.snapshot.held();
        let mut doc = (*doc).clone();
        doc["flags"][format!("{APP}-{feature}")] = json!(on);
        self.snapshot.replace_sourced(doc, sourced);
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

    /// Whether this node takes a Credits part (see [`CREDITS_MANA_SPLIT`]).
    pub fn credits_mana_split(&self) -> bool {
        self.dapps_flags.credits_mana_split()
    }

    /// The flags as this node enforces them, for a service that reads them beside the market (the credits routes).
    pub fn dapps_flags(&self) -> Arc<DappsFlags> {
        self.dapps_flags.clone()
    }

    /// A `dapps` variant's payload value while its flag is on (`@dcl/features-component` `getFeatureVariant`);
    /// `None` while the flag is off, unset, or the document is not held (see the module doc).
    pub fn dapps_variant(&self, feature: &str) -> Option<String> {
        let key = format!("{APP}-{feature}");
        let doc = self.dapps_flags.snapshot.document();
        if doc["flags"][&key].as_bool() != Some(true) {
            return None;
        }
        doc["variants"][&key]["payload"]["value"]
            .as_str()
            .map(str::to_string)
    }

    /// Refuses one of the Marketplace's calls while its rollout is off: a 403 that names the flag.
    pub(crate) fn require_marketplace(&self) -> Result<(), super::wire::ApiError> {
        if self.marketplace_features() {
            return Ok(());
        }
        Err(super::wire::ApiError::http(403, refusal()))
    }

    /// Reads the service (when configured) and applies the local overrides on top. A failed read keeps the last
    /// document the service answered while it still stands (see the module doc); before any answer, and once it
    /// no longer does, only the overrides count. The overrides that cannot be read are the ones last read, and the
    /// document is replaced either way, so an answer that no longer stands is dropped. Until the overrides have been
    /// read once every flag is off, so a local `false` is never skipped.
    pub async fn refresh_feature_flags(&self) -> anyhow::Result<()> {
        if !self.flags_source.url.is_empty() {
            match self.fetch_feature_flags().await {
                Ok(doc) => *self.dapps_flags.fetched.lock() = Some((doc, Instant::now())),
                Err(error) => {
                    let mut held = self.dapps_flags.fetched.lock();
                    match held.as_ref().map(|(_, at)| at.elapsed()) {
                        Some(age) if still_stands(age, self.flags_source.refresh) => {
                            tracing::warn!(%error, age_secs = age.as_secs(), "dapps feature flags: keeping the last answer")
                        }
                        Some(age) => {
                            tracing::error!(%error, age_secs = age.as_secs(), "dapps feature flags: the last answer is too old, only this node's overrides count");
                            *held = None;
                        }
                        None => {
                            tracing::warn!(%error, "dapps feature flags: no answer yet, only this node's overrides count")
                        }
                    }
                }
            }
        }
        let fetched = self
            .dapps_flags
            .fetched
            .lock()
            .as_ref()
            .map(|(doc, _)| doc.clone());
        let sourced = fetched.is_some();
        let base = fetched.unwrap_or_else(|| document(false));
        let read = feature_flags::local_overrides(&self.pool, APP).await;
        let overrides = match &read {
            Ok(overrides) => {
                *self.dapps_flags.overrides.lock() = Some(overrides.clone());
                Some(overrides.clone())
            }
            Err(_) => self.dapps_flags.overrides.lock().clone(),
        };
        let (doc, sourced) = match overrides {
            Some(overrides) => (
                feature_flags::apply_local_overrides(base, &overrides),
                sourced,
            ),
            None => (document(false), false),
        };
        self.dapps_flags.snapshot.replace_sourced(doc, sourced);
        read.map(drop)
            .context("reading the dapps feature flag overrides")
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
        self.dapps_flags
            .snapshot
            .replace_sourced(document(on), false);
    }

    pub fn set_dapps_document(&self, doc: Value) {
        self.dapps_flags.snapshot.replace_sourced(doc, false);
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
    fn the_split_key_is_the_one_the_pages_read_and_reads_off_until_it_says_true() {
        // library/shop discovery/lib/rolloutFlags.ts: REMOTE_FLAG.creditsManaSplit, read as `dapps-` + key.
        assert_eq!(
            format!("{APP}-{CREDITS_MANA_SPLIT}"),
            "dapps-shop-credits-mana-split"
        );
        // A node starts with it off, the Marketplace's test default notwithstanding; absent and not `true` read off.
        assert!(!DappsFlags::new().credits_mana_split());
        let flags = DappsFlags::for_tests(&[]);
        assert!(!flags.credits_mana_split());
        flags.snapshot.replace_sourced(
            json!({ "flags": { "dapps-shop-credits-mana-split": "true" } }),
            false,
        );
        assert!(!flags.credits_mana_split(), "only `true` turns it on");
        flags.set_for_tests(CREDITS_MANA_SPLIT, true);
        assert!(flags.credits_mana_split());
    }

    #[test]
    fn a_failed_read_keeps_the_last_answer_for_ten_refreshes_and_ten_minutes_at_least() {
        let minute = Duration::from_secs(60);
        assert!(still_stands(
            Duration::from_secs(599),
            Duration::from_secs(10)
        ));
        assert!(!still_stands(
            Duration::from_secs(601),
            Duration::from_secs(10)
        ));
        assert!(still_stands(minute * 10, minute));
        assert!(still_stands(minute * 20, minute * 2));
        assert!(!still_stands(minute * 21, minute * 2));
    }

    /// An answer too old to stand is dropped even when the overrides cannot be read: the overrides last read
    /// still apply, every other flag is off.
    #[tokio::test]
    async fn an_answer_too_old_is_dropped_while_the_overrides_cannot_be_read() {
        let unreachable = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(200))
            .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")
            .unwrap();
        let state = MarketplaceState::new(
            unreachable,
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
        )
        .with_feature_flags(Source {
            // Refused by the outbound guard, so the read fails without a network.
            url: "http://127.0.0.1".into(),
            ..Source::default()
        });
        let outfits = format!("{APP}-{OUTFIT_CREATORS}");
        let long_ago = Instant::now() - Duration::from_secs(3600);
        *state.dapps_flags.fetched.lock() = Some((document(true), long_ago));
        *state.dapps_flags.overrides.lock() = Some(vec![(outfits.clone(), true)]);
        assert!(state.marketplace_features());

        assert!(state.refresh_feature_flags().await.is_err());
        assert!(state.dapps_flags.fetched.lock().is_none());
        assert!(!state.marketplace_features(), "the old answer is dropped");
        assert!(
            state.dapps_flags.snapshot.enabled(APP, OUTFIT_CREATORS),
            "the overrides last read still apply"
        );
    }

    /// Before the overrides have been read once, a held answer does not apply: a local `false` could be missing.
    #[tokio::test]
    async fn every_flag_stays_off_until_the_overrides_have_been_read_once() {
        let unreachable = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(200))
            .connect_lazy("postgres://postgres@127.0.0.1:1/postgres")
            .unwrap();
        let state = MarketplaceState::new(
            unreachable,
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
        );
        *state.dapps_flags.fetched.lock() = Some((document(true), Instant::now()));
        assert!(state.marketplace_features());

        assert!(state.refresh_feature_flags().await.is_err());
        assert!(
            state.dapps_flags.fetched.lock().is_some(),
            "the answer is still held"
        );
        assert!(
            !state.marketplace_features(),
            "no flag applies before the overrides are read"
        );
    }

    #[test]
    fn a_document_names_the_marketplace_only_when_it_is_on() {
        let on = feature_flags::Snapshot::from_document(document(true), false);
        let off = feature_flags::Snapshot::from_document(document(false), false);
        assert!(on.enabled(APP, MARKETPLACE_FEATURES));
        assert!(!off.enabled(APP, MARKETPLACE_FEATURES));
    }
}

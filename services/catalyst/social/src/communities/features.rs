//! DAPPS decisions from the shared flags mirror. Source: social-service-ea
//! b6b2cef, adapters/feature-flags.ts and @dcl/features-component 1.0.2.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use crate::jobs::{Jobs, PeriodicCfg};

pub const AI_COMPLIANCE: &str = "communities_ai_compliance";
pub const DEV_AI_COMPLIANCE: &str = "dev_communities_ai_compliance";
pub const GLOBAL_MODERATORS: &str = "communities_global_moderators";
pub const REFERRAL_REGISTRATION_DISABLED: &str = "referral_registration_disabled";
/// The explorer flag whose `wallets` variant names the official wallets.
pub const OFFICIAL_WALLETS: &str = "explorer-alfa-official-wallets";

/// The official wallets in an explorer flags document, lowercased: while the flag is on, its
/// variant named `wallets` with a `csv` payload, of which only the first line counts (as the
/// Explorer clients read it). Anything else names none.
pub fn official_wallets_in(doc: &serde_json::Value) -> HashSet<String> {
    if doc["flags"][OFFICIAL_WALLETS].as_bool() != Some(true) {
        return HashSet::new();
    }
    let variant = &doc["variants"][OFFICIAL_WALLETS];
    let payload = &variant["payload"];
    let csv = payload["type"]
        .as_str()
        .is_some_and(|t| t.eq_ignore_ascii_case("csv"));
    let (Some("wallets"), true, Some(value)) =
        (variant["name"].as_str(), csv, payload["value"].as_str())
    else {
        return HashSet::new();
    };
    let first = value
        .split('\n')
        .next()
        .unwrap_or("")
        .trim_end_matches('\r');
    first
        .split(',')
        .filter(|a| !a.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub base_url: String,
    pub audience: String,
    pub interval: Duration,
    pub timeout: Duration,
    /// The SDK treats a nonempty override as enabled only for literal `1`.
    pub overrides: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            audience: "https://social-service-ea.decentraland.zone".into(),
            interval: Duration::from_secs(240),
            timeout: Duration::from_secs(10),
            overrides: HashMap::new(),
        }
    }
}

impl super::State {
    /// The wallets whose posts carry the official mark (`official_wallets_in`); none before
    /// explore's flags are attached.
    pub fn official_wallets(&self) -> HashSet<String> {
        self.explorer_flags
            .get()
            .map(|flags| official_wallets_in(&flags.document()))
            .unwrap_or_default()
    }

    pub fn feature_enabled(&self, feature: &str) -> bool {
        if let Some(value) = self
            .settings
            .features
            .overrides
            .get(feature)
            .filter(|v| !v.is_empty())
        {
            return value == "1";
        }
        self.flags.enabled("dapps", feature)
    }

    pub async fn load_flags(&self) -> sqlx::Result<()> {
        self.flags.reload(&self.pool, "dapps").await
    }

    pub async fn refresh_flags(&self, api: &crate::explore::api::Shared) -> anyhow::Result<()> {
        let settings = &self.settings.features;
        if settings.base_url.is_empty() {
            return Ok(());
        }
        let timeout = if settings.timeout.is_zero() {
            Duration::from_secs(10)
        } else {
            settings.timeout
        };
        crate::explore::api::feature_flags::refresh_app(
            api,
            &self.flags,
            "dapps",
            &format!("{}/dapps.json", settings.base_url.trim_end_matches('/')),
            &settings.audience,
            timeout,
        )
        .await
    }

    /// Every process refreshes its own snapshot. The four booleans and the
    /// moderator variant always come from the same successfully stored document.
    pub fn spawn_flags_job(self: &Arc<Self>, jobs: &Jobs, api: crate::explore::api::Shared) {
        if self.settings.features.base_url.is_empty() {
            return;
        }
        let state = self.clone();
        let interval = if state.settings.features.interval.is_zero() {
            Duration::from_secs(240)
        } else {
            state.settings.features.interval
        };
        jobs.spawn_periodic(
            "social-dapps-flags",
            PeriodicCfg::every(interval).after_first_interval(),
            move |_ctx| {
                let (state, api) = (state.clone(), api.clone());
                async move { state.refresh_flags(&api).await }
            },
        );
    }
}

#[cfg(test)]
mod official_wallet_tests {
    use super::*;
    use serde_json::json;

    fn doc(on: bool, name: &str, kind: &str, value: &str) -> serde_json::Value {
        json!({
            "flags": { OFFICIAL_WALLETS: on },
            "variants": { OFFICIAL_WALLETS: { "name": name, "payload": { "type": kind, "value": value } } }
        })
    }

    #[test]
    fn reads_the_first_csv_line_lowercased_while_the_flag_is_on() {
        let wallets = official_wallets_in(&doc(true, "wallets", "CSV", "0xAA,,0xbb\r\n0xcc"));
        assert_eq!(
            wallets,
            HashSet::from(["0xaa".to_string(), "0xbb".to_string()])
        );
    }

    #[test]
    fn names_none_when_off_or_malformed() {
        assert!(official_wallets_in(&doc(false, "wallets", "csv", "0xaa")).is_empty());
        assert!(official_wallets_in(&doc(true, "other", "csv", "0xaa")).is_empty());
        assert!(official_wallets_in(&doc(true, "wallets", "json", "0xaa")).is_empty());
        assert!(official_wallets_in(&json!({ "flags": {}, "variants": {} })).is_empty());
    }

    #[test]
    fn the_bundled_explorer_document_names_official_wallets() {
        let bundled = crate::explore::api::feature_flags::embedded_document();
        assert!(!official_wallets_in(&bundled).is_empty());
    }
}

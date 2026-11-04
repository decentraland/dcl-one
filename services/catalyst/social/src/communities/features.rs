//! DAPPS decisions from the shared flags mirror. Source: social-service-ea
//! b6b2cef, adapters/feature-flags.ts and @dcl/features-component 1.0.2.
use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::jobs::{Jobs, PeriodicCfg};

pub const AI_COMPLIANCE: &str = "communities_ai_compliance";
pub const DEV_AI_COMPLIANCE: &str = "dev_communities_ai_compliance";
pub const GLOBAL_MODERATORS: &str = "communities_global_moderators";
pub const REFERRAL_REGISTRATION_DISABLED: &str = "referral_registration_disabled";

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

//! The Explorer's rollout flags: the Unleash application `explorer` as Decentraland's
//! feature-flags service publishes it (`/explorer.json` under [`Service::FeatureFlags`]). Another
//! client may read the same file, and a flag both list under one name is one switch for both.
//!
//! - [`FLAGS`] lists the flags this client reads. Only those are kept; every other key in the
//!   file is ignored.
//! - The service resolves like every other backend ([`base_domain::service`]): its own launch
//!   option, a gateway or a node's mirror, else `feature-flags.<base domain>`.
//! - `X-Address-Hash` carries the installation id (`AppConfig::user_id`, the `deviceIdentifier`
//!   comms sends) so gradual rollouts stick to the player. The sign-in address is never sent, and
//!   the id goes only to a Decentraland host, the rule comms applies to `deviceIdentifier`.
//! - The service picks flags by the request's `Referer` hostname. A browser sends its page's;
//!   natively the request sends `Referer: https://<base domain>/`, again only to a Decentraland
//!   host.
//! - The file is read once per session, without holding anything up. A flag is off until the
//!   answer says `true`, and stays off if the file can't be read.
//! - The launch option `ff` (`--ff` / `?ff=`) overrides the answer, for testing and for running a
//!   reported session again: a comma list of `name` (forced on), `name:variant` (forced on with
//!   that variant) or `-name` (forced off); `-*` ignores the service entirely (nothing is
//!   fetched, every flag not forced on is off). `-*,bevy-test` runs with only that flag on. Only
//!   names in [`FLAGS`] and short plain variants are kept, so a link cannot grow what analytics
//!   records. A link can set it: a flag hides or shows a feature, it is never a permission.
//! - Once the read settles, [`FeatureFlags::report`] and [`FeatureFlags::overrides_report`] go to
//!   analytics (`Feature Flags Resolved`) and into the crash report.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use serde::{Deserialize, Serialize};

use crate::{
    base_domain::{self, Service},
    structs::AppConfig,
    util::{reqwest_client, TaskCompat, TaskExt},
};

/// Every flag this client reads, by its Unleash name without `explorer-`.
///
/// A name another client also lists is one switch for both; `bevy-<name>` is the convention for
/// a flag only this client reads. To add one, create `explorer-<name>` in Unleash
/// and add it here. Unleash cannot rename a flag, so choose its name once. `bevy-test` has no
/// feature behind it, for checking the read end to end. `alfa-official-wallets` carries the wallets
/// whose names show the official badge (the avatar crate's nametags).
pub const FLAGS: &[&str] = &[
    "bevy-test",
    "alfa-official-wallets",
    "bevy-left-drag-camera",
];

/// The application's prefix on every key of the file.
const APP_PREFIX: &str = "explorer-";

const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// A variant name that survives a `?ff=` list, a url query and an analytics field unchanged.
fn is_safe_variant(variant: &str) -> bool {
    (1..=64).contains(&variant.len())
        && variant
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Where the session's flags came from.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagsSource {
    /// The read has not settled; every flag is off.
    #[default]
    Pending,
    Service,
    /// The read failed.
    Unavailable,
    /// `ff` has `-*`: nothing is read.
    Ignored,
}

impl FlagsSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Service => "service",
            Self::Unavailable => "unavailable",
            Self::Ignored => "ignored",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FlagPayload {
    /// Unleash's payload type: `string`, `json`, `csv` or `number`.
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FlagVariant {
    pub name: String,
    pub enabled: bool,
    pub payload: Option<FlagPayload>,
}

/// The `ff` overrides.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
struct Overrides {
    /// Flags forced on, each with the variant it is forced to.
    on: BTreeMap<String, Option<String>>,
    off: BTreeSet<String>,
    /// `-*`: the service is not asked, and every flag not forced on is off.
    ignore_all: bool,
}

/// The `ff` overrides in a comma list: a name's last mention wins. Only names in `known` are kept
/// (lowercase), and a variant only when it is safe.
fn parse_overrides(raw: &str, known: &[&str]) -> Overrides {
    let mut overrides = Overrides::default();
    for item in raw.split(',').map(str::trim) {
        if item == "-*" {
            overrides.ignore_all = true;
        } else if let Some(name) = item.strip_prefix('-') {
            let name = name.to_lowercase();
            if known.contains(&name.as_str()) {
                overrides.on.remove(&name);
                overrides.off.insert(name);
            }
        } else {
            let (name, variant) = item.split_once(':').unwrap_or((item, ""));
            let name = name.to_lowercase();
            if known.contains(&name.as_str()) {
                overrides.off.remove(&name);
                overrides
                    .on
                    .insert(name, is_safe_variant(variant).then(|| variant.to_owned()));
            }
        }
    }
    overrides
}

type Answer = (HashMap<String, bool>, HashMap<String, FlagVariant>);

/// This client's part of the service's file: the flags in `known`, each on only where the file
/// says `true`, each variant by its flag's name. Malformed input gives empty maps.
fn parse_file(json: &serde_json::Value, known: &[&str]) -> Answer {
    let own = |key: &str| {
        let key = key.to_lowercase();
        let name = key.strip_prefix(APP_PREFIX)?;
        known.contains(&name).then(|| name.to_owned())
    };
    let mut flags = HashMap::new();
    if let Some(file) = json.get("flags").and_then(|v| v.as_object()) {
        for (key, value) in file {
            if let Some(name) = own(key) {
                flags.insert(name, value.as_bool() == Some(true));
            }
        }
    }
    let mut variants = HashMap::new();
    if let Some(file) = json.get("variants").and_then(|v| v.as_object()) {
        for (key, raw) in file {
            let (Some(name), Some(variant)) = (own(key), raw.as_object()) else {
                continue;
            };
            let Some(variant_name) = variant.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            let payload = variant.get("payload").and_then(|p| {
                Some(FlagPayload {
                    kind: p.get("type")?.as_str()?.to_owned(),
                    value: p.get("value")?.as_str()?.to_owned(),
                })
            });
            variants.insert(
                name,
                FlagVariant {
                    name: variant_name.to_owned(),
                    enabled: variant.get("enabled").and_then(|v| v.as_bool()) == Some(true),
                    payload,
                },
            );
        }
    }
    (flags, variants)
}

/// The file's url: the feature-flags service as the resolver has it.
fn explorer_url() -> String {
    base_domain::url(Service::FeatureFlags, "/explorer.json")
}

/// What a native request sends as `Referer`, which a browser sets itself.
#[cfg(not(target_arch = "wasm32"))]
fn referer(domain: &str) -> String {
    format!("https://{domain}/")
}

/// The session's flags: the service's answer under the `ff` overrides.
#[derive(Resource, Default)]
pub struct FeatureFlags {
    source: FlagsSource,
    flags: HashMap<String, bool>,
    variants: HashMap<String, FlagVariant>,
    overrides: Overrides,
}

impl FeatureFlags {
    /// Flags as the `ff` list forces them, with no service answer: for apps and tests that run
    /// without the read.
    pub fn from_overrides(raw: &str) -> Self {
        Self {
            overrides: parse_overrides(raw, FLAGS),
            ..Default::default()
        }
    }

    pub fn source(&self) -> FlagsSource {
        self.source
    }

    /// A flag as `ff` forces it, or None when the service decides.
    fn forced(&self, name: &str) -> Option<bool> {
        if self.overrides.on.contains_key(name) {
            Some(true)
        } else if self.overrides.off.contains(name) || self.overrides.ignore_all {
            Some(false)
        } else {
            None
        }
    }

    /// Whether a flag is on: as `ff` forces it, else as answered; off until the answer says `true`.
    pub fn is_on(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        self.forced(&name)
            .unwrap_or_else(|| self.flags.get(&name) == Some(&true))
    }

    /// A flag's variant: the one `ff` forces, else as answered; None without one or when `ff`
    /// forces the flag off. A forced variant has the answer's payload only when the answer is that
    /// same variant. Unleash names a disabled variant `disabled`.
    pub fn variant(&self, name: &str) -> Option<FlagVariant> {
        let name = name.to_lowercase();
        if self.forced(&name) == Some(false) {
            return None;
        }
        let answered = self.variants.get(&name);
        match self.overrides.on.get(&name).and_then(|v| v.as_deref()) {
            None => answered.cloned(),
            Some(forced) => Some(FlagVariant {
                name: forced.to_owned(),
                enabled: true,
                payload: answered
                    .filter(|a| a.name == forced)
                    .and_then(|a| a.payload.clone()),
            }),
        }
    }

    /// A variant's payload, only when the variant is the named one and enabled.
    pub fn payload(&self, name: &str, variant: &str) -> Option<FlagPayload> {
        self.variant(name)
            .filter(|v| v.enabled && v.name == variant)
            .and_then(|v| v.payload)
    }

    /// The flags that are on, as a `ff` value that reproduces them whatever the service answers
    /// later: `-*`, then each flag on, sorted, with its variant when one is enabled.
    pub fn report(&self) -> String {
        let mut on: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        if !self.overrides.ignore_all {
            for (name, _) in self.flags.iter().filter(|(_, enabled)| **enabled) {
                if self.overrides.off.contains(name) {
                    continue;
                }
                let variant = self.variants.get(name).filter(|v| v.enabled);
                on.insert(name, variant.map(|v| v.name.as_str()));
            }
        }
        for (name, variant) in &self.overrides.on {
            let kept = on.get(name.as_str()).copied().flatten();
            on.insert(name, variant.as_deref().or(kept));
        }
        std::iter::once("-*".to_owned())
            .chain(on.into_iter().map(|(name, variant)| item(name, variant)))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The launch option's own overrides as a `ff` value, empty without any.
    pub fn overrides_report(&self) -> String {
        let o = &self.overrides;
        o.ignore_all
            .then(|| "-*".to_owned())
            .into_iter()
            .chain(
                o.on.iter()
                    .map(|(name, variant)| item(name, variant.as_deref())),
            )
            .chain(o.off.iter().map(|name| format!("-{name}")))
            .collect::<Vec<_>>()
            .join(",")
    }
}

// A variant whose name would not come back the same from a pasted `ff` is left out.
fn item(name: &str, variant: Option<&str>) -> String {
    match variant.filter(|v| is_safe_variant(v)) {
        Some(variant) => format!("{name}:{variant}"),
        None => name.to_owned(),
    }
}

/// Reads the service once at startup. `overrides` is the launch option `ff`.
#[derive(Default)]
pub struct FeatureFlagsPlugin {
    pub overrides: Option<String>,
}

impl Plugin for FeatureFlagsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FeatureFlags {
            overrides: parse_overrides(self.overrides.as_deref().unwrap_or(""), FLAGS),
            ..Default::default()
        })
        .add_systems(Update, read_flags.run_if(read_pending));
    }
}

fn read_pending(flags: Res<FeatureFlags>) -> bool {
    flags.source == FlagsSource::Pending
}

/// Runs while the read is pending: first starts it (or settles on `-*`), then waits for it.
fn read_flags(
    mut flags: ResMut<FeatureFlags>,
    config: Res<AppConfig>,
    mut task: Local<Option<Task<Result<serde_json::Value, String>>>>,
    mut started: Local<bool>,
) {
    if !*started {
        *started = true;
        if flags.overrides.ignore_all {
            flags.source = FlagsSource::Ignored;
            return;
        }
        *task = Some(IoTaskPool::get().spawn_compat(fetch(explorer_url(), config.user_id.clone())));
        return;
    }
    let Some(result) = task.as_mut().and_then(|t| t.complete()) else {
        return;
    };
    *task = None;
    match result {
        Ok(file) => {
            (flags.flags, flags.variants) = parse_file(&file, FLAGS);
            flags.source = FlagsSource::Service;
        }
        Err(e) => {
            warn!("feature flags read failed; every flag is off this session: {e}");
            flags.source = FlagsSource::Unavailable;
        }
    }
}

/// One read of the file. `X-Address-Hash` is what Unleash buckets a gradual rollout by; it and
/// the native `Referer` go only where [`base_domain::device_identifier_domain`] allows.
async fn fetch(url: String, machine_id: String) -> Result<serde_json::Value, String> {
    let mut request = reqwest_client()
        .get(&url)
        .header("X-Debug", "false")
        .timeout(READ_TIMEOUT);
    if let Some(domain) = base_domain::device_identifier_domain(&url) {
        request = request.header("X-Address-Hash", machine_id);
        #[cfg(not(target_arch = "wasm32"))]
        {
            request = request.header("Referer", referer(domain));
        }
        #[cfg(target_arch = "wasm32")]
        let _ = domain;
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const KNOWN: &[&str] = &["alfa-a", "bevy-test"];

    fn flags(raw: &str, file: serde_json::Value) -> FeatureFlags {
        let (answered, variants) = parse_file(&file, KNOWN);
        FeatureFlags {
            source: FlagsSource::Service,
            flags: answered,
            variants,
            overrides: parse_overrides(raw, KNOWN),
        }
    }

    #[test]
    fn overrides_keep_known_names_and_safe_variants() {
        let o = parse_overrides("Bevy-Test:beta, other, -unknown, alfa-a:bad variant", KNOWN);
        assert_eq!(o.on["bevy-test"].as_deref(), Some("beta"));
        assert_eq!(o.on["alfa-a"], None);
        assert_eq!(o.on.len(), 2);
        assert!(o.off.is_empty() && !o.ignore_all);
        assert!(parse_overrides("bevy-test:a/b", KNOWN).on["bevy-test"].is_none());
        assert!(parse_overrides("bevy-test:", KNOWN).on["bevy-test"].is_none());
        let long = format!("bevy-test:{}", "a".repeat(65));
        assert!(parse_overrides(&long, KNOWN).on["bevy-test"].is_none());
    }

    #[test]
    fn overrides_last_mention_wins_and_star_ignores() {
        let o = parse_overrides("bevy-test,-bevy-test", KNOWN);
        assert!(o.on.is_empty() && o.off.contains("bevy-test"));
        let o = parse_overrides("-bevy-test,bevy-test:v", KNOWN);
        assert!(o.off.is_empty() && o.on["bevy-test"].as_deref() == Some("v"));
        assert!(parse_overrides("-*,bevy-test", KNOWN).ignore_all);
        assert_eq!(parse_overrides("", KNOWN), Overrides::default());
    }

    #[test]
    fn file_keeps_only_listed_explorer_keys() {
        let (flags, variants) = parse_file(
            &json!({
                "flags": {
                    "explorer-bevy-test": true,
                    "Explorer-Alfa-A": "true",
                    "explorer-other": true,
                    "bevy-test": true,
                },
                "variants": {
                    "EXPLORER-BEVY-TEST": {
                        "name": "v", "enabled": true,
                        "payload": { "type": "json", "value": "{}" }
                    },
                    "explorer-alfa-a": { "enabled": true },
                    "explorer-other": { "name": "x", "enabled": true },
                }
            }),
            KNOWN,
        );
        assert_eq!(flags.len(), 2);
        assert!(flags["bevy-test"]);
        assert!(!flags["alfa-a"]);
        assert_eq!(variants.len(), 1);
        assert_eq!(variants["bevy-test"].name, "v");
        assert_eq!(variants["bevy-test"].payload.as_ref().unwrap().kind, "json");
    }

    #[test]
    fn malformed_files_give_nothing() {
        for file in [
            json!(null),
            json!([1]),
            json!("x"),
            json!({ "flags": 3, "variants": [] }),
            json!({ "flags": { "explorer-bevy-test": null }, "variants": { "explorer-bevy-test": 1 } }),
        ] {
            let (flags, variants) = parse_file(&file, KNOWN);
            assert!(flags.values().all(|on| !on));
            assert!(variants.is_empty());
        }
    }

    #[test]
    fn overrides_win_over_the_answer() {
        let file = json!({ "flags": { "explorer-bevy-test": true, "explorer-alfa-a": false } });
        let f = flags("", file.clone());
        assert!(f.is_on("bevy-test") && f.is_on("BEVY-TEST") && !f.is_on("alfa-a"));
        assert!(!f.is_on("unlisted"));
        let f = flags("-bevy-test,alfa-a", file.clone());
        assert!(!f.is_on("bevy-test") && f.is_on("alfa-a"));
        let f = flags("-*,alfa-a", file);
        assert!(!f.is_on("bevy-test") && f.is_on("alfa-a"));
        assert!(!FeatureFlags::default().is_on("bevy-test"));
    }

    #[test]
    fn forced_variant_takes_the_payload_only_from_the_same_variant() {
        let file = json!({
            "flags": { "explorer-bevy-test": true },
            "variants": { "explorer-bevy-test": {
                "name": "a", "enabled": true, "payload": { "type": "string", "value": "p" }
            } }
        });
        let f = flags("", file.clone());
        assert_eq!(f.variant("bevy-test").unwrap().name, "a");
        assert!(f.payload("bevy-test", "a").is_some());
        assert!(f.payload("bevy-test", "b").is_none());
        let f = flags("bevy-test:a", file.clone());
        assert!(f.variant("bevy-test").unwrap().payload.is_some());
        let f = flags("bevy-test:b", file.clone());
        let v = f.variant("bevy-test").unwrap();
        assert!(v.name == "b" && v.enabled && v.payload.is_none());
        let f = flags("-bevy-test", file);
        assert!(f.variant("bevy-test").is_none());
        assert!(flags("alfa-a:x", json!({})).variant("alfa-a").is_some());
        assert!(flags("", json!({})).variant("alfa-a").is_none());
    }

    #[test]
    fn report_is_sorted_and_reproduces_the_flags() {
        let f = flags(
            "",
            json!({
                "flags": { "explorer-bevy-test": true, "explorer-alfa-a": true },
                "variants": { "explorer-bevy-test": { "name": "v", "enabled": true } }
            }),
        );
        assert_eq!(f.report(), "-*,alfa-a,bevy-test:v");
        assert_eq!(f.overrides_report(), "");
        let again = FeatureFlags {
            source: FlagsSource::Ignored,
            overrides: parse_overrides(&f.report(), KNOWN),
            ..Default::default()
        };
        for name in KNOWN {
            assert_eq!(again.is_on(name), f.is_on(name));
        }
        assert_eq!(again.variant("bevy-test").unwrap().name, "v");

        let f = flags(
            "bevy-test:w,-alfa-a",
            json!({ "flags": { "explorer-alfa-a": true } }),
        );
        assert_eq!(f.report(), "-*,bevy-test:w");
        assert_eq!(f.overrides_report(), "bevy-test:w,-alfa-a");
        let f = flags("-*", json!({}));
        assert_eq!(
            (f.report().as_str(), f.overrides_report().as_str()),
            ("-*", "-*")
        );
        // an answered variant that is not enabled, or not safe, is left out
        let f = flags(
            "",
            json!({
                "flags": { "explorer-bevy-test": true, "explorer-alfa-a": true },
                "variants": {
                    "explorer-bevy-test": { "name": "bad name", "enabled": true },
                    "explorer-alfa-a": { "name": "off", "enabled": false }
                }
            }),
        );
        assert_eq!(f.report(), "-*,alfa-a,bevy-test");
    }

    #[test]
    fn the_service_resolves_like_every_other_backend() {
        // the default composition (no test in this crate sets a base domain or a service)
        assert_eq!(
            explorer_url(),
            "https://feature-flags.decentraland.org/explorer.json"
        );
        assert_eq!(
            base_domain::device_identifier_domain(&explorer_url()),
            Some("decentraland.org")
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_native_referer_names_the_resolved_deployment() {
        assert_eq!(referer("decentraland.org"), "https://decentraland.org/");
        assert_eq!(referer("decentraland.zone"), "https://decentraland.zone/");
    }
}

//! The feature-flags document vocabulary every area reads: a document's
//! shape, a held snapshot, this node's overrides
//! (`explore_api.feature_flags_local`) and one fetch from the flags service.
//! Explore mirrors and serves the documents.

use std::sync::Arc;

use anyhow::Context as _;
use parking_lot::RwLock;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::js::is_js_whitespace;

/// The largest flags document read.
pub const BODY_CAP: usize = 4 << 20;

/// A flags document: an object with `flags` (name -> bool) and `variants` objects.
pub fn validate(doc: &Value) -> anyhow::Result<()> {
    let flags = doc
        .get("flags")
        .and_then(Value::as_object)
        .context("document has no `flags` object")?;
    if let Some((name, value)) = flags.iter().find(|(_, v)| !v.is_boolean()) {
        anyhow::bail!("flag {name} is not a boolean: {value}");
    }
    doc.get("variants")
        .and_then(Value::as_object)
        .context("document has no `variants` object")?;
    Ok(())
}

/// The held document and whether it came from the flags service (fetched or
/// stored) rather than a bundled or empty placeholder, swapped together.
pub struct Snapshot {
    held: RwLock<(Arc<Value>, bool)>,
}

impl Snapshot {
    /// An application without a bundled document starts with every flag off.
    /// Explorer's bundled defaults must never enable another application's flags.
    pub fn empty() -> Self {
        Self {
            held: RwLock::new((Arc::new(json!({"flags": {}, "variants": {}})), false)),
        }
    }

    /// A snapshot holding `doc`; `sourced` says whether it came from the flags service.
    pub fn from_document(doc: Value, sourced: bool) -> Self {
        Self {
            held: RwLock::new((Arc::new(doc), sourced)),
        }
    }

    /// Replaces the held document and whether it came from the flags service, together.
    pub fn replace_sourced(&self, doc: Value, sourced: bool) {
        *self.held.write() = (Arc::new(doc), sourced);
    }

    /// The held document and whether it came from the flags service, read together.
    pub fn held(&self) -> (Arc<Value>, bool) {
        self.held.read().clone()
    }

    /// False until a document from the flags service is held: readers with a local fallback use it.
    pub fn is_sourced(&self) -> bool {
        self.held.read().1
    }

    pub fn document(&self) -> Arc<Value> {
        self.held.read().0.clone()
    }

    /// `@dcl/features-component` 1.0.2 addresses flags by `<app>-<feature>`.
    pub fn enabled(&self, app: &str, feature: &str) -> bool {
        self.document()["flags"][format!("{app}-{feature}")]
            .as_bool()
            .unwrap_or(false)
    }

    /// The SDK exposes variants only while the document's flag is enabled.
    /// Its payload is a comma-separated string; CRLF/LF is stripped before
    /// splitting, and even empty list entries are retained by the adapter.
    pub fn variant_values(&self, app: &str, feature: &str) -> Option<Vec<String>> {
        variant_values_in(&self.document(), app, feature, false)
    }
}

/// [`Snapshot::variant_values`] of `doc`, also requiring the variant's own `enabled`
/// to be true, as `@dcl/features-component` callers that check `variant.enabled` do.
pub fn enabled_variant_values(doc: &Value, app: &str, feature: &str) -> Option<Vec<String>> {
    variant_values_in(doc, app, feature, true)
}

fn variant_values_in(
    doc: &Value,
    app: &str,
    feature: &str,
    variant_enabled: bool,
) -> Option<Vec<String>> {
    let key = format!("{app}-{feature}");
    if doc["flags"][&key].as_bool() != Some(true) {
        return None;
    }
    if variant_enabled && doc["variants"][&key]["enabled"].as_bool() != Some(true) {
        return None;
    }
    let value = doc["variants"][&key]["payload"]["value"].as_str()?;
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .replace("\r\n", "")
            .replace('\n', "")
            .split(',')
            .map(|value| {
                value
                    .to_lowercase()
                    .trim_matches(is_js_whitespace)
                    .to_string()
            })
            .collect(),
    )
}

/// `doc` with every `feature_flags_local` override of `app` applied to its `flags`.
pub async fn with_local_overrides(pool: &PgPool, app: &str, doc: Value) -> sqlx::Result<Value> {
    Ok(apply_local_overrides(
        doc,
        &local_overrides(pool, app).await?,
    ))
}

/// The `feature_flags_local` overrides of `app`, by name.
pub async fn local_overrides(pool: &PgPool, app: &str) -> sqlx::Result<Vec<(String, bool)>> {
    sqlx::query_as(
        "SELECT name, value FROM explore_api.feature_flags_local WHERE app = $1 ORDER BY name",
    )
    .bind(app)
    .fetch_all(pool)
    .await
}

/// `doc` with `overrides` applied to its `flags`.
pub fn apply_local_overrides(mut doc: Value, overrides: &[(String, bool)]) -> Value {
    if !overrides.is_empty() {
        let flags = doc
            .as_object_mut()
            .map(|o| o.entry("flags").or_insert_with(|| json!({})))
            .and_then(Value::as_object_mut);
        if let Some(flags) = flags {
            for (name, value) in overrides {
                flags.insert(name.clone(), Value::Bool(*value));
            }
        }
    }
    doc
}

pub async fn set_local_flag(pool: &PgPool, app: &str, name: &str, value: bool) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO explore_api.feature_flags_local (app, name, value)
         VALUES ($1, $2, $3)
         ON CONFLICT (app, name) DO UPDATE
         SET value = EXCLUDED.value, updated_at = timezone('UTC', now())",
    )
    .bind(app)
    .bind(name)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_local_flag(pool: &PgPool, app: &str, name: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM explore_api.feature_flags_local WHERE app = $1 AND name = $2")
        .bind(app)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

/// One application's document as the service answers it for `referer` (the service picks flags by Referer),
/// validated. `target` has already passed the caller's SSRF guard.
pub async fn fetch_document(
    http: &crate::http::Outbound,
    target: reqwest::Url,
    referer: &str,
    timeout: std::time::Duration,
) -> anyhow::Result<Value> {
    let response = http
        .get(target)
        .header("referer", referer)
        .timeout(timeout)
        .send()
        .await
        .context("fetching application feature flags")?;
    if !response.status().is_success() {
        anyhow::bail!("application feature flags answered {}", response.status());
    }
    let body = crate::http::read_body_capped(response, BODY_CAP)
        .await
        .context("reading application feature flags")?;
    let doc: Value = serde_json::from_slice(&body).context("feature flags are not JSON")?;
    validate(&doc)?;
    Ok(doc)
}

#[cfg(test)]
mod application_tests {
    use super::*;

    #[test]
    fn sdk_app_keys_and_enabled_variants_are_independent() {
        let snapshot = Snapshot::empty();
        assert!(!snapshot.enabled("dapps", "communities_ai_compliance"));
        snapshot.replace_sourced(
            json!({
                "flags": {
                    "communities_ai_compliance": true,
                    "explorer-communities_ai_compliance": true,
                    "dapps-communities_ai_compliance": false,
                    "dapps-communities_global_moderators": true
                },
                "variants": {"dapps-communities_global_moderators": {
                    "payload": {"value": " 0xAbC,\r\n0xDeF, ,"}
                }}
            }),
            false,
        );
        assert!(!snapshot.enabled("dapps", "communities_ai_compliance"));
        assert_eq!(
            snapshot.variant_values("dapps", "communities_global_moderators"),
            Some(vec!["0xabc".into(), "0xdef".into(), "".into(), "".into()])
        );
        assert!(
            enabled_variant_values(
                &snapshot.document(),
                "dapps",
                "communities_global_moderators"
            )
            .is_none(),
            "a variant without `enabled: true` lists nothing"
        );
        assert!(!snapshot.is_sourced());
        snapshot.replace_sourced((*snapshot.document()).clone(), true);
        assert!(snapshot.is_sourced());
        assert!(
            snapshot
                .variant_values("explorer", "communities_global_moderators")
                .is_none()
        );
        snapshot.replace_sourced(
            json!({"flags": {}, "variants": {
                "dapps-communities_global_moderators": {"payload": {"value": "0xabc"}}
            }}),
            false,
        );
        assert!(
            snapshot
                .variant_values("dapps", "communities_global_moderators")
                .is_none()
        );
    }
}

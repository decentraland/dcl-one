//! The project's texture setting, `.dcl-one/settings.json`:
//! `{ "optimizeTextures": bool, "keepSourceTextures": bool }`, both off when
//! the file or a key is missing. `start` and `deploy` set it with
//! `--[no-]optimize-textures` and `--[no-]keep-source-textures`; every later
//! run, and a deploy from the preview page, reads it back. Keys this module
//! does not own are kept when it writes.

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Scene-relative: inside `.dcl-one`, which is never published.
pub const SETTINGS_FILE: &str = ".dcl-one/settings.json";

const OPTIMIZE_KEY: &str = "optimizeTextures";
const KEEP_SOURCES_KEY: &str = "keepSourceTextures";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectSettings {
    /// The preview serves, and a deploy uploads, the optimized files.
    pub optimize_textures: bool,
    /// Every original image is still published, replaced or not.
    pub keep_source_textures: bool,
}

/// What the command line asked for; `None` keeps the stored value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub optimize_textures: Option<bool>,
    pub keep_source_textures: Option<bool>,
}

impl Overrides {
    /// `--x` / `--no-x` as one override; clap keeps the pair exclusive.
    pub fn from_flags(optimize: bool, no_optimize: bool, keep: bool, no_keep: bool) -> Self {
        let pick = |on: bool, off: bool| match (on, off) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        };
        Overrides {
            optimize_textures: pick(optimize, no_optimize),
            keep_source_textures: pick(keep, no_keep),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.optimize_textures.is_none() && self.keep_source_textures.is_none()
    }
}

fn path(root: &Path) -> PathBuf {
    root.join(SETTINGS_FILE)
}

fn read_object(root: &Path) -> Map<String, Value> {
    std::fs::read(path(root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// The stored setting; defaults (off) for a missing or unreadable file.
pub fn load(root: &Path) -> ProjectSettings {
    let map = read_object(root);
    let flag = |key: &str| map.get(key).and_then(Value::as_bool).unwrap_or(false);
    ProjectSettings {
        optimize_textures: flag(OPTIMIZE_KEY),
        keep_source_textures: flag(KEEP_SOURCES_KEY),
    }
}

/// Applies `overrides` to the stored setting, writes it back when it
/// changed, and returns the result.
pub fn apply(root: &Path, overrides: Overrides) -> Result<ProjectSettings> {
    let stored = load(root);
    let mut settings = stored;
    if let Some(on) = overrides.optimize_textures {
        settings.optimize_textures = on;
    }
    if let Some(on) = overrides.keep_source_textures {
        settings.keep_source_textures = on;
    }
    if settings != stored || (!overrides.is_empty() && !path(root).is_file()) {
        save(root, settings)?;
    }
    Ok(settings)
}

fn save(root: &Path, settings: ProjectSettings) -> Result<()> {
    crate::scene::work_dir(root).context("creating .dcl-one")?;
    let mut map = read_object(root);
    map.insert(OPTIMIZE_KEY.into(), Value::Bool(settings.optimize_textures));
    map.insert(
        KEEP_SOURCES_KEY.into(),
        Value::Bool(settings.keep_source_textures),
    );
    let mut text = serde_json::to_vec_pretty(&Value::Object(map)).expect("JSON values serialize");
    text.push(b'\n');
    super::write_if_changed(&path(root), &text)
        .with_context(|| format!("writing {}", path(root).display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::TempTree;

    #[test]
    fn missing_file_means_off() {
        let t = TempTree::new("texture-settings-missing");
        assert_eq!(load(&t.0), ProjectSettings::default());
        assert_eq!(apply(&t.0, Overrides::default()).unwrap(), ProjectSettings::default());
        assert!(!t.0.join(SETTINGS_FILE).exists(), "no flag, nothing written");
    }

    #[test]
    fn flags_persist_until_changed() {
        let t = TempTree::new("texture-settings-persist");
        let on = Overrides::from_flags(true, false, true, false);
        let stored = apply(&t.0, on).unwrap();
        assert!(stored.optimize_textures && stored.keep_source_textures);
        // a later run without flags reads it back
        assert_eq!(apply(&t.0, Overrides::default()).unwrap(), stored);
        assert_eq!(load(&t.0), stored);
        // --no-keep-source-textures turns one off and keeps the other
        let off = Overrides::from_flags(false, false, false, true);
        let stored = apply(&t.0, off).unwrap();
        assert!(stored.optimize_textures && !stored.keep_source_textures);
        let text = std::fs::read_to_string(t.0.join(SETTINGS_FILE)).unwrap();
        let json: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["optimizeTextures"], Value::Bool(true));
        assert_eq!(json["keepSourceTextures"], Value::Bool(false));
        assert!(t.0.join(".dcl-one/.gitignore").is_file());
    }

    #[test]
    fn other_keys_survive_and_garbage_reads_as_off() {
        let t = TempTree::new("texture-settings-keys");
        t.write(SETTINGS_FILE, "{\"somethingElse\": 3}");
        apply(&t.0, Overrides::from_flags(true, false, false, false)).unwrap();
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(t.0.join(SETTINGS_FILE)).unwrap())
                .unwrap();
        assert_eq!(json["somethingElse"], Value::from(3));
        assert_eq!(json["optimizeTextures"], Value::Bool(true));

        t.write(SETTINGS_FILE, "not json");
        assert_eq!(load(&t.0), ProjectSettings::default());
    }

    #[test]
    fn flag_pairs() {
        assert_eq!(
            Overrides::from_flags(false, true, false, false),
            Overrides {
                optimize_textures: Some(false),
                keep_source_textures: None
            }
        );
        assert!(Overrides::from_flags(false, false, false, false).is_empty());
    }
}

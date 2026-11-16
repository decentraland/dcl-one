//! Spring-bone parameters a wearable declares in its entity metadata, under
//! `data.springBones`: one map of bone name to parameters for each model file, keyed by that
//! file's content hash.

use std::{collections::HashMap, sync::Arc};

use bevy::{log::warn, math::Vec3};
use serde::{Deserialize, Deserializer};

/// The only layout version this reader was written against. Another version is still read.
const SUPPORTED_VERSION: i64 = 1;

/// How one bone swings. A field the metadata leaves out is zero.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpringBoneParams {
    /// pull back towards the rest direction
    pub stiffness: f32,
    /// share of the tail's velocity lost each step, 0 to 1
    pub drag: f32,
    /// world-space direction of the constant force, as authored: not normalised
    #[serde(deserialize_with = "gravity_dir")]
    pub gravity_dir: Vec3,
    pub gravity_power: f32,
}

/// Parameters by bone node name, for one model file. Names match exactly, case included.
pub type SpringBoneModel = HashMap<String, SpringBoneParams>;

#[derive(Deserialize, Default)]
#[serde(default)]
struct SpringBonesMeta {
    version: i64,
    models: Option<HashMap<String, SpringBoneModel>>,
}

/// A direction written as `{x, y, z}` or `[x, y, z]`; a missing part, or `null`, is zero.
fn gravity_dir<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec3, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Dir {
        Parts(Vec<f32>),
        Named {
            #[serde(default)]
            x: f32,
            #[serde(default)]
            y: f32,
            #[serde(default)]
            z: f32,
        },
    }

    Ok(match Option::<Dir>::deserialize(deserializer)? {
        None => Vec3::ZERO,
        Some(Dir::Named { x, y, z }) => Vec3::new(x, y, z),
        Some(Dir::Parts(parts)) => {
            let part = |ix: usize| parts.get(ix).copied().unwrap_or(0.0);
            Vec3::new(part(0), part(1), part(2))
        }
    })
}

/// Reads `data.springBones` into one shared map per model content hash. A wearable without the
/// block, or with one that does not parse, has no entries: its bones then take the defaults.
pub fn spring_bone_models(
    spring_bones: Option<&serde_json::Value>,
    wearable: &str,
) -> HashMap<String, Arc<SpringBoneModel>> {
    let Some(value) = spring_bones.filter(|value| !value.is_null()) else {
        return HashMap::new();
    };

    let meta = match SpringBonesMeta::deserialize(value) {
        Ok(meta) => meta,
        Err(error) => {
            warn!("ignoring the spring bones of wearable {wearable}: {error}");
            return HashMap::new();
        }
    };

    let Some(models) = meta.models else {
        return HashMap::new();
    };

    if meta.version != SUPPORTED_VERSION {
        warn!(
            "wearable {wearable} has spring bones version {}, read as version {SUPPORTED_VERSION}",
            meta.version
        );
    }

    models
        .into_iter()
        .map(|(hash, model)| (hash, Arc::new(model)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(value: serde_json::Value) -> SpringBoneModel {
        let models = spring_bone_models(Some(&value), "test");
        models
            .get("hash")
            .map(|model| (**model).clone())
            .unwrap_or_default()
    }

    #[test]
    fn reads_every_parameter_of_a_bone() {
        let model = model(json!({ "version": 1, "models": { "hash": { "Hair_01": {
            "stiffness": 0.7, "drag": 0.2, "gravityDir": { "x": 0, "y": -1, "z": 0 },
            "gravityPower": 0.3, "hitRadius": 0.1, "isRoot": true
        } } } }));

        assert_eq!(
            model["Hair_01"],
            SpringBoneParams {
                stiffness: 0.7,
                drag: 0.2,
                gravity_dir: Vec3::NEG_Y,
                gravity_power: 0.3,
            }
        );
    }

    #[test]
    fn gravity_direction_reads_as_object_or_array_with_missing_parts_zero() {
        let model = model(json!({ "version": 1, "models": { "hash": {
            "object": { "gravityDir": { "y": -2 } },
            "array": { "gravityDir": [1, 2, 3] },
            "short_array": { "gravityDir": [4] },
            "null": { "gravityDir": null },
        } } }));

        assert_eq!(model["object"].gravity_dir, Vec3::new(0.0, -2.0, 0.0));
        assert_eq!(model["array"].gravity_dir, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(model["short_array"].gravity_dir, Vec3::new(4.0, 0.0, 0.0));
        assert_eq!(model["null"].gravity_dir, Vec3::ZERO);
    }

    #[test]
    fn a_field_left_out_is_zero() {
        let model = model(json!({ "version": 1, "models": { "hash": {
            "Tail": { "stiffness": 1 }
        } } }));

        assert_eq!(
            model["Tail"],
            SpringBoneParams {
                stiffness: 1.0,
                ..Default::default()
            }
        );
    }

    #[test]
    fn another_version_is_still_read() {
        let model = model(json!({ "version": 2, "models": { "hash": {
            "Tail": { "drag": 0.5 }
        } } }));

        assert_eq!(model["Tail"].drag, 0.5);
    }

    #[test]
    fn each_model_file_keeps_its_own_bones() {
        let models = spring_bone_models(
            Some(&json!({ "version": 1, "models": {
                "male": { "Cape": { "drag": 0.1 } },
                "female": { "Cape": { "drag": 0.9 } },
            } })),
            "test",
        );

        assert_eq!(models["male"]["Cape"].drag, 0.1);
        assert_eq!(models["female"]["Cape"].drag, 0.9);
    }

    #[test]
    fn a_missing_or_malformed_block_has_no_entries() {
        for value in [
            json!(null),
            json!({}),
            json!({ "version": 1 }),
            json!({ "version": 1, "models": null }),
            json!({ "version": 1, "models": [] }),
            json!({ "version": 1, "models": { "hash": { "Tail": { "drag": "high" } } } }),
            json!("spring"),
        ] {
            assert!(
                spring_bone_models(Some(&value), "test").is_empty(),
                "{value}"
            );
        }
        assert!(spring_bone_models(None, "test").is_empty());
    }
}

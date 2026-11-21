//! Spring-bone parameters a wearable declares in its entity metadata, under
//! `data.springBones`: one map of bone name to parameters for each model file, keyed by that
//! file's content hash.

use std::{collections::HashMap, sync::Arc};

use bevy::{log::warn, math::Vec3};
use ipfs::ContentMap;
use serde::{Deserialize, Deserializer};

/// The only layout version this reader was written against. Another version is still read.
const SUPPORTED_VERSION: f64 = 1.0;

/// Bound on `stiffness`, `gravityPower` and each part of `gravityDir`, far above any authored
/// value, so their products stay finite.
pub const MAX_FORCE: f32 = 1000.0;

/// How one bone swings. A field the metadata leaves out is zero. Every value is finite and within
/// the range its field names.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpringBoneParams {
    /// pull back towards the rest direction, 0 to [`MAX_FORCE`]
    pub stiffness: f32,
    /// share of the tail's velocity lost each step, 0 to 1
    pub drag: f32,
    /// world-space direction of the constant force, as authored: not normalised, each part
    /// within [`MAX_FORCE`] of zero
    #[serde(deserialize_with = "gravity_dir")]
    pub gravity_dir: Vec3,
    /// within [`MAX_FORCE`] of zero
    pub gravity_power: f32,
}

/// Parameters by bone node name, for one model file. Names match exactly, case included.
pub type SpringBoneModel = HashMap<String, SpringBoneParams>;

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

impl SpringBoneParams {
    /// The parameters with every value in the range the simulation may rely on. JSON holds no
    /// NaN; a number too large for `f32`, such as `1e39`, reads as infinity and is clamped like
    /// any other value out of range.
    fn sanitised(self) -> Self {
        Self {
            stiffness: self.stiffness.clamp(0.0, MAX_FORCE),
            drag: self.drag.clamp(0.0, 1.0),
            gravity_dir: self
                .gravity_dir
                .clamp(Vec3::splat(-MAX_FORCE), Vec3::splat(MAX_FORCE)),
            gravity_power: self.gravity_power.clamp(-MAX_FORCE, MAX_FORCE),
        }
    }
}

/// Reads `data.springBones` into one shared map per model content hash. Wearable metadata is
/// untrusted: a bone that does not parse is left out, as if it were not declared, and the bones
/// beside it are kept. Values out of range are clamped.
pub fn spring_bone_models(
    spring_bones: Option<&serde_json::Value>,
    wearable: &str,
) -> HashMap<String, Arc<SpringBoneModel>> {
    let Some(value) = spring_bones.filter(|value| !value.is_null()) else {
        return HashMap::new();
    };

    let models = match value.get("models") {
        Some(serde_json::Value::Object(models)) => models,
        // a block that declares no models
        None | Some(serde_json::Value::Null) if value.is_object() => return HashMap::new(),
        _ => {
            warn!("ignoring the spring bones of wearable {wearable}: no map of models");
            return HashMap::new();
        }
    };

    let version = value.get("version");
    if !is_supported_version(version) {
        warn!(
            "wearable {wearable} has spring bones version {}, read as version {SUPPORTED_VERSION}",
            version.unwrap_or(&serde_json::Value::Null)
        );
    }

    models
        .iter()
        .filter_map(|(hash, bones)| {
            let Some(bones) = bones.as_object() else {
                warn!("ignoring the spring bones of model {hash} of wearable {wearable}: not a map of bones");
                return None;
            };

            let model: SpringBoneModel = bones
                .iter()
                .filter_map(|(bone, params)| {
                    match SpringBoneParams::deserialize(params) {
                        Ok(params) => Some((bone.clone(), params.sanitised())),
                        Err(error) => {
                            warn!("ignoring spring bone {bone} of wearable {wearable}: {error}");
                            None
                        }
                    }
                })
                .collect();

            Some((hash.clone(), Arc::new(model)))
        })
        .collect()
}

/// The parameters declared for the model file `main_file`, found under that file's content hash.
/// The file name matches as every content lookup does, whatever its case.
pub fn model_spring_bones(
    models: &HashMap<String, Arc<SpringBoneModel>>,
    content: &ContentMap,
    main_file: &str,
) -> Option<Arc<SpringBoneModel>> {
    let hash = content.hash(main_file)?;
    models.get(hash.as_ref()).cloned()
}

/// `1` in any JSON number form (`1`, `1.0`, `1e0`), as the catalyst accepts it.
fn is_supported_version(version: Option<&serde_json::Value>) -> bool {
    version.and_then(serde_json::Value::as_f64) == Some(SUPPORTED_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_one_is_supported_in_every_json_number_form() {
        for version in ["1", "1.0", "1e0"] {
            let value: serde_json::Value = serde_json::from_str(version).unwrap();
            assert!(is_supported_version(Some(&value)), "{version}");
        }
        for version in ["2", "1.5", "\"1\""] {
            let value: serde_json::Value = serde_json::from_str(version).unwrap();
            assert!(!is_supported_version(Some(&value)), "{version}");
        }
        assert!(!is_supported_version(None));
    }

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
            json!({ "version": 1, "models": { "hash": [] } }),
            json!("spring"),
        ] {
            assert!(
                spring_bone_models(Some(&value), "test").is_empty(),
                "{value}"
            );
        }
        assert!(spring_bone_models(None, "test").is_empty());
    }

    #[test]
    fn a_malformed_bone_or_model_is_left_out_alone() {
        let models = spring_bone_models(
            Some(&json!({ "version": 1, "models": {
                "hash": {
                    "Good": { "drag": 0.5 },
                    "Text": { "drag": "high" },
                    "NotABone": 3,
                    "BadDirPart": { "gravityDir": [0, "down", 0] },
                    "BadDirName": { "gravityDir": { "y": "down" } },
                },
                "broken": "bones",
                "other": { "Good": { "drag": 0.25 } },
            } })),
            "test",
        );

        assert_eq!(models.len(), 2);
        assert_eq!(models["hash"].len(), 1);
        assert_eq!(models["hash"]["Good"].drag, 0.5);
        assert_eq!(models["other"]["Good"].drag, 0.25);
    }

    #[test]
    fn a_value_too_large_for_f32_is_clamped() {
        let model = model(json!({ "version": 1, "models": { "hash": {
            "Stiffness": { "stiffness": 1e39 },
            "Drag": { "drag": -1e39 },
            "Power": { "gravityPower": 1e39 },
            "DirPart": { "gravityDir": [0, 1e39, 0] },
            "DirName": { "gravityDir": { "z": -1e39 } },
        } } }));

        assert_eq!(model.len(), 5, "{model:?}");
        assert_eq!(model["Stiffness"].stiffness, MAX_FORCE);
        assert_eq!(model["Drag"].drag, 0.0);
        assert_eq!(model["Power"].gravity_power, MAX_FORCE);
        assert_eq!(model["DirPart"].gravity_dir, Vec3::new(0.0, MAX_FORCE, 0.0));
        assert_eq!(
            model["DirName"].gravity_dir,
            Vec3::new(0.0, 0.0, -MAX_FORCE)
        );
    }

    #[test]
    fn values_out_of_range_are_clamped() {
        let model = model(json!({ "version": 1, "models": { "hash": {
            "High": {
                "stiffness": 1e9, "drag": 7, "gravityDir": [1e9, -1e9, 0.5], "gravityPower": 1e9
            },
            "Low": { "stiffness": -2, "drag": -0.5, "gravityPower": -1e9 },
        } } }));

        assert_eq!(
            model["High"],
            SpringBoneParams {
                stiffness: MAX_FORCE,
                drag: 1.0,
                gravity_dir: Vec3::new(MAX_FORCE, -MAX_FORCE, 0.5),
                gravity_power: MAX_FORCE,
            }
        );
        assert_eq!(
            model["Low"],
            SpringBoneParams {
                stiffness: 0.0,
                drag: 0.0,
                gravity_dir: Vec3::ZERO,
                gravity_power: -MAX_FORCE,
            }
        );
    }

    #[test]
    fn a_version_that_is_not_an_integer_is_still_read() {
        for version in [json!("1"), json!(1.5), json!(null), json!([1])] {
            let model = model(json!({ "version": version, "models": { "hash": {
                "Tail": { "drag": 0.5 }
            } } }));

            assert_eq!(model["Tail"].drag, 0.5, "{version}");
        }
    }

    #[test]
    fn a_model_file_finds_its_bones_by_content_hash() {
        let models = spring_bone_models(
            Some(&json!({ "version": 1, "models": {
                "hash-male": { "Cape": { "drag": 0.1 } },
                "hash-female": { "Cape": { "drag": 0.9 } },
            } })),
            "test",
        );
        let content = ContentMap::from_content([
            ("male/Cape.glb".to_owned(), "hash-male".to_owned()),
            ("female/Cape.glb".to_owned(), "hash-female".to_owned()),
            ("Plain.glb".to_owned(), "hash-plain".to_owned()),
        ]);
        let drag = |main_file: &str| {
            model_spring_bones(&models, &content, main_file).map(|model| model["Cape"].drag)
        };

        // each body shape's representation names its own file
        assert_eq!(drag("male/Cape.glb"), Some(0.1));
        assert_eq!(drag("female/Cape.glb"), Some(0.9));
        // the name matches whatever its case
        assert_eq!(drag("MALE/cape.GLB"), Some(0.1));
        // a file with no `models` entry, and a file the entity does not hold
        assert_eq!(drag("Plain.glb"), None);
        assert_eq!(drag("missing.glb"), None);
    }
}

//! A smart wearable's scene: the SDK7 scene a wearable's content carries for a body shape
//! (`male/scene.json`, `female/scene.json`, or a bare `scene.json`).
//!
//! A wearable's collection gets one alias per body shape, `{shape}.smart_wearable`, naming that
//! shape's scene.json. Loading the alias as an [`EntityDefinition`] runs
//! [`SmartWearableSceneLoader`], which builds the scene's definition from the scene.json and the
//! wearable's content and registers the scene's own collection, so the scene then loads like any
//! other portable.

use std::sync::Arc;

use anyhow::anyhow;
use bevy::{
    asset::{AssetLoader, LoadContext},
    prelude::*,
};
use common::structs::SceneMeta;
use ipfs::{ipfs_path::IpfsPath, ContentMap, EntityDefinition, IpfsIo, IpfsModifier, IpfsResource};

pub const BODY_SHAPES: [&str; 2] = ["male", "female"];

/// The extension of the per-shape alias.
pub const SMART_WEARABLE_EXTENSION: &str = "smart_wearable";

/// The content file that loads a wearable's scene for `shape` (`male` or `female`).
pub fn scene_file(shape: &str) -> String {
    format!("{shape}.{SMART_WEARABLE_EXTENSION}")
}

/// The scene id of a wearable's scene for `shape`: the key in `PortableScenes` and the id of the
/// loaded definition.
pub fn scene_id(collection: &str, shape: &str) -> String {
    format!("{collection}-sw-{shape}")
}

/// The content key of the scene.json for `shape`: the body shape's own, else a bare one.
pub fn scene_json_key<'a>(
    files: impl IntoIterator<Item = &'a String>,
    shape: &str,
) -> Option<String> {
    let own = format!("{shape}/scene.json");
    let files: Vec<&String> = files.into_iter().collect();
    files
        .iter()
        .find(|f| f.eq_ignore_ascii_case(&own))
        .or_else(|| files.iter().find(|f| f.eq_ignore_ascii_case("scene.json")))
        .map(|f| (*f).clone())
}

/// `content` with the `{shape}.smart_wearable` alias of each body shape that has a scene.json.
pub fn with_scene_aliases(content: ContentMap) -> ContentMap {
    BODY_SHAPES.iter().fold(content, |content, shape| {
        let hash = scene_json_key(content.files(), shape)
            .and_then(|key| content.hash(&key).map(|hash| hash.into_owned()));
        match hash {
            Some(hash) => content.with(scene_file(shape), hash),
            None => content,
        }
    })
}

/// The scene's definition: an SDK7 scene whose `main` exists once the body shape's folder is the
/// root.
pub fn scene_definition(
    collection: &str,
    shape: &str,
    scene_json: &[u8],
    content: &ContentMap,
) -> Result<EntityDefinition, anyhow::Error> {
    let metadata: serde_json::Value =
        serde_json::from_slice(scene_json).map_err(|e| anyhow!("invalid scene.json: {e}"))?;
    let meta: SceneMeta =
        serde_json::from_value(metadata.clone()).map_err(|e| anyhow!("invalid scene.json: {e}"))?;
    if meta.runtime_version.as_deref() != Some("7") {
        return Err(anyhow!(
            "runtimeVersion {} is not 7",
            meta.runtime_version.as_deref().unwrap_or("(none)")
        ));
    }
    let others: Vec<String> = BODY_SHAPES
        .iter()
        .filter(|other| **other != shape)
        .map(|other| format!("{other}/"))
        .collect();
    let others: Vec<&str> = others.iter().map(String::as_str).collect();
    let content = content.strip_prefix(&format!("{shape}/"), &others);
    if content.hash(&meta.main).is_none() {
        return Err(anyhow!("main file {} is missing", meta.main));
    }
    Ok(EntityDefinition {
        id: scene_id(collection, shape),
        pointers: Vec::new(),
        content,
        metadata: Some(metadata),
    })
}

/// Loads `{shape}.smart_wearable` of a wearable's collection (the scene.json) as the scene's
/// definition, and registers the scene's collection under its id with the wearable's base url.
pub struct SmartWearableSceneLoader {
    ipfs: Arc<IpfsIo>,
}

impl FromWorld for SmartWearableSceneLoader {
    fn from_world(world: &mut World) -> Self {
        Self {
            ipfs: world.resource::<IpfsResource>().inner.clone(),
        }
    }
}

impl AssetLoader for SmartWearableSceneLoader {
    type Asset = EntityDefinition;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<EntityDefinition, anyhow::Error> {
        let path = IpfsPath::new_from_path(load_context.path())?
            .ok_or_else(|| anyhow!("not an ipfs path: {:?}", load_context.path()))?;
        let collection = path
            .scene_hash()
            .ok_or_else(|| anyhow!("no collection in {:?}", load_context.path()))?
            .to_owned();
        let shape = path
            .content_path()
            .and_then(|file| file.strip_suffix(&format!(".{SMART_WEARABLE_EXTENSION}")))
            .filter(|shape| BODY_SHAPES.contains(shape))
            .ok_or_else(|| anyhow!("not a smart wearable scene: {:?}", load_context.path()))?
            .to_owned();

        let mut scene_json = Vec::new();
        reader.read_to_end(&mut scene_json).await?;
        let (wearable, base_url) = self
            .ipfs
            .entity_definition(&collection)
            .await
            .ok_or_else(|| anyhow!("wearable collection {collection} is not registered"))?;
        let definition = scene_definition(&collection, &shape, &scene_json, &wearable.collection)?;

        self.ipfs
            .add_collection_async(
                definition.id.clone(),
                definition.content.clone(),
                Some(IpfsModifier {
                    base_url: Some(base_url),
                }),
                Some(String::from_utf8_lossy(&scene_json).into_owned()),
            )
            .await;
        Ok(definition)
    }

    fn extensions(&self) -> &[&str] {
        &[SMART_WEARABLE_EXTENSION]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(files: &[&str]) -> ContentMap {
        ContentMap::from_content(
            files
                .iter()
                .map(|file| (file.to_string(), format!("h-{file}"))),
        )
    }

    const SCENE_JSON: &str =
        r#"{"main":"bin/index.js","runtimeVersion":"7","scene":{"base":"0,0","parcels":["0,0"]}}"#;

    #[test]
    fn each_shape_aliases_its_own_scene_json_else_the_bare_one() {
        let aliased = with_scene_aliases(content(&["male/scene.json", "scene.json", "x.glb"]));
        assert_eq!(
            aliased.hash("male.smart_wearable").as_deref(),
            Some("h-male/scene.json")
        );
        assert_eq!(
            aliased.hash("female.smart_wearable").as_deref(),
            Some("h-scene.json")
        );

        let plain = with_scene_aliases(content(&["male/model.glb"]));
        assert_eq!(plain.hash("male.smart_wearable").as_deref(), None);
        assert_eq!(plain.hash("female.smart_wearable").as_deref(), None);
    }

    #[test]
    fn the_definition_roots_the_shape_folder_and_drops_the_other_shape() {
        let wearable = content(&[
            "male/scene.json",
            "male/bin/index.js",
            "female/bin/index.js",
            "female/only.png",
            "shared.png",
        ]);
        let definition =
            scene_definition("hash", "male", SCENE_JSON.as_bytes(), &wearable).unwrap();
        assert_eq!(definition.id, "hash-sw-male");
        assert_eq!(
            definition.content.hash("bin/index.js").as_deref(),
            Some("h-male/bin/index.js")
        );
        assert_eq!(
            definition.content.hash("shared.png").as_deref(),
            Some("h-shared.png")
        );
        assert_eq!(definition.content.hash("only.png").as_deref(), None);
        assert_eq!(definition.content.hash("female/only.png").as_deref(), None);
        assert!(definition.metadata.is_some());
    }

    #[test]
    fn a_bad_scene_json_fails_the_definition() {
        let wearable = content(&["male/bin/index.js"]);
        let fails = |json: &str| scene_definition("hash", "male", json.as_bytes(), &wearable);
        assert!(fails("not json").is_err());
        assert!(fails(&SCENE_JSON.replace(r#""7""#, r#""6""#)).is_err());
        assert!(fails(&SCENE_JSON.replace("bin/index.js", "bin/missing.js")).is_err());
        // the other shape's main does not count
        let female_only = content(&["female/bin/index.js"]);
        assert!(scene_definition("hash", "male", SCENE_JSON.as_bytes(), &female_only).is_err());
    }
}

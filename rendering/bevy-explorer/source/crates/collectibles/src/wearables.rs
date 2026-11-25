use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use crate::{
    spring_bones::{model_spring_bones, spring_bone_models, SpringBoneModel},
    urn::{CollectibleInstance, CollectibleUrn},
    Collectible, CollectibleData, CollectibleType, Collectibles, CollectiblesTypePlugin,
};
use anyhow::anyhow;
use bevy::{
    asset::{io::AssetSourceId, AssetEvents, AssetLoader, AssetPath, RenderAssetTransferPriority},
    diagnostic::FrameCount,
    gltf::{Gltf, GltfLoaderSettings},
    image::ImageLoaderSettings,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_asset::RenderAssetUsages,
    tasks::{IoTaskPool, Task},
};
use serde::Deserialize;

use common::util::{TaskCompat, TaskExt};
use ipfs::{ipfs_path::ContentPathExt, EntityDefinitionLoader, IpfsAssetServer};

pub struct WearablePlugin;

impl Plugin for WearablePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CollectiblesTypePlugin::<Wearable>::default())
            .init_resource::<WearableCollections>();
        app.register_asset_loader(WearableLoader);
        app.register_asset_loader(WearableMetaLoader);
        app.add_systems(Update, (load_collections, retain_wearables));
        app.add_systems(PostUpdate, clean_images.before(AssetEvents));
    }

    fn finish(&self, app: &mut App) {
        // reads the ipfs resource, which exists once every plugin is built
        app.init_asset_loader::<crate::smart_wearables::SmartWearableSceneLoader>();
    }
}

#[derive(Resource, Default)]
pub struct WearableCollections(pub HashMap<String, String>);

pub type WearableUrn = CollectibleUrn<Wearable>;
pub type WearableInstance = CollectibleInstance<Wearable>;

#[derive(Deserialize, Debug, Component, Clone)]
pub struct WearableMeta {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub thumbnail: String,
    pub rarity: Option<String>,
    pub data: WearableData,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WearableData {
    pub tags: Vec<String>,
    pub category: WearableCategory,
    pub representations: Vec<WearableRepresentation>,
    pub hides: Vec<WearableCategory>,
    pub replaces: Vec<WearableCategory>,
    pub removes_default_hiding: Option<Vec<String>>,
    pub outline_compatible: Option<bool>,
    /// Read by [`spring_bone_models`], which tolerates a block that does not parse.
    pub spring_bones: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WearableRepresentation {
    pub body_shapes: Vec<String>,
    pub main_file: String,
    pub override_replaces: Vec<WearableCategory>,
    pub override_hides: Vec<WearableCategory>,
    pub contents: Vec<String>,
}

#[derive(Deserialize)]
struct Collection {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Collections {
    collections: Vec<Collection>,
}

fn load_collections(
    mut once: Local<bool>,
    mut collections: ResMut<WearableCollections>,
    mut task: Local<Option<Task<Result<Collections, anyhow::Error>>>>,
    ipfs: IpfsAssetServer,
) {
    if *once {
        return;
    }

    match *task {
        None => {
            let client = ipfs.ipfs().client();
            let t: Task<Result<Collections, anyhow::Error>> =
                IoTaskPool::get().spawn_compat(async move {
                    let response = client
                        .get(common::base_domain::url(
                            common::base_domain::Service::Catalyst,
                            "/lambdas/collections",
                        ))
                        .timeout(std::time::Duration::from_secs(10))
                        .send()
                        .await
                        .map_err(|e| anyhow!(e))?;
                    response.json::<Collections>().await.map_err(|e| anyhow!(e))
                });
            *task = Some(t)
        }
        Some(ref mut active_task) => match active_task.complete() {
            None => (),
            Some(Err(e)) => {
                warn!("failed to acquire collections: {e}");
                *task = None;
                *once = true;
            }
            Some(Ok(collections_result)) => {
                collections.0 = HashMap::from_iter(
                    collections_result
                        .collections
                        .into_iter()
                        .map(|c| (c.id, c.name)),
                );
                *task = None;
                *once = true;
            }
        },
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub struct WearableCategory {
    pub slot: &'static str,
    pub is_texture: bool,
}

impl<'de> serde::Deserialize<'de> for WearableCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(WearableCategory::from_str(s.as_str()).unwrap_or(WearableCategory::UNKNOWN))
    }
}

impl WearableCategory {
    pub const UNKNOWN: WearableCategory = WearableCategory::texture("unknown");

    pub const EYES: WearableCategory = WearableCategory::texture("eyes");
    pub const EYEBROWS: WearableCategory = WearableCategory::texture("eyebrows");
    pub const MOUTH: WearableCategory = WearableCategory::texture("mouth");

    pub const FACIAL_HAIR: WearableCategory = WearableCategory::model("facial_hair");
    pub const HAIR: WearableCategory = WearableCategory::model("hair");
    pub const HAND_WEAR: WearableCategory = WearableCategory::model("hands_wear");
    pub const BODY_SHAPE: WearableCategory = WearableCategory::model("body_shape");
    pub const UPPER_BODY: WearableCategory = WearableCategory::model("upper_body");
    pub const LOWER_BODY: WearableCategory = WearableCategory::model("lower_body");
    pub const FEET: WearableCategory = WearableCategory::model("feet");
    pub const EARRING: WearableCategory = WearableCategory::model("earring");
    pub const EYEWEAR: WearableCategory = WearableCategory::model("eyewear");
    pub const HAT: WearableCategory = WearableCategory::model("hat");
    pub const HELMET: WearableCategory = WearableCategory::model("helmet");
    pub const MASK: WearableCategory = WearableCategory::model("mask");
    pub const TIARA: WearableCategory = WearableCategory::model("tiara");
    pub const TOP_HEAD: WearableCategory = WearableCategory::model("top_head");
    pub const SKIN: WearableCategory = WearableCategory::model("skin");

    // only used for hiding
    pub const HEAD: WearableCategory = WearableCategory::model("head");
    pub const HANDS: WearableCategory = WearableCategory::model("hands");

    const fn model(slot: &'static str) -> Self {
        Self {
            slot,
            is_texture: false,
        }
    }

    const fn texture(slot: &'static str) -> Self {
        Self {
            slot,
            is_texture: true,
        }
    }
}

impl FromStr for WearableCategory {
    type Err = anyhow::Error;

    fn from_str(slot: &str) -> Result<WearableCategory, Self::Err> {
        match slot {
            "body_shape" => Ok(Self::BODY_SHAPE),

            "hair" => Ok(Self::HAIR),
            "eyebrows" => Ok(Self::EYEBROWS),
            "eyes" => Ok(Self::EYES),
            "mouth" => Ok(Self::MOUTH),
            "facial_hair" => Ok(Self::FACIAL_HAIR),

            "upper_body" => Ok(Self::UPPER_BODY),
            "hands_wear" => Ok(Self::HAND_WEAR),
            "lower_body" => Ok(Self::LOWER_BODY),
            "feet" => Ok(Self::FEET),

            "hat" => Ok(Self::HAT),
            "eyewear" => Ok(Self::EYEWEAR),
            "earring" => Ok(Self::EARRING),
            "mask" => Ok(Self::MASK),
            "top_head" => Ok(Self::TOP_HEAD),
            "tiara" => Ok(Self::TIARA),
            "helmet" => Ok(Self::HELMET),
            "skin" => Ok(Self::SKIN),

            "head" => Ok(Self::HEAD),
            "hands" => Ok(Self::HANDS), // legacy support

            _ => {
                warn!("unrecognised wearable category: {slot}");
                Err(anyhow::anyhow!("unrecognised wearable category: {slot}"))
            }
        }
    }
}

impl WearableCategory {
    // does not include hide-only categories
    pub fn iter() -> impl Iterator<Item = &'static Self> {
        [
            Self::BODY_SHAPE,
            Self::HAIR,
            Self::EYEBROWS,
            Self::EYES,
            Self::MOUTH,
            Self::FACIAL_HAIR,
            Self::UPPER_BODY,
            Self::HAND_WEAR,
            Self::LOWER_BODY,
            Self::FEET,
            Self::HAT,
            Self::EYEWEAR,
            Self::EARRING,
            Self::MASK,
            Self::TOP_HEAD,
            Self::TIARA,
            Self::HELMET,
            Self::SKIN,
        ]
        .iter()
    }

    // does not include hide-only categories
    pub fn hides_order() -> impl Iterator<Item = &'static Self> {
        [
            Self::SKIN,
            Self::UPPER_BODY,
            Self::HAND_WEAR,
            Self::LOWER_BODY,
            Self::FEET,
            Self::HELMET,
            Self::HAT,
            Self::TOP_HEAD,
            Self::MASK,
            Self::EYEWEAR,
            Self::EARRING,
            Self::TIARA,
            Self::HAIR,
            Self::EYEBROWS,
            Self::EYES,
            Self::MOUTH,
            Self::FACIAL_HAIR,
            Self::BODY_SHAPE,
        ]
        .iter()
    }

    pub fn index(&self) -> Option<usize> {
        Self::iter().position(|w| w == self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WearableModel {
    pub gltf: Handle<Gltf>,
    pub outline_compatible: bool,
    /// The spring-bone parameters the wearable declares for this model file, by bone name.
    pub spring_bones: Option<Arc<SpringBoneModel>>,
}

#[derive(Debug, TypePath, Clone, PartialEq)]
pub struct Wearable {
    pub category: WearableCategory,
    pub hides: HashSet<WearableCategory>,
    pub model: Option<WearableModel>,
    pub texture: Option<Handle<Image>>,
    pub mask: Option<Handle<Image>>,
}

#[derive(Clone, Debug)]
pub struct WearableExtraData {
    pub category: WearableCategory,
    /// The content files named `scene.json` (`male/scene.json`, `female/scene.json`, or a bare
    /// `scene.json`): a non-empty list marks a smart wearable candidate.
    pub scene_json_files: Vec<String>,
}

/// The content keys that make a wearable a smart wearable candidate.
pub fn scene_json_files(content: &ipfs::ContentMap) -> Vec<String> {
    let mut files: Vec<String> = content
        .files()
        .filter(|file| file.ends_with("scene.json"))
        .cloned()
        .collect();
    files.sort();
    files
}

impl CollectibleType for Wearable {
    type Meta = WearableMeta;
    type ExtraData = WearableExtraData;

    fn source_collections() -> &'static [&'static str] {
        &[]
    }

    fn extension() -> &'static str {
        "wearable"
    }

    fn data_extension() -> &'static str {
        "wearable_data"
    }

    fn with_content_aliases(content: ipfs::ContentMap) -> ipfs::ContentMap {
        crate::smart_wearables::with_scene_aliases(content)
    }
}

/// Loads the files a wearable's representations name, by asset path.
pub trait WearableFiles {
    fn gltf(&mut self, path: PathBuf) -> Handle<Gltf>;
    fn image(&mut self, path: PathBuf) -> Handle<Image>;
}

fn gltf_settings(s: &mut GltfLoaderSettings) {
    s.load_cameras = false;
    s.load_lights = false;
    s.load_meshes = RenderAssetUsages::MAIN_WORLD;
    s.load_materials = RenderAssetUsages::RENDER_WORLD;
    s.transfer_priority = RenderAssetTransferPriority::Priority(2);
}

fn image_settings(s: &mut ImageLoaderSettings) {
    s.asset_usage = RenderAssetUsages::RENDER_WORLD;
    s.transfer_priority = RenderAssetTransferPriority::Priority(2);
}

struct LoaderFiles<'a, 'b>(&'a mut bevy::asset::LoadContext<'b>);

impl WearableFiles for LoaderFiles<'_, '_> {
    fn gltf(&mut self, path: PathBuf) -> Handle<Gltf> {
        self.0.loader().with_settings(gltf_settings).load(path)
    }

    fn image(&mut self, path: PathBuf) -> Handle<Image> {
        self.0.loader().with_settings(image_settings).load(path)
    }
}

struct ServerFiles<'a>(&'a AssetServer);

impl WearableFiles for ServerFiles<'_> {
    fn gltf(&mut self, path: PathBuf) -> Handle<Gltf> {
        self.0.load_with_settings(path, gltf_settings)
    }

    fn image(&mut self, path: PathBuf) -> Handle<Image> {
        self.0.load_with_settings(path, image_settings)
    }
}

/// A wearable from its definition: one representation for each body shape a representation
/// names, keyed by the lowercase body shape urn, its files resolved under `parent` (the folder
/// of the collection's `collectible.wearable`) and loaded through `files`.
pub fn build_wearable(
    meta: WearableMeta,
    content: &ipfs::ContentMap,
    hash: String,
    urn: String,
    parent: &Path,
    files: &mut impl WearableFiles,
) -> Result<Collectible<Wearable>, anyhow::Error> {
    let category = meta.data.category;
    let thumbnail = parent
        .resolve_content_uri(&meta.thumbnail)
        .to_string_lossy()
        .into_owned();

    let spring_bones = spring_bone_models(meta.data.spring_bones.as_ref(), &meta.id);

    let mut representations = HashMap::new();

    for representation in meta.data.representations.into_iter() {
        let spring_bones = model_spring_bones(&spring_bones, content, &representation.main_file);

        let (model, texture, mask) = if category.is_texture {
            let texture = representation
                .contents
                .iter()
                .find(|f| {
                    f.to_lowercase().ends_with(".png") && !f.to_lowercase().ends_with("_mask.png")
                })
                .map(|f| files.image(parent.resolve_content_uri(f)));
            let mask = representation
                .contents
                .iter()
                .find(|f| f.to_lowercase().ends_with("_mask.png"))
                .map(|f| files.image(parent.resolve_content_uri(f)));

            (None, texture, mask)
        } else {
            if !representation.main_file.to_lowercase().ends_with(".glb")
                && !representation.main_file.to_lowercase().ends_with(".gltf")
            {
                return Err(anyhow!(
                    "expected .gl[b|tf] main file, found {}",
                    representation.main_file
                ));
            }

            let model = files.gltf(parent.resolve_content_uri(&representation.main_file));

            (Some(model), None, None)
        };

        for body_shape in representation.body_shapes {
            let mut hides: HashSet<_> = if representation.override_hides.is_empty() {
                &meta.data.hides
            } else {
                &representation.override_hides
            }
            .iter()
            .copied()
            .collect();

            hides.extend(
                if representation.override_replaces.is_empty() {
                    &meta.data.replaces
                } else {
                    &representation.override_replaces
                }
                .iter()
                .copied(),
            );

            if category == WearableCategory::SKIN {
                hides.extend([
                    WearableCategory::HEAD,
                    WearableCategory::HAIR,
                    WearableCategory::FACIAL_HAIR,
                    WearableCategory::MOUTH,
                    WearableCategory::EYEBROWS,
                    WearableCategory::EYES,
                    WearableCategory::UPPER_BODY,
                    WearableCategory::LOWER_BODY,
                    WearableCategory::FEET,
                    WearableCategory::HAND_WEAR,
                    WearableCategory::BODY_SHAPE,
                ]);
            }

            let keeps_hands = meta
                .data
                .removes_default_hiding
                .as_ref()
                .map(|removes| removes.iter())
                .unwrap_or_default()
                .any(|r| r == "hands");
            if (category == WearableCategory::UPPER_BODY
                || hides.contains(&WearableCategory::UPPER_BODY))
                && !keeps_hands
            {
                hides.insert(WearableCategory::HANDS);
            }

            hides.remove(&category);

            representations.insert(
                body_shape.to_lowercase(),
                Wearable {
                    category,
                    hides,
                    model: model.clone().map(|gltf| WearableModel {
                        gltf,
                        outline_compatible: meta.data.outline_compatible.unwrap_or(true),
                        spring_bones: spring_bones.clone(),
                    }),
                    texture: texture.clone(),
                    mask: mask.clone(),
                },
            );
        }
    }

    Ok(Collectible {
        data: CollectibleData {
            thumbnail,
            hash,
            urn,
            name: meta.name.unwrap_or_default(),
            description: meta.description.unwrap_or_default(),
            available_representations: representations.keys().cloned().collect(),
            extra_data: WearableExtraData {
                category,
                scene_json_files: scene_json_files(content),
            },
        },
        representations,
    })
}

/// A wearable whose definition the engine already holds, its files read from the registered
/// collection `collection`: a creator preview's wearable, registered with
/// [`crate::CollectibleManager::add_builtin`] instead of being looked up on a catalyst.
pub fn inline_wearable(
    meta: WearableMeta,
    content: &ipfs::ContentMap,
    collection: &str,
    urn: String,
    assets: &AssetServer,
) -> Result<Collectible<Wearable>, anyhow::Error> {
    let definition = PathBuf::from(&ipfs::ipfs_path::IpfsPath::new(
        ipfs::ipfs_path::IpfsType::new_content_file(
            collection.to_owned(),
            format!("collectible.{}", Wearable::extension()),
        ),
    ));
    let parent = definition.parent().unwrap_or(Path::new(""));
    build_wearable(
        meta,
        content,
        collection.to_owned(),
        urn,
        parent,
        &mut ServerFiles(assets),
    )
}

struct WearableLoader;

impl AssetLoader for WearableLoader {
    type Asset = Collectible<Wearable>;

    type Settings = ();

    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut entity = EntityDefinitionLoader
            .load(reader, settings, load_context)
            .await?;
        let metadata = entity.metadata.ok_or(anyhow!("no metadata?"))?;
        let meta = serde_json::from_value::<WearableMeta>(metadata)?;
        let parent = load_context.path().parent().unwrap().to_path_buf();
        build_wearable(
            meta,
            &entity.content,
            entity.id,
            entity.pointers.pop().unwrap_or_default(),
            &parent,
            &mut LoaderFiles(load_context),
        )
    }
}

#[derive(Component)]
pub struct UsedWearables(pub HashSet<CollectibleUrn<Wearable>>);

fn retain_wearables(
    used: Query<&UsedWearables>,
    mut collectibles: ResMut<Collectibles<Wearable>>,
    frame: Res<FrameCount>,
) {
    let urns = used.iter().fold(
        HashSet::<&CollectibleUrn<Wearable>>::default(),
        |mut urns, used| {
            urns.extend(used.0.iter());
            urns
        },
    );

    collectibles.retain(frame.0, |urn| urns.contains(urn));
}

struct WearableMetaLoader;

impl AssetLoader for WearableMetaLoader {
    type Asset = CollectibleData<Wearable>;

    type Settings = ();

    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut entity = EntityDefinitionLoader
            .load(reader, settings, load_context)
            .await?;
        let scene_json_files = scene_json_files(&entity.content);
        let metadata = entity.metadata.ok_or(anyhow!("no metadata?"))?;
        let meta = serde_json::from_value::<WearableMeta>(metadata)?;

        let category = meta.data.category;
        let thumbnail = load_context
            .path()
            .parent()
            .unwrap()
            .resolve_content_uri(&meta.thumbnail)
            .to_string_lossy()
            .into_owned();

        let available_representations = meta
            .data
            .representations
            .into_iter()
            .flat_map(|rep| {
                rep.body_shapes
                    .into_iter()
                    .map(|shape| shape.to_lowercase())
            })
            .collect();

        Ok(CollectibleData {
            thumbnail,
            hash: entity.id,
            urn: entity.pointers.pop().unwrap_or_default(),
            name: meta.name.unwrap_or_default(),
            description: meta.description.unwrap_or_default(),
            available_representations,
            extra_data: WearableExtraData {
                category,
                scene_json_files: scene_json_files.clone(),
            },
        })
    }
}

fn clean_images(
    mut ev: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    mut retry: Local<Vec<AssetId<Image>>>,
    asset_server: Res<AssetServer>,
) {
    for id in retry.drain(..) {
        if asset_server
            .get_path(id)
            .as_ref()
            .is_some_and(screenshot_reference)
        {
            continue;
        }
        let Some(image) = images.get(id) else {
            continue;
        };
        if image.asset_usage.intersects(RenderAssetUsages::MAIN_WORLD) {
            let image = images.get_mut(id).unwrap();
            image.asset_usage = RenderAssetUsages::RENDER_WORLD;
        }
    }

    for ev in ev.read() {
        if let AssetEvent::LoadedWithDependencies { id } = ev {
            if asset_server
                .get_path(*id)
                .as_ref()
                .is_some_and(screenshot_reference)
            {
                continue;
            }
            let Some(image) = images.get(*id) else {
                // retry once next frame
                retry.push(*id);
                continue;
            };
            if image.asset_usage.intersects(RenderAssetUsages::MAIN_WORLD) {
                let image = images.get_mut(*id).unwrap();
                image.asset_usage = RenderAssetUsages::RENDER_WORLD;
            }
        }
    }
}

fn screenshot_reference(path: &AssetPath<'_>) -> bool {
    // These images are compared on the CPU long after loading. Dropping MAIN_WORLD
    // leaves metadata in Assets<Image>, but the renderer takes their pixel buffers.
    matches!(path.source(), AssetSourceId::Name(source) if source.as_ref() == "embedded")
        && path.path().starts_with("images/screenshots")
}

#[cfg(test)]
mod screenshot_reference_tests {
    use super::*;

    #[test]
    fn preserves_only_embedded_screenshot_references() {
        assert!(screenshot_reference(&AssetPath::parse(
            "embedded://images/screenshots/reference.png"
        )));
        for path in [
            "images/screenshots/reference.png",
            "embedded://images/screenshots-other/reference.png",
            "embedded://images/wearables/texture.png",
            "ipfs://images/screenshots/reference.png",
        ] {
            assert!(!screenshot_reference(&AssetPath::parse(path)), "{path}");
        }
    }
}

#[cfg(test)]
mod build_tests {
    use super::*;
    use ipfs::ContentMap;
    use std::path::{Path, PathBuf};

    const MALE: &str = "urn:decentraland:off-chain:base-avatars:BaseMale";
    const FEMALE: &str = "urn:decentraland:off-chain:base-avatars:BaseFemale";

    #[derive(Default)]
    struct Recorder(Vec<PathBuf>);

    impl WearableFiles for Recorder {
        fn gltf(&mut self, path: PathBuf) -> Handle<Gltf> {
            self.0.push(path);
            Handle::default()
        }

        fn image(&mut self, path: PathBuf) -> Handle<Image> {
            self.0.push(path);
            Handle::default()
        }
    }

    /// A wearable whose definition the engine already holds (a creator preview's) is built as a
    /// deployed one is: a representation for each body shape it names, each loading that body's
    /// model from the collection, with the hides its definition gives.
    #[test]
    fn a_held_definition_builds_the_wearable_for_each_body() {
        let representation = |body: &str, main: &str| {
            serde_json::json!({
                "bodyShapes": [body],
                "mainFile": main,
                "overrideHides": [],
                "overrideReplaces": [],
                "contents": [main]
            })
        };
        let meta: WearableMeta = serde_json::from_value(serde_json::json!({
            "id": "urn:6f1d3c2e-0000-4000-8000-000000000003",
            "name": "Preview Hat",
            "thumbnail": "",
            "data": {
                "tags": [],
                "category": "hat",
                "hides": ["hair"],
                "replaces": ["eyewear"],
                "representations": [
                    representation(MALE, "male/model.glb"),
                    representation(FEMALE, "female/model.glb")
                ]
            }
        }))
        .unwrap();
        let content = ContentMap::from_content([
            ("male/model.glb".to_owned(), "b64-m".to_owned()),
            ("female/model.glb".to_owned(), "b64-m".to_owned()),
            ("male/scene.json".to_owned(), "b64-s".to_owned()),
        ]);
        let parent = Path::new("$content_file/abc-preview");
        let mut files = Recorder::default();

        let collectible = build_wearable(
            meta,
            &content,
            "abc-preview".to_owned(),
            "urn:decentraland:off-chain:preview:abc".to_owned(),
            parent,
            &mut files,
        )
        .unwrap();

        let mut bodies: Vec<&String> = collectible.representations.keys().collect();
        bodies.sort();
        assert_eq!(
            bodies,
            [&FEMALE.to_lowercase(), &MALE.to_lowercase()],
            "keyed as the avatar asks, by lowercase body urn"
        );
        for wearable in collectible.representations.values() {
            assert_eq!(wearable.category, WearableCategory::HAT);
            assert_eq!(
                wearable.hides,
                HashSet::from_iter([WearableCategory::HAIR, WearableCategory::EYEWEAR])
            );
            assert!(wearable.model.is_some());
        }
        assert_eq!(
            files.0,
            vec![
                parent.resolve_content_uri("male/model.glb"),
                parent.resolve_content_uri("female/model.glb"),
            ]
        );
        assert_eq!(
            collectible.data.urn,
            "urn:decentraland:off-chain:preview:abc"
        );
        assert_eq!(collectible.data.hash, "abc-preview");
        assert_eq!(collectible.data.name, "Preview Hat");
        assert_eq!(
            collectible.data.extra_data.scene_json_files,
            vec!["male/scene.json".to_owned()]
        );
    }
}

pub mod base_wearables;
mod cache;
#[cfg(test)]
mod dynamic_lifecycle_tests;
pub mod emotes;
pub mod ext;
#[cfg(test)]
mod metadata_lifecycle_tests;
pub mod urn;
pub mod wearables;

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::*;

    /// Installs the real wearable cache schedules with already resolved remote pointers.
    /// Tests supply their own asset reader and loader, without catalogue HTTP requests.
    pub fn wearable_cache(
        app: &mut App,
        pointers: impl IntoIterator<Item = (wearables::WearableUrn, String)>,
    ) {
        app.add_plugins(CollectiblesTypePlugin::<wearables::Wearable>::default())
            .add_systems(Update, expire_idle_wearables);
        let mut cache = app
            .world_mut()
            .resource_mut::<Collectibles<wearables::Wearable>>();
        for (urn, hash) in pointers {
            cache.record_remote(urn, PointerResult::Hash(hash));
        }
    }
}

use std::{marker::PhantomData, path::PathBuf};

use bevy::{
    diagnostic::FrameCount,
    ecs::system::SystemParam,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

use common::memory_reclaim::{
    AddMemoryReclaimer, MemoryReclaimState, ReclaimBudget, ReclaimDomains, ReclaimSpec, ReclaimTurn,
};
use common::util::TaskExt;
pub use emotes::*;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    ActiveEntityTask, IpfsAssetServer, IpfsModifier,
};
use serde::Deserialize;
use urn::CollectibleUrn;
use wearables::WearablePlugin;

pub struct CollectiblesPlugin;

impl Plugin for CollectiblesPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((EmotesPlugin, WearablePlugin));
    }
}

pub struct CollectiblesTypePlugin<T: CollectibleType>(PhantomData<fn() -> T>);

impl<T: CollectibleType> Default for CollectiblesTypePlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: CollectibleType> Plugin for CollectiblesTypePlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<Collectibles<T>>()
            .init_asset::<Collectible<T>>()
            .init_asset::<CollectibleData<T>>()
            .add_systems(Update, (request_collectibles::<T>, prune_dynamic::<T>))
            .add_memory_reclaimer(
                ReclaimSpec::new(T::extension(), 10, ReclaimDomains::RamAndGpu),
                reclaim_representations::<T>,
            )
            .add_memory_reclaimer(
                ReclaimSpec::new(T::data_extension(), 20, ReclaimDomains::Ram),
                reclaim_metadata::<T>,
            );
    }
}

pub trait CollectibleType: std::fmt::Debug + TypePath + Send + Sync + 'static {
    type Meta: for<'a> Deserialize<'a>;
    type ExtraData: Clone + Send + Sync + 'static;
    /// The off-chain collections a bare name (`"sittingChair2"`) may come from. A bare name parses
    /// to a urn under `UNKNOWN_SOURCE_COLLECTION`; the manager requests the name under every
    /// source collection instead and aliases the unknown-source urn to whichever has it. Lookups
    /// by the unknown-source urn then work, and `CollectibleManager::source_urn` gives the real one.
    fn source_collections() -> &'static [&'static str];
    fn extension() -> &'static str;
    fn data_extension() -> &'static str;
    fn is_dynamic(_urn: &CollectibleUrn<Self>) -> bool
    where
        Self: Sized,
    {
        false
    }
}

pub enum PointerResult<T: CollectibleType> {
    Hash(String),
    Builtin(Handle<Collectible<T>>),
    Missing,
}

#[derive(Asset, TypePath)]
pub struct Collectible<T: CollectibleType> {
    pub representations: HashMap<String, T>,
    pub data: CollectibleData<T>,
}

#[derive(Debug, Asset, TypePath)]
pub struct CollectibleData<T: CollectibleType> {
    pub hash: String,
    pub urn: String,
    pub thumbnail: String,
    pub available_representations: HashSet<String>,
    pub name: String,
    pub description: String,
    pub extra_data: T::ExtraData,
}

impl<T: CollectibleType> Clone for CollectibleData<T> {
    fn clone(&self) -> Self {
        Self {
            hash: self.hash.clone(),
            urn: self.urn.clone(),
            thumbnail: self.thumbnail.clone(),
            available_representations: self.available_representations.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            extra_data: self.extra_data.clone(),
        }
    }
}

#[derive(Resource)]
pub struct Collectibles<T: CollectibleType> {
    pointers: HashMap<CollectibleUrn<T>, PointerResult<T>>,
    pointer_request: HashSet<CollectibleUrn<T>>,
    /// requested urn -> the `source_collections` urn it resolved to
    aliases: HashMap<CollectibleUrn<T>, CollectibleUrn<T>>,
    cache: cache::IdleCache<Collectible<T>>,
    data_cache: cache::IdleCache<CollectibleData<T>>,
    // Non-owning indexes; AssetEvent::Removed prunes them without a whole-map scan.
    dynamic: HashMap<CollectibleUrn<T>, AssetId<Collectible<T>>>,
    dynamic_urns: HashMap<AssetId<Collectible<T>>, CollectibleUrn<T>>,
}

impl<T: CollectibleType> Collectibles<T> {
    /// The urn to name a collectible by outside the manager (on the wire): the source-collection
    /// urn it resolved to, else the bare name it was requested by, else the urn as given.
    pub fn source_urn<'a>(&'a self, urn: &'a CollectibleUrn<T>) -> &'a str {
        match self.aliases.get(urn) {
            Some(source) => source.as_str(),
            None => urn.unknown_source_name().unwrap_or(urn.as_str()),
        }
    }

    fn record_remote(&mut self, urn: CollectibleUrn<T>, result: PointerResult<T>) {
        // A delayed response must not replace a local builtin or synthesized scene emote.
        if !T::is_dynamic(&urn)
            && !matches!(self.pointers.get(&urn), Some(PointerResult::Builtin(_)))
        {
            self.pointers.insert(urn, result);
        }
    }
}

impl<T: CollectibleType> Default for Collectibles<T> {
    fn default() -> Self {
        Self {
            pointers: Default::default(),
            pointer_request: Default::default(),
            aliases: Default::default(),
            cache: Default::default(),
            data_cache: Default::default(),
            dynamic: Default::default(),
            dynamic_urns: Default::default(),
        }
    }
}

#[derive(SystemParam)]
pub struct CollectibleManager<'w, 's, T: CollectibleType> {
    collectibles: ResMut<'w, Collectibles<T>>,
    assets: ResMut<'w, Assets<Collectible<T>>>,
    datas: ResMut<'w, Assets<CollectibleData<T>>>,
    ipfas: IpfsAssetServer<'w, 's>,
    frame: Res<'w, FrameCount>,
}

#[derive(Debug)]
pub enum CollectibleError {
    Loading,
    /// The remote pointer is absent, or a dynamic collectible has no local value.
    /// Dynamic values require synthesis through `CollectibleManager::add_dynamic`.
    Missing,
    Failed,
    NoRepresentation,
}

const RETAIN_TICKS: u32 = 3;

impl<T: CollectibleType> CollectibleManager<'_, '_, T> {
    pub fn get_hash(
        &mut self,
        urn: impl AsRef<CollectibleUrn<T>>,
    ) -> Result<String, CollectibleError> {
        if T::is_dynamic(urn.as_ref()) {
            return Err(CollectibleError::NoRepresentation);
        }
        match self.collectibles.pointers.get(urn.as_ref()) {
            Some(PointerResult::Missing) => Err(CollectibleError::Missing),
            Some(PointerResult::Hash(hash)) => Ok(hash.to_owned()),
            Some(PointerResult::Builtin(_)) => Err(CollectibleError::NoRepresentation),
            None => {
                self.collectibles
                    .pointer_request
                    .insert(urn.as_ref().clone());
                Err(CollectibleError::Loading)
            }
        }
    }

    fn representation_handle(
        &mut self,
        urn: &CollectibleUrn<T>,
    ) -> Result<Handle<Collectible<T>>, CollectibleError> {
        if T::is_dynamic(urn) {
            let id = *self
                .collectibles
                .dynamic
                .get(urn)
                .ok_or(CollectibleError::Missing)?;
            if !self.assets.contains(id) {
                return Err(CollectibleError::Missing);
            }
            return Ok(self
                .collectibles
                .cache
                .get_or_insert_with(urn.as_str(), self.frame.0, || {
                    self.assets
                        .get_strong_handle(id)
                        .expect("dynamic asset checked above")
                })
                .clone());
        }
        match self.collectibles.pointers.get(urn) {
            Some(PointerResult::Missing) => Err(CollectibleError::Missing),
            Some(PointerResult::Builtin(handle)) => Ok(handle.clone()),
            Some(PointerResult::Hash(hash)) => {
                let hash = hash.clone();
                Ok(self
                    .collectibles
                    .cache
                    .get_or_insert_with(&hash, self.frame.0, || {
                        let path = IpfsPath::new(IpfsType::new_content_file(
                            hash.clone(),
                            format!("collectible.{}", T::extension()),
                        ));
                        self.ipfas.asset_server().load(PathBuf::from(&path))
                    })
                    .clone())
            }
            None => {
                self.collectibles.pointer_request.insert(urn.clone());
                Err(CollectibleError::Loading)
            }
        }
    }

    pub fn get_representation(
        &mut self,
        urn: impl AsRef<CollectibleUrn<T>>,
        body_shape: &str,
    ) -> Result<&T, CollectibleError> {
        self.get_representation_with_handle(urn, body_shape)
            .map(|(representation, _)| representation)
    }

    /// Consumers such as animation graph nodes retain this handle for their entire lifetime.
    pub fn get_representation_with_handle(
        &mut self,
        urn: impl AsRef<CollectibleUrn<T>>,
        body_shape: &str,
    ) -> Result<(&T, Handle<Collectible<T>>), CollectibleError> {
        let handle = self.representation_handle(urn.as_ref())?;
        let Some(asset) = self.assets.get(handle.id()) else {
            return Err(
                if matches!(
                    self.ipfas.asset_server().load_state(handle.id()),
                    bevy::asset::LoadState::Failed(_)
                ) {
                    CollectibleError::Failed
                } else {
                    CollectibleError::Loading
                },
            );
        };
        let representation = asset
            .representations
            .get(&body_shape.to_lowercase())
            .ok_or(CollectibleError::NoRepresentation)?;
        Ok((representation, handle))
    }

    /// Dynamic collectibles must already be synthesized through [`Self::add_dynamic`].
    /// `Missing` for an unbuilt or reclaimed dynamic value does not start a remote load.
    pub fn get_data(
        &mut self,
        urn: impl AsRef<CollectibleUrn<T>>,
    ) -> Result<&CollectibleData<T>, CollectibleError> {
        let urn = urn.as_ref();
        match (T::is_dynamic(urn), self.collectibles.pointers.get(urn)) {
            (true, _) | (_, Some(PointerResult::Builtin(_))) => {
                let handle = self.representation_handle(urn)?;
                self.assets
                    .get(handle.id())
                    .map(|asset| &asset.data)
                    .ok_or(CollectibleError::Loading)
            }
            (_, Some(PointerResult::Missing)) => Err(CollectibleError::Missing),
            (_, Some(PointerResult::Hash(hash))) => {
                let hash = hash.clone();
                let handle =
                    self.collectibles
                        .data_cache
                        .get_or_insert_with(&hash, self.frame.0, || {
                            let path = IpfsPath::new(IpfsType::new_content_file(
                                hash.clone(),
                                format!("collectible.{}", T::data_extension()),
                            ));
                            self.ipfas.asset_server().load(PathBuf::from(&path))
                        });
                self.datas.get(handle.id()).ok_or_else(|| {
                    if matches!(
                        self.ipfas.asset_server().load_state(handle.id()),
                        bevy::asset::LoadState::Failed(_)
                    ) {
                        CollectibleError::Failed
                    } else {
                        CollectibleError::Loading
                    }
                })
            }
            (_, None) => {
                self.collectibles.pointer_request.insert(urn.clone());
                Err(CollectibleError::Loading)
            }
        }
    }

    /// As [`Collectibles::source_urn`].
    pub fn source_urn<'a>(&'a self, urn: &'a CollectibleUrn<T>) -> &'a str {
        self.collectibles.source_urn(urn)
    }

    pub fn add_builtin(&mut self, urn: CollectibleUrn<T>, value: Collectible<T>) {
        if let Some(PointerResult::Builtin(h)) = self.collectibles.pointers.get(&urn) {
            let asset = self.assets.get_mut(h).unwrap();
            asset.representations.extend(value.representations);
            asset
                .data
                .available_representations
                .extend(value.data.available_representations);
        } else {
            let handle = self.assets.add(value);
            self.collectibles
                .pointers
                .insert(urn, PointerResult::Builtin(handle));
        }
    }

    /// Synthesized scene content has ordinary idle pins, unlike permanent engine builtins.
    pub fn add_dynamic(&mut self, urn: CollectibleUrn<T>, value: Collectible<T>) {
        assert!(T::is_dynamic(&urn));
        let existing = self
            .collectibles
            .dynamic
            .get(&urn)
            .and_then(|id| self.assets.get_strong_handle(*id));
        let handle = if let Some(handle) = existing {
            let asset = self.assets.get_mut(&handle).unwrap();
            asset.representations.extend(value.representations);
            asset
                .data
                .available_representations
                .extend(value.data.available_representations);
            handle
        } else {
            self.assets.add(value)
        };
        self.collectibles.dynamic.insert(urn.clone(), handle.id());
        self.collectibles
            .dynamic_urns
            .insert(handle.id(), urn.clone());
        self.collectibles.pointer_request.remove(&urn);
        self.collectibles
            .cache
            .get_or_insert_with(urn.as_str(), self.frame.0, || handle);
    }
}

fn prune_dynamic<T: CollectibleType>(
    mut removed: EventReader<AssetEvent<Collectible<T>>>,
    mut cache: ResMut<Collectibles<T>>,
) {
    for event in removed.read() {
        if let AssetEvent::Removed { id } = event {
            if let Some(urn) = cache.dynamic_urns.remove(id) {
                if cache.dynamic.get(&urn) == Some(id) {
                    cache.dynamic.remove(&urn);
                }
            }
        }
    }
}

fn load_pending<A: Asset>(server: &AssetServer, id: AssetId<A>) -> bool {
    match server.load_state(id) {
        bevy::asset::LoadState::Loading => true,
        bevy::asset::LoadState::Loaded => matches!(
            server.recursive_dependency_load_state(id),
            bevy::asset::RecursiveDependencyLoadState::Loading
        ),
        _ => false,
    }
}

fn reclaim_representations<T: CollectibleType>(
    mut cache: ResMut<Collectibles<T>>,
    frame: Res<FrameCount>,
    server: Res<AssetServer>,
    mut budget: ResMut<ReclaimBudget>,
) {
    if let Some(mut turn) = budget.begin(T::extension(), cache.cache.len()) {
        cache
            .cache
            .reclaim(frame.0, &mut turn, |id| load_pending(&server, id));
        budget.finish_turn(turn, cache.cache.len());
    }
}

fn reclaim_metadata<T: CollectibleType>(
    mut cache: ResMut<Collectibles<T>>,
    frame: Res<FrameCount>,
    server: Res<AssetServer>,
    mut budget: ResMut<ReclaimBudget>,
) {
    if let Some(mut turn) = budget.begin(T::data_extension(), cache.data_cache.len()) {
        cache
            .data_cache
            .reclaim(frame.0, &mut turn, |id| load_pending(&server, id));
        budget.finish_turn(turn, cache.data_cache.len());
    }
}

fn expire_idle_wearables(
    mut cache: ResMut<Collectibles<wearables::Wearable>>,
    frame: Res<FrameCount>,
    server: Res<AssetServer>,
    pressure: Option<Res<MemoryReclaimState>>,
) {
    // Under pressure, all expiry shares the reclaimer's budget and rotation.
    if pressure.is_some_and(|pressure| pressure.ram_active || pressure.gpu_active) {
        return;
    }
    cache
        .cache
        .reclaim(frame.0, &mut ReclaimTurn::bounded(10, 64), |id| {
            load_pending(&server, id)
        });
}

pub fn request_collectibles<T: CollectibleType>(
    mut collectibles: ResMut<Collectibles<T>>,
    mut active_task: Local<Option<(ActiveEntityTask, HashSet<CollectibleUrn<T>>)>>,
    ipfas: IpfsAssetServer,
) {
    if let Some((mut task, mut requested_entities)) = active_task.take() {
        match task.complete() {
            Some(Ok(entities)) => {
                ipfas.ipfs().record_attachment_batch(entities.len());
                debug!("got results: {:?}", entities.len());

                for entity in entities {
                    let collection = entity
                        .content
                        .with(format!("collectible.{}", T::extension()), entity.id.clone())
                        .with(
                            format!("collectible.{}", T::data_extension()),
                            entity.id.clone(),
                        );

                    ipfas.ipfs().add_collection(
                        entity.id.clone(),
                        collection,
                        Some(IpfsModifier {
                            base_url: Some(base_wearables::content_url()),
                        }),
                        entity.metadata.as_ref().map(ToString::to_string),
                    );

                    let Some(metadata) = entity.metadata else {
                        warn!("no metadata on wearable");
                        continue;
                    };
                    debug!("loaded collectible {:?} -> {:?}", entity.pointers, metadata);
                    for pointer in entity.pointers.into_iter() {
                        let Ok(pointer) = CollectibleUrn::<T>::try_from(pointer.as_str()) else {
                            warn!("bad pointer: {}", pointer);
                            continue;
                        };

                        debug!("{} -> {}", pointer, entity.id);
                        requested_entities.remove(&pointer);
                        collectibles.record_remote(pointer, PointerResult::Hash(entity.id.clone()));
                    }
                }

                // any urns left in the hashset were requested but not returned
                for urn in &requested_entities {
                    debug!("missing {urn}");
                    collectibles.record_remote(urn.clone(), PointerResult::Missing);
                }

                // an unknown-source urn is never returned, so it always lands here: resolve it to
                // whichever source collection has the name
                for urn in requested_entities {
                    let found = source_urns::<T>(&urn).find_map(|source| {
                        match collectibles.pointers.get(&source) {
                            Some(PointerResult::Hash(hash)) => Some((source, hash.clone())),
                            _ => None,
                        }
                    });
                    if let Some((source, hash)) = found {
                        debug!("{urn} -> {source}");
                        collectibles.record_remote(urn.clone(), PointerResult::Hash(hash));
                        collectibles.aliases.insert(urn, source);
                    }
                }
            }
            Some(Err(e)) => {
                warn!("failed to resolve entities: {e}");
            }
            None => {
                debug!("waiting for collectible resolve");
                *active_task = Some((task, requested_entities));
            }
        }
    } else {
        let requested = std::mem::take(&mut collectibles.pointer_request);

        let mut requested = requested
            .into_iter()
            .filter(|r| !T::is_dynamic(r) && !collectibles.pointers.contains_key(r))
            .collect::<HashSet<_>>();

        // fetch an unknown-source name under every source collection, in the same round trip
        let sources = requested
            .iter()
            .flat_map(source_urns::<T>)
            .filter(|s| !collectibles.pointers.contains_key(s))
            .collect::<Vec<_>>();
        requested.extend(sources);

        // an unknown-source urn stays in the set (to be resolved from the results) but isn't a
        // pointer any server knows
        let pointers = requested
            .iter()
            .filter(|urn| urn.unknown_source_name().is_none())
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        if !pointers.is_empty() {
            debug!("requesting: {:?} ({:?})", requested, pointers);
            // ask the catalyst the entities' files are fetched from (`base_url` above), not the
            // realm's content server: a realm on another environment resolves the same pointer
            // to a hash that catalyst doesn't have
            *active_task = Some((
                ipfas.ipfs().active_entities(
                    ipfs::ActiveEntitiesRequest::Pointers(pointers),
                    Some(base_wearables::base_url().as_str()),
                ),
                requested,
            ));
        }
    }
}

/// The places an unknown-source name may be found: the name under each of the type's
/// `source_collections`. A urn under a real collection names exactly one place, so none.
fn source_urns<T: CollectibleType>(
    urn: &CollectibleUrn<T>,
) -> impl Iterator<Item = CollectibleUrn<T>> + '_ {
    urn.unknown_source_name().into_iter().flat_map(|name| {
        T::source_collections()
            .iter()
            .filter_map(move |collection| CollectibleUrn::new(&format!("{collection}:{name}")).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_remote_results_cannot_replace_builtin_emotes() {
        let mut collectibles = Collectibles::<Emote>::default();
        let builtin = EmoteUrn::new("wave").unwrap();
        collectibles
            .pointers
            .insert(builtin.clone(), PointerResult::Builtin(Handle::default()));
        collectibles.record_remote(builtin.clone(), PointerResult::Missing);
        assert!(matches!(
            collectibles.pointers.get(&builtin),
            Some(PointerResult::Builtin(_))
        ));
    }

    fn sources<T: CollectibleType>(urn: &str) -> Vec<String> {
        source_urns(&CollectibleUrn::<T>::new(urn).unwrap())
            .map(String::from)
            .collect()
    }

    #[test]
    fn bare_emote_names_are_tried_under_every_source_collection() {
        let bare = EmoteUrn::new("sittingChair2").unwrap();
        assert_eq!(
            bare.as_str(),
            "urn:decentraland:off-chain:unknown-source:sittingchair2"
        );
        assert_eq!(bare.unknown_source_name(), Some("sittingchair2"));
        assert_eq!(
            sources::<Emote>("sittingChair2"),
            [
                "urn:decentraland:off-chain:base-emotes:sittingchair2",
                "urn:decentraland:off-chain:base-scene-emotes:sittingchair2"
            ]
        );
        assert!(EmoteUrn::new("urn:decentraland:off-chain:base-emotes:wave")
            .unwrap()
            .unknown_source_name()
            .is_none());
        assert!(sources::<Emote>("urn:decentraland:off-chain:base-scene-emotes:punch").is_empty());
        assert!(sources::<Emote>("urn:decentraland:matic:collections-v2:0xabc:0").is_empty());
        assert!(sources::<wearables::Wearable>(
            "urn:decentraland:off-chain:base-avatars:f_eyes_00"
        )
        .is_empty());
    }
}

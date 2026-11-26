#![forbid(unsafe_code)]

#[cfg(feature = "explorer")]
use common::sync::MutexExt;
#[cfg(feature = "explorer")]
use std::path::PathBuf;
use std::{collections::VecDeque, f32::consts::PI, time::Duration};

use bevy::{
    animation::{AnimationTarget, AnimationTargetId},
    asset::RenderAssetUsages,
    gltf::Gltf,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{
        mesh::{
            skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
            MeshAabb, MeshTag,
        },
        primitives::Aabb,
        render_resource::Face,
        view::RenderLayers,
    },
    scene::InstanceId,
};
#[cfg(feature = "explorer")]
use bevy::{
    asset::AsyncReadExt,
    tasks::{IoTaskPool, Task},
};
#[cfg(feature = "explorer")]
use bevy_console::ConsoleCommand;
use collectibles::{
    base_wearables,
    wearables::{UsedWearables, Wearable, WearableCategory, WearableModel, WearableUrn},
    CollectibleError, CollectibleManager, Emote, EmoteUrn,
};
#[cfg(feature = "explorer")]
use console::DoAddConsoleCommand;
use hover_glow::{can_glow, set_hover_glows};
use scene_material::{
    SceneBound, SceneBounds, SceneMaterial, SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG,
    SCENE_MATERIAL_NO_DITHERING_MESH_TAG, SCENE_MATERIAL_OUTLINE_MESH_TAG,
    SCENE_MATERIAL_TOON_MESH_TAG,
};

pub mod animate;
#[cfg(feature = "explorer")]
pub mod attach;
#[cfg(feature = "explorer")]
mod avatar_state;
#[cfg(feature = "explorer")]
pub mod avatar_texture;
#[cfg(feature = "explorer")]
pub mod colliders;
#[cfg(feature = "explorer")]
mod debug_avatars;
#[cfg(feature = "explorer")]
mod debug_wear;
#[cfg(feature = "explorer")]
pub mod dynamic_nametag;
#[cfg(feature = "explorer")]
pub mod emote_report;
#[cfg(feature = "explorer")]
pub mod foot_ik;
#[cfg(feature = "explorer")]
pub mod foreign_dynamics;
#[cfg(feature = "explorer")]
pub mod ghost;
pub mod head_ik;
mod hover_glow;
#[cfg(feature = "explorer")]
mod ik_debug;
#[cfg(feature = "explorer")]
pub mod jump_marker;
pub mod mask_material;
pub mod material_cache;
#[cfg(feature = "explorer")]
pub mod nametag;
#[cfg(feature = "explorer")]
pub mod npc_dynamics;
pub mod pause_hidden;
pub mod point_at_ik;
#[cfg(feature = "explorer")]
pub mod point_at_marker;
mod processing_timing;
#[cfg(feature = "explorer")]
mod reaction;
pub mod spring_bones;
mod two_bone_ik;

#[cfg(test)]
mod outline_emission;
#[cfg(test)]
mod tests;

#[cfg(feature = "explorer")]
use crate::{
    dynamic_nametag::NametagHeadOffset, jump_marker::JumpMarkerPlugin, nametag::NametagPlugin,
};
use common::{
    asset_cache::{clean_asset_cache, AssetCache},
    structs::{
        AppConfig, AttachPoints, Clips, EmoteCommand, EmoteMask, PrimaryUser, SessionGraphics,
    },
    util::{SceneSpawnerPlus, TryPushChildrenEx},
};
#[cfg(feature = "explorer")]
use common::{
    structs::MonotonicTimestamp,
    util::{JoinRelativeExt, TaskExt},
};
#[cfg(feature = "explorer")]
use comms::{
    global_crdt::{ForeignPlayer, GlobalCrdtState},
    profile::UserProfile,
};
use dcl_component::proto_components::{
    common::Color3,
    sdk::components::{common::LoadingState, PbAvatarShape},
    Color3DclToBevy,
};
#[cfg(feature = "explorer")]
use dcl_component::{
    proto_components::sdk::components::{
        pb_avatar_equipped_data::EquippedWearable, PbAssetLoadLoadingState, PbAvatarBase,
        PbAvatarEquippedData, PbGltfContainerLoadingState,
    },
    SceneComponentId, SceneEntityId,
};
#[cfg(feature = "explorer")]
use dcl_types::interface::{ComponentPosition, CrdtType};
#[cfg(feature = "explorer")]
use ipfs::EntityDefinition;
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    partition::PartitionedDownloads,
    IpfsAssetServer, IpfsResource,
};
#[cfg(feature = "explorer")]
use scene_runner::{
    renderer_context::RendererSceneContext, update_world::AddCrdtInterfaceExt, util::ConsoleRelay,
    ContainingScene, SceneEntity,
};

use crate::{
    animate::{AvatarAnimPlayer, LOWER_BODY_BONES, LOWER_BODY_MASK_GROUP},
    material_cache::{bounds_bits, AvatarMaskKey, AvatarMatKey, BoundsBits},
};
#[cfg(feature = "explorer")]
use crate::{
    attach::AttachPlugin, avatar_texture::AvatarTexturePlugin, colliders::AvatarColliderPlugin,
    dynamic_nametag::DynamicNametagPlugin, emote_report::EmoteReportPlugin, foot_ik::FootIkPlugin,
    foreign_dynamics::PlayerMovementPlugin, ghost::GhostAvatarPlugin, head_ik::HeadIkPlugin,
    npc_dynamics::NpcMovementPlugin, point_at_ik::PointAtIkPlugin,
    point_at_marker::PointAtMarkerPlugin,
};

use self::{
    animate::AvatarAnimationPlugin,
    mask_material::{MaskMaterial, MaskMaterialPlugin},
};

/// The render-free part of the avatar stack: what a headless server needs from foreign players
/// (their bevy transforms, their profile in scene crdt, their emotes reported to scenes) without
/// spawning an avatar. `AvatarPlugin` builds on it; the headless binary adds it alone.
#[cfg(feature = "explorer")]
pub struct AvatarCorePlugin;

#[cfg(feature = "explorer")]
impl Plugin for AvatarCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PlayerMovementPlugin);
        app.add_plugins(EmoteReportPlugin);
        app.add_systems(Update, update_avatar_info);
    }
}

/// The avatar renderer: from an `AvatarSelection` to a dressed, animated, emoting avatar. It
/// knows no player, scene or comms: the Explorer adds those through `AvatarPlugin`, the
/// avatar preview adds this alone. Scene-placed avatars report their loading through
/// `AvatarLoadReport` events and take their scene's bounds and hash from `AvatarSceneContext`.
pub struct AvatarRenderPlugin;

impl Plugin for AvatarRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AvatarLoadReport>();
        // the emote lifecycle feed; comms and the emote report add it too, idempotently
        app.add_event::<common::structs::EmoteLifecycleEvent>();
        // the session's Max Avatars; the Explorer's settings initialise it too
        app.init_resource::<SessionGraphics>();
        // read by emote playback; the Explorer's input crate initialises it too
        app.init_resource::<common::structs::SceneDrivenAnimationFeedback>();
        app.add_plugins(processing_timing::AvatarProcessingTimingPlugin);
        app.add_plugins(MaskMaterialPlugin);
        app.add_plugins(AvatarAnimationPlugin);
        app.add_plugins(spring_bones::SpringBonesPlugin);
        app.init_resource::<AssetCache<AvatarMatKey, SceneMaterial>>();
        app.init_resource::<AssetCache<AvatarMaskKey, MaskMaterial>>();
        app.add_systems(
            PostUpdate,
            (
                clean_asset_cache::<AvatarMatKey, SceneMaterial>,
                clean_asset_cache::<AvatarMaskKey, MaskMaterial>,
            ),
        );
        app.add_systems(
            Update,
            (
                update_render_avatar,
                spawn_scenes,
                process_avatar.after(update_render_avatar),
                set_avatar_visibility,
                retag_avatars_on_style_change,
            ),
        );

        app.add_observer(add_attach_points_to_avatar_shape);
        app.add_observer(remove_attach_points_from_avatar_shape);
    }
}

#[cfg(feature = "explorer")]
pub struct AvatarPlugin;

#[cfg(feature = "explorer")]
impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AvatarCorePlugin);
        app.add_plugins(AvatarRenderPlugin);
        app.add_plugins(NpcMovementPlugin);
        app.add_plugins(AttachPlugin);
        app.add_plugins(AvatarColliderPlugin);
        app.add_plugins(AvatarTexturePlugin);
        app.add_plugins(DynamicNametagPlugin);
        app.add_plugins(NametagPlugin);
        app.add_plugins(reaction::ReactionPlugin);
        app.add_plugins(JumpMarkerPlugin);
        app.add_plugins(GhostAvatarPlugin);
        app.add_plugins(FootIkPlugin);
        app.add_plugins(HeadIkPlugin);
        app.add_plugins(ik_debug::IkDebugPlugin);
        app.add_plugins(debug_avatars::DebugAvatarsPlugin);
        app.add_plugins(PointAtIkPlugin);
        app.add_plugins(PointAtMarkerPlugin);
        app.add_plugins(pause_hidden::PauseHiddenAvatarsPlugin);
        app.add_systems(
            Update,
            (
                update_base_avatar_shape,
                // the selection and its scene context land together, before the renderer reads them
                (select_avatar, sync_avatar_scene_context)
                    .chain()
                    .before(update_render_avatar),
                apply_avatar_load_reports.after(process_avatar),
            ),
        );

        app.add_crdt_lww_component::<PbAvatarShape, AvatarShape>(
            SceneComponentId::AVATAR_SHAPE,
            ComponentPosition::Any,
        );

        app.add_preview_console_command::<DebugDumpAvatar, _>(debug_dump_avatar);
        app.add_plugins(avatar_state::AvatarStatePlugin);
    }
}

/// What the renderer needs from the scene an avatar belongs to: the bounds that clip and
/// dither it, and the scene hash that resolves file-path emotes. The Explorer keeps it in step
/// with the scene (`sync_avatar_scene_context`); a selection without it is unbounded.
#[derive(Component, Clone, Debug, Default)]
pub struct AvatarSceneContext {
    pub bounds: SceneBounds,
    pub hash: String,
}

/// An avatar's loading progress, for the scene that placed it. `owner` is the entity holding
/// the `AvatarSelection`; the Explorer's `apply_avatar_load_reports` writes the report into
/// that entity's scene crdt.
#[derive(Event, Clone, Debug)]
pub struct AvatarLoadReport {
    pub owner: Entity,
    pub body_shape: String,
    pub state: LoadingState,
}

/// Copies the scene's bounds and hash onto a scene-placed avatar whenever its selection changes.
#[cfg(feature = "explorer")]
fn sync_avatar_scene_context(
    mut commands: Commands,
    changed: Query<(Entity, &AvatarSelection), Changed<AvatarSelection>>,
    contexts: Query<&RendererSceneContext>,
) {
    for (entity, selection) in changed.iter() {
        let Some(ctx) = selection.scene.and_then(|scene| contexts.get(scene).ok()) else {
            continue;
        };
        commands.entity(entity).try_insert(AvatarSceneContext {
            bounds: ctx.scene_bounds.clone(),
            hash: ctx.hash.to_string(),
        });
    }
}

/// Writes each `AvatarLoadReport` into the owning scene's crdt: the asset load state always, the
/// gltf container state while loading and once finished.
#[cfg(feature = "explorer")]
fn apply_avatar_load_reports(
    mut reports: EventReader<AvatarLoadReport>,
    scene_ents: Query<&SceneEntity>,
    mut contexts: Query<&mut RendererSceneContext>,
    timestamp: Res<MonotonicTimestamp<PbAssetLoadLoadingState>>,
) {
    for report in reports.read() {
        let Ok(scene_ent) = scene_ents.get(report.owner) else {
            continue;
        };
        let Ok(mut context) = contexts.get_mut(scene_ent.root) else {
            continue;
        };
        context.update_crdt(
            SceneComponentId::ASSET_LOAD_LOADING_STATE,
            CrdtType::GO_ANY,
            scene_ent.id,
            &PbAssetLoadLoadingState {
                current_state: report.state as i32,
                asset: report.body_shape.clone(),
                timestamp: timestamp.next_timestamp(),
            },
        );
        if matches!(report.state, LoadingState::Loading | LoadingState::Finished) {
            context.update_crdt(
                SceneComponentId::GLTF_CONTAINER_LOADING_STATE,
                CrdtType::LWW_ENT,
                scene_ent.id,
                &PbGltfContainerLoadingState {
                    current_state: report.state as i32,
                    ..Default::default()
                },
            );
        }
    }
}

/// Send player avatar info into scene crdts — a foreign player's into their own context,
/// the local player's (`PLAYER`) into the client's single context (a multi-context
/// server has no real local player: skip). Also registered directly by the headless
/// server, where the rest of AvatarPlugin (render-bound) is omitted.
#[cfg(feature = "explorer")]
pub fn update_avatar_info(
    updated_players: Query<(Option<&ForeignPlayer>, &UserProfile), Changed<UserProfile>>,
    mut contexts: Query<&mut GlobalCrdtState>,
) {
    for (player, profile) in &updated_players {
        let Some(mut state) = (match player {
            Some(player) => contexts.get_mut(player.context).ok(),
            None => contexts.single_mut().ok(),
        }) else {
            continue;
        };
        let avatar = &profile.content.avatar;
        state.update_crdt(
            SceneComponentId::AVATAR_BASE,
            CrdtType::LWW_ANY,
            player.map(|p| p.scene_id).unwrap_or(SceneEntityId::PLAYER),
            &PbAvatarBase {
                name: profile.content.name.clone(),
                // Voice activity is not reported to scenes by this engine yet.
                talking: None,
                skin_color: avatar.skin.map(|c| c.color),
                eyes_color: avatar.eyes.map(|c| c.color),
                hair_color: avatar.hair.map(|c| c.color),
                body_shape_urn: avatar
                    .body_shape
                    .as_deref()
                    .map(ToString::to_string)
                    .unwrap_or(base_wearables::default_bodyshape_urn().to_string()),
            },
        );
        state.update_crdt(
            SceneComponentId::AVATAR_EQUIPPED_DATA,
            CrdtType::LWW_ANY,
            player.map(|p| p.scene_id).unwrap_or(SceneEntityId::PLAYER),
            &PbAvatarEquippedData {
                wearable_urns: avatar.wearables.to_vec(),
                emote_urns: (0..10)
                    .map(|ix| {
                        avatar
                            .emotes
                            .as_ref()
                            .unwrap_or(&Vec::default())
                            .iter()
                            .find(|emote| emote.slot == ix)
                            .map(|emote| emote.urn.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
                // `visible` stays unset: what the outfit hides is known only once it has loaded
                wearables: avatar
                    .wearables
                    .iter()
                    .map(|urn| EquippedWearable {
                        urn: urn.clone(),
                        visible: None,
                    })
                    .collect(),
                force_render: avatar.force_render.clone().unwrap_or_default(),
            },
        )
    }
}

#[derive(Component, Clone)]
pub struct AvatarShape(pub PbAvatarShape);

impl From<PbAvatarShape> for AvatarShape {
    fn from(value: PbAvatarShape) -> Self {
        Self(value)
    }
}

#[cfg(feature = "explorer")]
impl From<&UserProfile> for AvatarShape {
    fn from(profile: &UserProfile) -> Self {
        AvatarShape(PbAvatarShape {
            id: profile.content.eth_address.clone(),
            // add label only for foreign players
            name: Some(profile.content.name.to_owned()),
            body_shape: Some(
                profile
                    .content
                    .avatar
                    .body_shape
                    .to_owned()
                    .unwrap_or(base_wearables::default_bodyshape_urn().to_string()),
            ),
            skin_color: Some(
                profile
                    .content
                    .avatar
                    .skin
                    .map(|skin| skin.color)
                    .unwrap_or(Color3 {
                        r: 0.6,
                        g: 0.462,
                        b: 0.356,
                    }),
            ),
            hair_color: Some(
                profile
                    .content
                    .avatar
                    .hair
                    .map(|hair| hair.color)
                    .unwrap_or(Color3 {
                        r: 0.283,
                        g: 0.142,
                        b: 0.0,
                    }),
            ),
            eye_color: Some(
                profile
                    .content
                    .avatar
                    .eyes
                    .map(|eye| eye.color)
                    .unwrap_or(Color3 {
                        r: 0.6,
                        g: 0.462,
                        b: 0.356,
                    }),
            ),
            expression_trigger_id: None,
            expression_trigger_timestamp: None,
            talking: None,
            wearables: profile.content.avatar.wearables.to_vec(),
            emotes: profile
                .content
                .avatar
                .emotes
                .as_ref()
                .map(|emotes| {
                    let mut vec = vec![Default::default(); 10];
                    for emote in emotes.iter() {
                        if (emote.slot as usize) < vec.len() {
                            vec[emote.slot as usize] = emote.urn.clone()
                        }
                    }
                    vec
                })
                .unwrap_or_default(),
            force_render: profile
                .content
                .avatar
                .force_render
                .clone()
                .unwrap_or_default(),
            show_only_wearables: None,
        })
    }
}

// set (foreign) user's default avatar shape based on profile data
#[cfg(feature = "explorer")]
fn update_base_avatar_shape(
    mut commands: Commands,
    root_avatar_defs: Query<(Entity, Option<&ForeignPlayer>, &UserProfile), Changed<UserProfile>>,
) {
    for (ent, maybe_player, profile) in &root_avatar_defs {
        let id = match maybe_player {
            Some(player) => player.scene_id,
            None => SceneEntityId::PLAYER,
        };

        debug!("updating default avatar for {id}");

        let mut avatar_shape = AvatarShape::from(profile);

        // show label only for other players
        if maybe_player.is_none() {
            avatar_shape.0.name = None;
        }

        commands.entity(ent).try_insert(avatar_shape);
    }
}

/// A mesh of the avatar's base body (head, torso, hands, legs, feet), as opposed to a wearable's.
/// The avatar preview hides these to show a wearable on its own.
#[derive(Component, Debug, Clone, Copy)]
pub struct AvatarBodyMesh;

#[derive(Component)]
pub struct AvatarSelection {
    /// the scene root that placed this avatar, if any (bounds and hash come from
    /// `AvatarSceneContext`)
    pub scene: Option<Entity>,
    pub shape: AvatarShape,
    /// despawn the avatar when its selection goes
    pub automatic_delete: bool,
    pub disable_dither: bool,
}

// choose the avatar shape based on current scene of the player
#[cfg(feature = "explorer")]
fn select_avatar(
    mut commands: Commands,
    mut root_avatar_defs: Query<
        (
            Entity,
            Option<&ForeignPlayer>,
            &AvatarShape,
            Ref<AvatarShape>,
            Option<&mut AvatarSelection>,
        ),
        Or<(With<ForeignPlayer>, With<PrimaryUser>)>,
    >,
    scene_avatar_defs: Query<(Entity, &SceneEntity, &AvatarShape, Ref<AvatarShape>)>,
    orphaned_avatar_selections: Query<(Entity, &AvatarSelection), Without<AvatarShape>>,
    containing_scene: ContainingScene,
) {
    struct AvatarUpdate {
        base_name: String,
        update_shape: Option<AvatarShape>,
        active_scenes: HashSet<Entity>,
        prev_source: Option<Entity>,
        current_source: Option<Entity>,
    }

    let mut updates = HashMap::new();

    // set up initial state
    for (entity, maybe_player, base_shape, ref_avatar, maybe_prev_selection) in
        root_avatar_defs.iter()
    {
        let id = maybe_player
            .map(|p| p.scene_id)
            .unwrap_or(SceneEntityId::PLAYER);
        updates.insert(
            id,
            AvatarUpdate {
                base_name: base_shape.0.name.clone().unwrap_or_else(|| "Guest".into()),
                update_shape: ref_avatar.is_changed().then_some(base_shape.clone()),
                active_scenes: containing_scene.get(entity),
                prev_source: maybe_prev_selection
                    .as_ref()
                    .map(|prev| prev.scene)
                    .unwrap_or_default(),
                current_source: None,
            },
        );
    }

    for (ent, scene_ent, scene_avatar_shape, ref_avatar) in scene_avatar_defs.iter() {
        let Some(update) = updates.get_mut(&scene_ent.id) else {
            // this is an NPC avatar, attach selection immediately
            if ref_avatar.is_changed() {
                commands.entity(ent).try_insert(AvatarSelection {
                    scene: Some(scene_ent.root),
                    shape: AvatarShape(scene_avatar_shape.0.clone()),
                    automatic_delete: true,
                    disable_dither: false,
                });

                debug!("npc avatar {:?}", scene_ent);
            }

            continue;
        };

        if !update.active_scenes.contains(&scene_ent.root) {
            continue;
        }

        // this is the source
        update.current_source = Some(ent);

        if ref_avatar.is_changed() || update.prev_source != update.current_source {
            // and it needs to be updated
            update.update_shape = Some(AvatarShape(PbAvatarShape {
                name: Some(
                    scene_avatar_shape
                        .0
                        .name
                        .clone()
                        .unwrap_or_else(|| update.base_name.clone()),
                ),
                ..scene_avatar_shape.0.clone()
            }));
        } else {
            // doesn't need to be updated, even if the base shape changed
            // TODO this probably won't pick up name changes though ?
            update.update_shape = None;
        }
    }

    // update avatar selection on foreign players
    for (entity, maybe_player, base_shape, _, maybe_prev_selection) in root_avatar_defs.iter_mut() {
        let id = maybe_player
            .map(|p| p.scene_id)
            .unwrap_or(SceneEntityId::PLAYER);
        let Some(update) = updates.remove(&id) else {
            error!("apparently i never inserted {id}?!");
            continue;
        };
        let needs_update =
            update.current_source != update.prev_source || update.update_shape.is_some();

        if needs_update {
            debug!(
                "updating selected avatar for {} -> {:?}",
                id, update.current_source
            );

            let shape = update.update_shape.unwrap_or(base_shape.clone());
            if let Some(mut selection) = maybe_prev_selection {
                selection.shape = shape;
                selection.scene = update.current_source;
            } else {
                commands.entity(entity).try_insert(AvatarSelection {
                    scene: update.current_source,
                    shape,
                    automatic_delete: true,
                    disable_dither: maybe_player.is_none(), // disable for primary avatar only
                });
            }
        }
    }

    // remove any orphans
    for (entity, selection) in &orphaned_avatar_selections {
        if selection.automatic_delete {
            if let Ok(mut commands) = commands.get_entity(entity) {
                commands.remove::<AvatarSelection>();
            }
        }
    }
}

#[derive(Component)]
pub struct AvatarDefinition {
    render: AvatarRenderDefinition,
    /// played once the avatar is processed. a change of emote alone is applied to the avatar
    /// as it stands, without a rebuild
    emote: Option<EmoteCommand>,
}

/// everything the built avatar is made from: a change here means a rebuild
#[derive(PartialEq)]
pub struct AvatarRenderDefinition {
    body: Wearable,
    body_shape: String,
    skin_color: Color,
    hair_color: Color,
    eyes_color: Color,
    /// each worn wearable with its urn, in the order of their categories
    wearables: Vec<(WearableUrn, Wearable)>,
    hides: HashSet<WearableCategory>,
    bounds: SceneBounds,
    disable_dither: bool,
    avatar_outline: bool,
}

impl AvatarRenderDefinition {
    /// The tag of a mesh of `model`. Avatars are always toon shaded; the outline follows the
    /// setting in the definition, and toggling it re-renders avatars (see
    /// retag_avatars_on_style_change). Wearables can opt out of the outline via
    /// `outlineCompatible: false`.
    fn model_tag(&self, model: Option<&WearableModel>) -> u32 {
        let outline = self.avatar_outline && model.is_some_and(|model| model.outline_compatible);
        (if outline {
            SCENE_MATERIAL_OUTLINE_MESH_TAG
        } else {
            0
        }) | (if self.disable_dither {
            SCENE_MATERIAL_NO_DITHERING_MESH_TAG
        } else {
            0
        }) | SCENE_MATERIAL_TOON_MESH_TAG
    }

    /// the body model is guaranteed by spawn_scenes
    fn body_tag(&self) -> u32 {
        self.model_tag(self.body.model.as_ref())
    }

    /// The eyes, eyebrows and mouth are drawn on the body's head, so they outline and fade as
    /// the body does; they are lit by the ambient light alone (toon.wgsl's
    /// `facial_feature_lighting`).
    fn mask_tag(&self) -> u32 {
        self.body_tag() | SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG
    }
}

#[derive(Component)]
pub struct RetryRenderAvatar;

/// The files an avatar could not be built from. The avatar is rebuilt without them: a failed
/// body falls back to the default body, and a failed wearable leaves its category to the
/// body's default, so a broken file never leaves the avatar missing or with parts hidden.
/// Each rebuild adds at least one file, so the fallback ends.
#[derive(Component, Default, Debug)]
pub struct AvatarLoadFailures {
    bodies: HashSet<WearableUrn>,
    wearables: HashSet<WearableUrn>,
}

/// Whether a mesh of `model` glows while its avatar is hovered: a wearable opts out with
/// `outlineCompatible: false`, whatever the outline setting.
fn hover_glows(model: Option<&WearableModel>) -> bool {
    model.is_none_or(|model| model.outline_compatible)
}

impl AvatarLoadFailures {
    /// The body to build with: the requested one, else the default body, else none when both
    /// failed.
    fn body(failures: Option<&Self>, requested: WearableUrn) -> Option<WearableUrn> {
        let Some(failures) = failures else {
            return Some(requested);
        };
        if !failures.bodies.contains(&requested) {
            return Some(requested);
        }
        let default = base_wearables::default_bodyshape_urn();
        (!failures.bodies.contains(&default)).then_some(default)
    }

    fn wearable_failed(failures: Option<&Self>, urn: &WearableUrn) -> bool {
        failures.is_some_and(|failures| failures.wearables.contains(urn))
    }
}

/// The body an avatar shape asks for: its own, else the default body.
fn requested_body(shape: &PbAvatarShape) -> WearableUrn {
    shape
        .body_shape
        .as_ref()
        .and_then(|b| WearableUrn::new(b).ok())
        .unwrap_or(base_wearables::default_bodyshape_urn())
}

/// Drops the build `build` of the avatar on `avatar` and has the avatar rebuilt without the
/// failed files. The avatar shown before stays until the rebuild replaces it.
fn abandon_build(
    commands: &mut Commands,
    build: Entity,
    avatar: Entity,
    body: Option<WearableUrn>,
    wearables: Vec<WearableUrn>,
) {
    commands.entity(build).try_despawn();
    record_avatar_failures(commands, avatar, body, wearables);
}

/// Records files the avatar on `avatar` failed to build from and has it rebuilt without them.
fn record_avatar_failures(
    commands: &mut Commands,
    avatar: Entity,
    body: Option<WearableUrn>,
    wearables: Vec<WearableUrn>,
) {
    commands
        .entity(avatar)
        .queue_silenced(move |mut entity: EntityWorldMut| {
            let mut failures = entity.take::<AvatarLoadFailures>().unwrap_or_default();
            failures.bodies.extend(body);
            failures.wearables.extend(wearables);
            entity.insert((failures, RetryRenderAvatar));
        });
}

// when the avatar outline setting is toggled, re-render every
// avatar so their meshes are respawned with the right mesh tags. the tags are set
// once at mesh creation in process_avatar, so an in-place flip isn't enough.
fn retag_avatars_on_style_change(
    config: Res<AppConfig>,
    mut prev: Local<Option<bool>>,
    avatars: Query<Entity, With<AvatarSelection>>,
    mut commands: Commands,
) {
    let cur = config.graphics.avatar_outline;
    if *prev != Some(cur) {
        if prev.is_some() {
            for entity in &avatars {
                commands.entity(entity).try_insert(RetryRenderAvatar);
            }
        }
        *prev = Some(cur);
    }
}

// load wearables and create renderable avatar entity once all loaded
fn update_render_avatar(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &AvatarSelection,
            Option<&Children>,
            Option<&AttachPoints>,
            Option<&AvatarSceneContext>,
            Option<&PreviousAvatar>,
            Option<&PartitionedDownloads>,
            Option<&AvatarLoadFailures>,
        ),
        Or<(Changed<AvatarSelection>, With<RetryRenderAvatar>)>,
    >,
    mut removed_selections: RemovedComponents<AvatarSelection>,
    children: Query<(&Children, Option<&AttachPoints>)>,
    mut avatar_render_entities: Query<(&mut AvatarDefinition, Has<AvatarProcessed>)>,
    mut wearable_loader: CollectibleManager<Wearable>,
    mut reports: EventWriter<AvatarLoadReport>,
    ipfas: IpfsAssetServer,
    config: Res<AppConfig>,
) {
    // remove renderable entities when avatar selection is removed
    for entity in removed_selections.read() {
        // the outfit's hashes leave the request-slot queue with the component
        commands.entity(entity).try_remove::<PartitionedDownloads>();
        if let Ok((children, attach_points)) = children.get(entity) {
            if let Some(attach_points) = attach_points {
                commands
                    .entity(entity)
                    .try_push_children(&attach_points.entities());
            }
            for render_child in children
                .iter()
                .filter(|child| avatar_render_entities.get(*child).is_ok())
            {
                commands.entity(render_child).despawn();
            }
        }
    }

    for (
        entity,
        selection,
        maybe_children,
        maybe_attach_points,
        maybe_scene_ctx,
        maybe_prev,
        maybe_downloads,
        maybe_failures,
    ) in &query
    {
        commands.entity(entity).try_remove::<RetryRenderAvatar>();

        debug!("updating render avatar");

        // get body shape, falling back to the default body when the requested one failed
        let requested_body = requested_body(&selection.shape.0);
        let Some(body_urn) = AvatarLoadFailures::body(maybe_failures, requested_body.clone())
        else {
            warn!("no body to build {requested_body} with: it and the default body failed");
            reports.write(AvatarLoadReport {
                owner: entity,
                body_shape: requested_body.as_str().to_owned(),
                state: LoadingState::FinishedWithError,
            });
            continue;
        };
        if body_urn != requested_body {
            debug!("body {requested_body} failed, building with {body_urn}");
        }

        // the avatar's downloads are filed under its body's and wearables' hashes, so they
        // take the avatar's partition in the request-slot queue; whatever has resolved so far
        let worn: Vec<WearableUrn> = std::iter::once(body_urn.clone())
            .chain(
                selection
                    .shape
                    .0
                    .wearables
                    .iter()
                    .flat_map(|w| WearableUrn::new(w).ok()),
            )
            .chain(
                base_wearables::default_wearables(&body_urn)
                    .map(|default| default.base().to_owned()),
            )
            .collect();
        let hashes =
            common::partition::resolved_hashes(worn.iter().map(|urn| urn.as_str()), |urn| {
                WearableUrn::new(urn)
                    .ok()
                    .and_then(|urn| wearable_loader.get_hash(&urn).ok())
            });
        // only on change: inserting it re-registers the hashes with the request slots
        if maybe_downloads.is_none_or(|current| current.0 != hashes) {
            commands
                .entity(entity)
                .try_insert(PartitionedDownloads(hashes));
        }

        let body = match wearable_loader.get_representation(&body_urn, body_urn.as_str()) {
            Ok(body) => body.clone(),
            Err(CollectibleError::Loading) => {
                debug!("waiting for hash from body {body_urn}");
                commands.entity(entity).try_insert(RetryRenderAvatar);
                continue;
            }
            Err(other) => {
                warn!("failed to resolve body {body_urn}: {other:?}");
                // rebuilt with the default body, or reported as failed if this was it
                record_avatar_failures(&mut commands, entity, Some(body_urn), Vec::new());
                continue;
            }
        };

        // get wearables
        let mut all_loaded = true;
        let specified_wearables: Vec<_> = selection
            .shape
            .0
            .wearables
            .iter()
            .flat_map(|w| WearableUrn::new(w).ok())
            // a wearable whose model failed leaves its category to the default
            .filter(|wearable| !AvatarLoadFailures::wearable_failed(maybe_failures, wearable))
            .flat_map(|wearable| {
                match wearable_loader.get_representation(&wearable, body_urn.as_str()) {
                    Ok(data) => Some((data.category, (data.clone(), wearable))),
                    Err(CollectibleError::Loading) => {
                        debug!("waiting for wearable {wearable:?}");
                        all_loaded = false;
                        None
                    }
                    Err(other) => {
                        debug!("skipping failed wearable {wearable:?}: {other:?}");
                        None
                    }
                }
            })
            .collect();

        // take only the first item for each category and build a category map
        let mut wearables = HashMap::new();
        for (category, data) in specified_wearables {
            if !wearables.contains_key(&category) && category != WearableCategory::BODY_SHAPE {
                wearables.insert(category, data);
            }
        }

        // add defaults
        let defaults: Vec<_> = base_wearables::default_wearables(&body_urn)
            .filter(|default| !AvatarLoadFailures::wearable_failed(maybe_failures, default.base()))
            .flat_map(|default| {
                match wearable_loader.get_representation(default.base(), body_urn.as_str()) {
                    Ok(data) => Some((data.clone(), default.base().to_owned())),
                    Err(CollectibleError::Loading) => {
                        debug!("waiting for {default:?}");
                        all_loaded = false;
                        None
                    }
                    // a default that never resolves leaves its category to the base model,
                    // instead of a retry every frame
                    Err(other) => {
                        warn!("skipping failed default wearable {default:?}: {other:?}");
                        None
                    }
                }
            })
            .collect();

        if !all_loaded {
            commands.entity(entity).try_insert(RetryRenderAvatar);
            continue;
        }

        for (default, default_urn) in defaults {
            if !wearables.contains_key(&default.category) {
                debug!("adding default {default:?}");
                wearables.insert(default.category, (default, default_urn));
            }
        }

        // calculate what is hidden
        let mut hides_first_pass: HashSet<WearableCategory, _> = HashSet::new();

        debug!("calculating hides phase 1");
        for category in WearableCategory::hides_order() {
            if hides_first_pass.contains(category) {
                debug!("skip {:?}, already hidden", category);
                continue;
            }

            if let Some((wearable, _)) = wearables.get(category) {
                hides_first_pass.extend(wearable.hides.iter());
                debug!(
                    "add {:?} = {:?} -> {:?}",
                    category, wearable.hides, hides_first_pass
                );
            }

            hides_first_pass
                .retain(|cat| !selection.shape.0.force_render.iter().any(|h| h == cat.slot));
        }

        debug!("calculating hides phase 2");
        let mut hides: HashSet<WearableCategory, _> = HashSet::new();
        for category in WearableCategory::hides_order() {
            if hides_first_pass.contains(category) {
                debug!("skip {:?}, hidden", category);
                continue;
            }

            if let Some((wearable, _)) = wearables.get(category) {
                hides.extend(wearable.hides.iter());
                debug!("add {:?} = {:?} -> {:?}", category, wearable.hides, hides);
            }

            hides.retain(|cat| !selection.shape.0.force_render.iter().any(|h| h == cat.slot));
        }

        if selection.shape.0.show_only_wearables() {
            hides.insert(WearableCategory::BODY_SHAPE);
        }

        let initial_count = wearables.len();
        wearables.retain(|cat, _| !hides.contains(cat));
        debug!(
            "hides {} / {} categories, leaves {}/{} wearables",
            hides.len(),
            WearableCategory::iter().count(),
            wearables.len(),
            initial_count
        );

        let mut wearables: Vec<(WearableUrn, Wearable)> = wearables
            .into_values()
            .map(|(wearable, urn)| (urn, wearable))
            .collect();
        // a fixed order, so an unchanged definition compares equal
        wearables.sort_by_key(|(_, wearable)| wearable.category);

        let mut urns: HashSet<_> = wearables.iter().map(|(urn, _)| urn.clone()).collect();
        urns.insert(body_urn.clone());

        debug!("avatar definition loaded: {wearables:?}");
        let definition = AvatarDefinition {
            render: AvatarRenderDefinition {
                body,
                body_shape: body_urn.as_str().to_owned(),
                wearables,
                hides,
                skin_color: selection
                    .shape
                    .0
                    .skin_color
                    .unwrap_or(Color3 {
                        r: 0.6,
                        g: 0.462,
                        b: 0.356,
                    })
                    .convert_srgb(),
                hair_color: selection
                    .shape
                    .0
                    .hair_color
                    .unwrap_or(Color3 {
                        r: 0.283,
                        g: 0.142,
                        b: 0.0,
                    })
                    .convert_srgb(),
                eyes_color: selection
                    .shape
                    .0
                    .eye_color
                    .unwrap_or(Color3 {
                        r: 0.6,
                        g: 0.462,
                        b: 0.356,
                    })
                    .convert_srgb(),
                bounds: maybe_scene_ctx
                    .map(|ctx| ctx.bounds.clone())
                    .unwrap_or_default(),
                disable_dither: selection.disable_dither,
                avatar_outline: config.graphics.avatar_outline,
            },
            emote: selection
                .shape
                .0
                .expression_trigger_id
                .as_ref()
                // a cleared trigger on a scene-sourced shape stops the emote: an empty
                // command. profile-derived shapes never carry a trigger.
                .or(selection.scene.is_some().then_some(&String::new()))
                .and_then(|e| {
                    let urn = if e.is_empty() {
                        String::new()
                    } else if e.starts_with("urn:") {
                        e.clone()
                    } else {
                        // File path emote (e.g. "models/emotes/foo.glb") — resolve
                        // through the scene's content map to build a scene-emote URN,
                        // mirroring the logic in op_scene_emote.
                        let scene_emote = maybe_scene_ctx
                            .and_then(|ctx| {
                                let scene_hash = &ctx.hash;
                                let ipfs_path = IpfsPath::new(IpfsType::new_content_file(
                                    scene_hash.to_string(),
                                    e.to_lowercase(),
                                ));
                                let ipfs_context = ipfas.ipfs().context.blocking_read();
                                let emote_hash = ipfs_path.hash(&ipfs_context)?;
                                Some(format!(
                                    "urn:decentraland:off-chain:scene-emote:{scene_hash}-{emote_hash}-false"
                                ))
                            });

                        // otherwise a bare emote name ("robot"), kept as written: the
                        // collection it comes from is only known once it resolves. content
                        // map first because a file path is also a valid single-segment urn.
                        // base emote names are all alphanumeric, so anything else the
                        // content map missed is a missing file, not an emote name.
                        match scene_emote.or_else(|| {
                            e.chars()
                                .all(|c| c.is_ascii_alphanumeric())
                                .then(|| e.clone())
                        }) {
                            Some(urn) => urn,
                            None => {
                                warn!("ignoring avatar shape emote '{e}': not in the scene content map, and not a base emote name");
                                return None;
                            }
                        }
                    };
                    Some(EmoteCommand {
                        urn,
                        r#loop: false,
                        timestamp: selection
                            .shape
                            .0
                            .expression_trigger_timestamp
                            .unwrap_or_default(),
                        mask: EmoteMask::FullBody,
                    })
                }),
        };

        // an unchanged render definition (a profile change that doesn't show, like its
        // snapshots) keeps the avatar already built, or still loading. a new emote alone goes
        // to the avatar as it stands: played now once it's processed, else when it is
        let current = maybe_children.and_then(|children| {
            children.iter().rfind(|child| {
                maybe_prev.is_none_or(|prev| prev.0 != *child)
                    && avatar_render_entities.contains(*child)
            })
        });
        if let Some(Ok((mut current, processed))) =
            current.map(|current| avatar_render_entities.get_mut(current))
        {
            if current.render == definition.render {
                if current.emote != definition.emote {
                    debug!("avatar emote changed");
                    if let Some(emote) = definition.emote.as_ref().filter(|_| processed) {
                        commands.entity(entity).try_insert(emote.clone());
                    }
                    current.emote = definition.emote;
                } else {
                    debug!("avatar definition unchanged");
                }
                continue;
            }
        }

        if let Some(attach_points) = maybe_attach_points {
            // reparent attach points. they leave the bones the nametag's offset was measured
            // on: it is measured again on the new body
            commands
                .entity(entity)
                .try_push_children(&attach_points.entities());
            #[cfg(feature = "explorer")]
            commands.entity(entity).try_remove::<NametagHeadOffset>();
        }

        reports.write(AvatarLoadReport {
            owner: entity,
            body_shape: body_urn.as_str().to_owned(),
            state: LoadingState::Loading,
        });
        commands.entity(entity).try_with_children(|commands| {
            commands.spawn((
                Transform::from_rotation(Quat::from_rotation_y(PI)),
                Visibility::default(),
                definition,
                UsedWearables(urns),
            ));
        });

        // remove existing children
        let mut chose_existing = false;
        if let Some(children) = maybe_children {
            for render_child in children
                .iter()
                .filter(|child| avatar_render_entities.get(*child).is_ok())
            {
                if chose_existing || maybe_prev.is_some_and(|e| e.0 != render_child) {
                    // one built avatar stays on show while the new one builds
                    debug!("despawn {render_child:?}: the previous avatar is already chosen");
                    commands.entity(render_child).despawn();
                } else {
                    debug!("set prev -> {render_child:?}");
                    chose_existing = true;
                    commands
                        .entity(entity)
                        .try_insert(PreviousAvatar(render_child));
                }
            }
        }
    }
}

/// Whether a face wearable's texture or mask is still loading. A failed one does not hold the
/// build: that face is drawn without it.
fn face_images_loading<'a>(
    wearables: impl IntoIterator<Item = &'a Wearable>,
    load_state: impl Fn(&Handle<Image>) -> Option<bevy::asset::LoadState>,
) -> bool {
    wearables
        .into_iter()
        .flat_map(|wearable| wearable.texture.iter().chain(&wearable.mask))
        .any(|image| matches!(load_state(image), Some(bevy::asset::LoadState::Loading)))
}

#[derive(Component)]
pub struct AvatarLoaded {
    body_instance: InstanceId,
    wearable_instances: Vec<Option<InstanceId>>,
    skin_materials: HashSet<Handle<StandardMaterial>>,
    hair_materials: HashSet<Handle<StandardMaterial>>,
}
#[derive(Component)]
pub struct AvatarProcessed;

/// The suffix the glTF loader adds to a material's label for the copy it makes for a mirrored
/// (negative-scale) node, whose faces wind the other way.
const MIRRORED_MATERIAL_SUFFIX: &str = " (inverted)";

/// Whether `material` is the glTF loader's copy of a material for a mirrored node.
fn is_mirrored_material(material: &Handle<StandardMaterial>) -> bool {
    material
        .path()
        .and_then(|path| path.label())
        .is_some_and(|label| label.ends_with(MIRRORED_MATERIAL_SUFFIX))
}

/// A glTF material and, when a mirrored node uses it, the loader's mirrored copy, which
/// `Gltf::materials` does not list.
fn material_and_mirrored(
    material: &Handle<StandardMaterial>,
    asset_server: &AssetServer,
) -> impl Iterator<Item = Handle<StandardMaterial>> {
    let mirrored = material.path().and_then(|path| {
        let label = path.label()?;
        let mirrored = path
            .clone()
            .with_label(format!("{label}{MIRRORED_MATERIAL_SUFFIX}"));
        asset_server.get_handle::<StandardMaterial>(mirrored)
    });
    std::iter::once(material.clone_weak()).chain(mirrored.map(|handle| handle.clone_weak()))
}

/// Divides the KHR_materials_emissive_strength out of a loaded wearable glTF's materials: the glTF
/// loader multiplies the emissive by it, and avatars do not take that extension. The strengths
/// come from the glTF's source, the document and binary chunk the loader kept for this, which is
/// then dropped: a glTF already divided has no source left.
fn drop_emissive_strengths(
    gltfs: &mut Assets<Gltf>,
    id: AssetId<Gltf>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    if gltfs.get(id).is_none_or(|gltf| gltf.source.is_none()) {
        return;
    }
    let Some(gltf) = gltfs.get_mut(id) else {
        return;
    };
    let Some(source) = gltf.source.take() else {
        return;
    };
    // the loader keeps the materials in the document's order
    for (material, handle) in source.document.materials().zip(&gltf.materials) {
        if let Some(strength) = material
            .emissive_strength()
            .filter(|s| *s > 0.0 && *s != 1.0)
        {
            divide_emissive(materials, handle, strength, asset_server);
        }
    }
}

/// Divides `strength` out of a glTF material's emissive colour, and out of its mirrored copy's.
fn divide_emissive(
    materials: &mut Assets<StandardMaterial>,
    material: &Handle<StandardMaterial>,
    strength: f32,
    asset_server: &AssetServer,
) {
    for handle in material_and_mirrored(material, asset_server) {
        if let Some(material) = materials.get_mut(&handle) {
            // the alpha is the exposure weight, not part of the colour
            material.emissive.red /= strength;
            material.emissive.green /= strength;
            material.emissive.blue /= strength;
        }
    }
}

/// Tells a scene how the avatar on one of its entities is loading, as the AssetLoad loading
/// state of that entity (a grow-only set the scene reads in order): Loading once the look's
/// files are being fetched, Finished once it renders, FinishedWithError when it never will.
// instantiate avatar gltfs
fn spawn_scenes(
    mut commands: Commands,
    query: Query<
        (Entity, &AvatarDefinition, &ChildOf),
        (Without<AvatarLoaded>, Without<AvatarProcessed>),
    >,
    asset_server: Res<AssetServer>,
    mut gltfs: ResMut<Assets<Gltf>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut scene_spawner: ResMut<SceneSpawner>,
    // absent without the asset pipeline (tests): nothing reloads
    ipfs: Option<Res<IpfsResource>>,
) {
    for (ent, def, parent) in query.iter() {
        let def = &def.render;
        let body_urn = || WearableUrn::new(&def.body_shape).ok();
        let any_loading = def
            .body
            .model
            .iter()
            .chain(
                def.wearables
                    .iter()
                    .flat_map(|(_, wearable)| wearable.model.as_ref()),
            )
            .any(|model| {
                let h_model = &model.gltf;
                match asset_server.get_load_state(h_model) {
                    Some(bevy::asset::LoadState::Loading) => true,
                    // a model whose `glb` variant failed loads again from its original: wait
                    // for it rather than build the avatar without that piece
                    Some(bevy::asset::LoadState::Failed(_)) => ipfs
                        .as_ref()
                        .zip(h_model.path())
                        .is_some_and(|(ipfs, path)| {
                            ipfs.bevy_variant_model_reload_pending(path.path())
                        }),
                    _ => false,
                }
            });

        // the face's textures and masks too: the avatar on show stays until this one can draw its
        // eyes, eyebrows and mouth, instead of showing a bare face while they load
        let face_loading = face_images_loading(
            def.wearables.iter().map(|(_, wearable)| wearable),
            |image| asset_server.get_load_state(image),
        );

        if any_loading || face_loading {
            continue;
        }

        let models = def.body.model.iter().chain(
            def.wearables
                .iter()
                .flat_map(|(_, wearable)| wearable.model.as_ref()),
        );
        for model in models {
            drop_emissive_strengths(
                &mut gltfs,
                model.gltf.id(),
                &mut standard_materials,
                &asset_server,
            );
        }

        let Some(gltf) = def
            .body
            .model
            .as_ref()
            .and_then(|model| gltfs.get(&model.gltf))
        else {
            match def
                .body
                .model
                .as_ref()
                .and_then(|model| asset_server.get_load_state(&model.gltf))
            {
                Some(bevy::asset::LoadState::Loading) | Some(bevy::asset::LoadState::NotLoaded) => {
                    // nothing to do
                }
                otherwise => {
                    warn!("failed to load body gltf: {otherwise:?}");
                    abandon_build(&mut commands, ent, parent.parent(), body_urn(), Vec::new());
                }
            }
            continue;
        };

        let Some(h_scene) = gltf.default_scene.as_ref() else {
            warn!("body gltf has no default scene");
            abandon_build(&mut commands, ent, parent.parent(), body_urn(), Vec::new());
            continue;
        };

        // a wearable whose model failed would still hide the body parts it covers: rebuild
        // without it, so its category falls back to the default
        let failed_wearables: Vec<WearableUrn> = def
            .wearables
            .iter()
            .filter(|(_, wearable)| {
                wearable.model.as_ref().is_some_and(|model| {
                    match asset_server.get_load_state(&model.gltf) {
                        Some(bevy::asset::LoadState::Failed(_)) => true,
                        Some(bevy::asset::LoadState::Loaded) => gltfs
                            .get(&model.gltf)
                            .is_none_or(|gltf| gltf.default_scene.is_none()),
                        _ => false,
                    }
                })
            })
            .map(|(urn, _)| urn.clone())
            .collect();
        if !failed_wearables.is_empty() {
            warn!("wearable models failed to load: {failed_wearables:?}");
            abandon_build(&mut commands, ent, parent.parent(), None, failed_wearables);
            continue;
        }

        // store skin/hair materials to update with color on spawned instance
        let mut skin_materials = HashSet::default();
        let mut hair_materials = HashSet::default();

        skin_materials.extend(
            gltf.named_materials
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains("skin"))
                .flat_map(|(_, h)| material_and_mirrored(h, &asset_server)),
        );

        hair_materials.extend(
            gltf.named_materials
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains("hair"))
                .flat_map(|(_, h)| material_and_mirrored(h, &asset_server)),
        );

        let body_instance = scene_spawner.spawn_as_child(h_scene.clone(), ent);

        let instances = def
            .wearables
            .iter()
            .flat_map(|(_, wearable)| &wearable.model)
            .flat_map(|model| {
                let h_gltf = &model.gltf;
                match asset_server.get_load_state(h_gltf) {
                    Some(bevy::asset::LoadState::Loaded) => (),
                    otherwise => {
                        // keep the slot so `wearable_instances` stays aligned with the
                        // wearables that have models
                        warn!("wearable gltf didn't work out: {otherwise:?}");
                        return Some(None);
                    }
                }

                // checked above, with the wearables that failed
                let Some(gltf) = gltfs.get(h_gltf) else {
                    return Some(None);
                };
                let gltf_scene_handle = gltf.default_scene.as_ref();

                // store skin/hair materials
                skin_materials.extend(
                    gltf.named_materials
                        .iter()
                        .filter(|(name, _)| name.to_lowercase().contains("skin"))
                        .flat_map(|(_, h)| material_and_mirrored(h, &asset_server)),
                );
                hair_materials.extend(
                    gltf.named_materials
                        .iter()
                        .filter(|(name, _)| name.to_lowercase().contains("hair"))
                        .flat_map(|(_, h)| material_and_mirrored(h, &asset_server)),
                );

                Some(
                    gltf_scene_handle
                        .map(|h_scene| scene_spawner.spawn_as_child(h_scene.clone(), ent)),
                )
            });

        debug!("avatar files loaded");
        commands.entity(ent).try_insert((
            AvatarLoaded {
                body_instance,
                wearable_instances: instances.collect(),
                skin_materials,
                hair_materials,
            },
            Visibility::Hidden,
        ));
    }
}

#[derive(Component)]
pub struct AvatarMaterials(pub HashSet<AssetId<SceneMaterial>>);

#[derive(Component, Debug)]
pub struct PreviousAvatar(Entity);

/// An avatar's emission for one channel of a glTF emissive factor, in the toon lighting's output
/// units (lit white is 1): the factor times 5, taken as a linear value with no gamma decode, times
/// 2.5, so a factor of 1 emits 12.5. toon.wgsl undoes bound_material's x5 lift.
fn avatar_emission(factor: f32) -> f32 {
    factor.max(0.0) * 5.0 * 2.5
}
/// The camera-to-player dither's base opacity for avatars: an avatar fades as a whole, by the
/// worst point of its capsule, so all of it shows the floor. Higher than text's for that reason.
const AVATAR_DITHER_BASE_OPACITY: f32 = 0.5;

// update materials and hide base parts
fn process_avatar(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &GlobalTransform,
            &AvatarDefinition,
            &AvatarLoaded,
            &ChildOf,
        ),
        Without<AvatarProcessed>,
    >,
    scene_spawner: SceneSpawnerPlus,
    mut instance_ents: Query<(
        &mut Visibility,
        &ChildOf,
        &GlobalTransform,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&Mesh3d>,
        Option<&AnimationPlayer>,
    )>,
    named_ents: Query<&Name>,
    mut skins: Query<&mut SkinnedMesh>,
    standard_materials: Res<Assets<StandardMaterial>>,
    mut scene_materials: ResMut<Assets<SceneMaterial>>,
    mut mask_materials: ResMut<Assets<MaskMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    gltfs: Res<Assets<Gltf>>,
    attach_points: Query<&AttachPoints>,
    (config, inverse_bindposes, mut mat_cache, mut mask_cache, mut run): (
        Res<AppConfig>,
        Res<Assets<SkinnedMeshInverseBindposes>>,
        ResMut<AssetCache<AvatarMatKey, SceneMaterial>>,
        ResMut<AssetCache<AvatarMaskKey, MaskMaterial>>,
        ResMut<processing_timing::AvatarProcessingRun>,
    ),
    mut emote_loader: CollectibleManager<Emote>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    (name_and_parent, local_transforms, previous_avatar, previous_animator, mut reports): (
        Query<(&Name, &ChildOf)>,
        Query<(&Transform, &ChildOf)>,
        Query<&PreviousAvatar>,
        Query<
            (),
            (
                With<AnimationPlayer>,
                With<AnimationGraphHandle>,
                With<Clips>,
                With<AnimationTransitions>,
            ),
        >,
        EventWriter<AvatarLoadReport>,
    ),
) {
    let started = bevy::platform::time::Instant::now();
    let mut processed = 0;
    for (avatar_ent, root_gt, def, loaded_avatar, root_player_entity) in query.iter() {
        let (def, emote) = (&def.render, &def.emote);
        let not_loaded = !scene_spawner.instance_is_really_ready(loaded_avatar.body_instance)
            || loaded_avatar
                .wearable_instances
                .iter()
                .any(|maybe_instance| {
                    maybe_instance
                        .is_some_and(|instance| !scene_spawner.instance_is_really_ready(instance))
                });

        if not_loaded {
            debug!("not loaded...");
            continue;
        }
        processed += 1;

        // https://docs.decentraland.org/creator/wearables/creating-wearables/#max-width-height-and-depth-dimension-of-the-wearables
        let aabb_bounds = (
            Vec3::new(-1.24, 0.0, -0.7) + root_gt.translation(),
            Vec3::new(1.24, 2.42, 0.7) + root_gt.translation(),
        );

        let bounds_key = bounds_bits(&def.bounds);
        // how far the visible meshes reach above the head bone, along it and in its own units:
        // the nametag clears them. read from each skin's bind pose, so the pose the avatar is
        // built in does not matter
        let mut meshes_top = f32::NEG_INFINITY;
        let mut instance_scene_materials = HashMap::new();
        let mut armature_node = None;
        let mut target_armature_entities = HashMap::new();

        // hide and colour the base model
        for scene_ent in scene_spawner.iter_instance_entities(loaded_avatar.body_instance) {
            let Ok((mut vis, parent, gt, maybe_h_mat, maybe_h_mesh, maybe_player)) =
                instance_ents.get_mut(scene_ent)
            else {
                continue;
            };

            if maybe_player.is_some() {
                commands.entity(scene_ent).remove::<AnimationPlayer>();
            }

            let Ok(name) = named_ents.get(scene_ent) else {
                continue;
            };
            let name = name.to_lowercase();

            // add animation player to armature root
            if name == "armature" && armature_node.is_none() {
                armature_node = Some(scene_ent);
            }

            // record bone entities
            if name.to_lowercase().starts_with("avatar_") {
                target_armature_entities.insert(name.to_lowercase(), scene_ent);
            }

            if let Some(h_mesh) = maybe_h_mesh {
                commands.entity(scene_ent).try_insert(AvatarBodyMesh);
                if def.hides.contains(&WearableCategory::BODY_SHAPE) {
                    commands.entity(scene_ent).try_insert(Visibility::Hidden);
                }

                if name.contains("head") && def.hides.contains(&WearableCategory::HEAD) {
                    commands.entity(scene_ent).try_insert(Visibility::Hidden);
                }

                if name.contains("hands") && def.hides.contains(&WearableCategory::HANDS) {
                    commands.entity(scene_ent).try_insert(Visibility::Hidden);
                }

                upload_cpu_mesh(&mut meshes, h_mesh);

                let hidden = def.hides.contains(&WearableCategory::BODY_SHAPE)
                    || (name.contains("head") && def.hides.contains(&WearableCategory::HEAD));
                if !hidden {
                    meshes_top = meshes_top.max(top_above_head(
                        scene_ent,
                        h_mesh,
                        &skins,
                        &named_ents,
                        &inverse_bindposes,
                        &meshes,
                    ));
                }

                commands.entity(scene_ent).try_insert(calculate_local_aabb(
                    aabb_bounds.0,
                    aabb_bounds.1,
                    gt,
                ));
            }

            if let Some(h_mat) = maybe_h_mat {
                commands
                    .entity(scene_ent)
                    .remove::<MeshMaterial3d<StandardMaterial>>();

                if let Some(mat) = standard_materials.get(h_mat) {
                    let instance_mat = instance_scene_materials
                        .entry(h_mat.clone_weak())
                        .or_insert_with(|| {
                            derived_scene_material(
                                mat,
                                h_mat,
                                def,
                                loaded_avatar,
                                -5000.0, // make base model appear under any wearables at the same position, like skinpaint
                                &bounds_key,
                                config.graphics.oob,
                                &mut mat_cache,
                                &mut scene_materials,
                            )
                        });
                    let mut mesh = commands.entity(scene_ent);
                    mesh.try_insert((
                        MeshMaterial3d(instance_mat.clone()),
                        MeshTag(def.body_tag()),
                    ));
                    set_hover_glows(
                        &mut mesh,
                        hover_glows(def.body.model.as_ref()) && can_glow(mat.alpha_mode),
                    );
                }
            }

            let masks = [
                ("mask_eyes", def.eyes_color, WearableCategory::EYES, true),
                (
                    "mask_eyebrows",
                    def.hair_color,
                    WearableCategory::EYEBROWS,
                    false,
                ),
                ("mask_mouth", def.skin_color, WearableCategory::MOUTH, false),
            ];

            let Ok(parent_name) = named_ents.get(parent.parent()) else {
                continue;
            };
            let parent_name = parent_name.to_lowercase();

            debug!("parent: {parent_name}");

            for (suffix, color, category, no_mask_means_ignore_color) in masks.into_iter() {
                if parent_name.ends_with(suffix) {
                    *vis = Visibility::Hidden;

                    if let Some((_, wearable)) =
                        def.wearables.iter().find(|(_, w)| w.category == category)
                    {
                        debug!("setting {suffix} color {:?}", color);
                        // a mask without its texture is malformed: drawn as unmasked
                        if let (Some(mask), Some(texture)) =
                            (wearable.mask.as_ref(), wearable.texture.clone())
                        {
                            debug!("using mask for {suffix}");
                            let key = AvatarMaskKey::new(
                                texture.id(),
                                mask.id(),
                                color,
                                bounds_key.clone(),
                                config.graphics.oob,
                            );
                            let mask_material =
                                mask_cache.get_or_add(key, &mut mask_materials, || {
                                    MaskMaterial::new(
                                        color,
                                        texture,
                                        mask.clone(),
                                        &def.bounds,
                                        config.graphics.oob,
                                    )
                                });
                            commands
                                .entity(scene_ent)
                                .try_insert((
                                    MeshMaterial3d(mask_material),
                                    MeshTag(def.mask_tag()),
                                ))
                                .remove::<MeshMaterial3d<SceneMaterial>>();
                        } else {
                            debug!("no mask for {suffix}");
                            let base_color = if no_mask_means_ignore_color {
                                Color::WHITE
                            } else {
                                color
                            };
                            let key = AvatarMatKey::new(
                                None,
                                wearable.texture.as_ref().map(|t| t.id()),
                                base_color,
                                LinearRgba::BLACK,
                                0.0,
                                bounds_key.clone(),
                                config.graphics.oob,
                            );
                            let material =
                                mat_cache.get_or_add(key, &mut scene_materials, || SceneMaterial {
                                    base: StandardMaterial {
                                        base_color,
                                        base_color_texture: wearable.texture.clone(),
                                        alpha_mode: AlphaMode::Blend,
                                        ..Default::default()
                                    },
                                    extension: SceneBound::new(&def.bounds, config.graphics.oob)
                                        .with_base_opacity(AVATAR_DITHER_BASE_OPACITY)
                                        .with_capsule_dither(),
                                });
                            commands
                                .entity(scene_ent)
                                .try_insert((MeshMaterial3d(material), MeshTag(def.mask_tag())));
                        };
                        // blended either way
                        set_hover_glows(&mut commands.entity(scene_ent), false);
                        *vis = Visibility::Inherited;
                    }
                }
            }

            let hiders = [
                ("ubody_basemesh", WearableCategory::UPPER_BODY),
                ("lbody_basemesh", WearableCategory::LOWER_BODY),
                ("feet_basemesh", WearableCategory::FEET),
                // ("head", WearableCategory::HEAD),
                // ("head_basemesh", WearableCategory::HEAD),
                ("mask_eyes", WearableCategory::HEAD),
                ("mask_eyebrows", WearableCategory::HEAD),
                ("mask_mouth", WearableCategory::HEAD),
            ];

            for (hidename, category) in hiders {
                if parent_name.ends_with(hidename) {
                    // todo construct hides better so we don't need to scan the wearables here
                    if def.hides.contains(&category)
                        || def.wearables.iter().any(|(_, w)| {
                            w.category == WearableCategory::SKIN || w.category == category
                        })
                    {
                        *vis = Visibility::Hidden;
                    }
                }
            }
        }

        // a body that cannot be animated is rebuilt with the default body, instead of being
        // checked again every frame
        let unanimatable = armature_node.is_none() || target_armature_entities.is_empty();
        let (Some(armature_node), false) = (armature_node, unanimatable) else {
            warn!("body {} has no armature or bones", def.body_shape);
            abandon_build(
                &mut commands,
                avatar_ent,
                root_player_entity.parent(),
                WearableUrn::new(&def.body_shape).ok(),
                Vec::new(),
            );
            continue;
        };

        // the animator goes on only once the body is known to be animatable: a graph made for a
        // build that is abandoned would keep the player without its lower-body mask, which only a
        // fresh graph gets once the bone targets are known below
        let mut new_graph = None;
        if previous_animator.get(root_player_entity.parent()).is_err() {
            let mut player = AnimationPlayer::default();
            let mut graph = AnimationGraph::new();
            let mut clips = Clips::default();
            let mut transitions = AnimationTransitions::default();
            // play default idle anim to avoid t-posing
            if let Some(clip) = emote_loader
                .get_representation(EmoteUrn::new("Idle_Male").unwrap(), &def.body_shape)
                .ok()
                .and_then(|rep| rep.avatar_animation(&gltfs).ok())
                .flatten()
            {
                let ix = graph.add_clip(clip, 1.0, graph.root);
                clips.named.insert("Idle_Male".into(), (ix, 0.0));
                transitions.play(&mut player, ix, Duration::from_secs_f32(0.2));
            }
            let graph = graphs.add(graph);
            new_graph = Some(graph.clone());
            commands.entity(root_player_entity.parent()).try_insert((
                player,
                transitions,
                clips,
                AnimationGraphHandle(graph),
            ));
        }

        if let Some(emote) = emote {
            debug!("set emote -> {emote:?}");
            commands
                .entity(root_player_entity.parent())
                .try_insert(emote.clone());
        }

        // record the node with the animator
        commands
            .entity(root_player_entity.parent())
            .try_insert(AvatarAnimPlayer(root_player_entity.parent()));

        {
            // reparent hands
            if let Ok(attach_points) = attach_points.get(root_player_entity.parent()) {
                for (key, attach_point) in [
                    ("avatar_head", attach_points.head),
                    ("avatar_neck", attach_points.neck),
                    ("avatar_spine", attach_points.spine),
                    ("avatar_spine1", attach_points.spine_1),
                    ("avatar_spine2", attach_points.spine_2),
                    ("avatar_hips", attach_points.hip),
                    ("avatar_leftshoulder", attach_points.left_shoulder),
                    ("avatar_leftarm", attach_points.left_arm),
                    ("avatar_leftforearm", attach_points.left_forearm),
                    ("avatar_lefthand", attach_points.left_hand),
                    ("avatar_lefthandindex1", attach_points.left_hand_index),
                    ("avatar_rightshoulder", attach_points.right_shoulder),
                    ("avatar_rightarm", attach_points.righ_arm),
                    ("avatar_rightforearm", attach_points.right_forearm),
                    ("avatar_righthand", attach_points.right_hand),
                    ("avatar_righthandindex1", attach_points.right_hand_index),
                    ("avatar_leftupleg", attach_points.left_thigh),
                    ("avatar_leftleg", attach_points.left_shin),
                    ("avatar_leftfoot", attach_points.left_foot),
                    ("avatar_lefttoebase", attach_points.left_toe_base),
                    ("avatar_rightupleg", attach_points.right_thigh),
                    ("avatar_rightleg", attach_points.right_shin),
                    ("avatar_rightfoot", attach_points.right_foot),
                    ("avatar_righttoebase", attach_points.right_toe_base),
                ] {
                    reparent_attach_point(
                        &mut commands,
                        &target_armature_entities,
                        key,
                        attach_point,
                    );
                }
            } else {
                warn!("no attach points");
            }

            // add AnimationTargets
            for (bone, ent) in target_armature_entities.iter() {
                let mut path = VecDeque::default();
                let mut e = *ent;
                // a bone outside the armature (malformed body) is left unanimated
                let in_armature = loop {
                    let Ok((name, parent)) = name_and_parent.get(e) else {
                        break false;
                    };
                    path.push_front(name);
                    if name.to_lowercase() == "armature" {
                        break true;
                    }
                    e = parent.parent();
                };
                if !in_armature {
                    warn!(
                        "bone {bone} of body {} is outside its armature",
                        def.body_shape
                    );
                    continue;
                }
                let id = AnimationTargetId::from_names(path.into_iter());

                // hips and legs stay with locomotion under an upper-body emote (`MaskedEmote`)
                if LOWER_BODY_BONES.contains(&bone.as_str()) {
                    if let Some(graph) = new_graph.as_ref().and_then(|graph| graphs.get_mut(graph))
                    {
                        graph.add_target_to_mask_group(id, LOWER_BODY_MASK_GROUP);
                    }
                }

                commands.entity(*ent).try_insert(AnimationTarget {
                    id,
                    player: root_player_entity.parent(),
                });
            }

            commands.entity(armature_node).try_insert(AnimationTarget {
                id: AnimationTargetId::from_name(&Name::new("Armature")),
                player: root_player_entity.parent(),
            });
        }

        // color the components of wearables
        // wearable_instances is built from the wearables with models, in order
        let wearable_models = def.wearables.iter().filter_map(|(_, w)| w.model.as_ref());
        let mut spring_chains = Vec::new();
        let mut moved_branches = HashSet::new();
        for (instance, model) in loaded_avatar.wearable_instances.iter().zip(wearable_models) {
            let Some(instance) = instance else {
                warn!("failed to load instance for wearable");
                continue;
            };

            let mut armature_map = HashMap::new();
            // defer despawn until after we've reparented any springbones
            let mut wearable_armature_to_despawn = HashSet::new();

            for scene_ent in scene_spawner.iter_instance_entities(*instance) {
                let Ok((_, parent, gt, maybe_h_mat, maybe_h_mesh, maybe_player)) =
                    instance_ents.get_mut(scene_ent)
                else {
                    continue;
                };

                if maybe_player.is_some() {
                    commands.entity(scene_ent).remove::<AnimationPlayer>();
                }

                let Ok(parent_name) = named_ents.get(parent.parent()) else {
                    continue;
                };
                let parent_name = parent_name.to_lowercase();

                let Ok(name) = named_ents.get(scene_ent) else {
                    continue;
                };
                let name = name.to_lowercase();

                // record bone entities so we can remap them, and delete this instance
                if name.to_lowercase().starts_with("avatar_") {
                    if let Some(target) = target_armature_entities.get(&name.to_lowercase()) {
                        armature_map.insert(scene_ent, target);
                    }
                    if parent_name == "armature" {
                        wearable_armature_to_despawn.insert(scene_ent);
                    }
                    continue;
                }

                // move children of the root to the body mesh
                if parent_name.to_lowercase() == "armature" {
                    commands
                        .entity(scene_ent)
                        .try_insert(ChildOf(armature_node));
                }

                if let Some(h_mesh) = maybe_h_mesh {
                    upload_cpu_mesh(&mut meshes, h_mesh);
                    meshes_top = meshes_top.max(top_above_head(
                        scene_ent,
                        h_mesh,
                        &skins,
                        &named_ents,
                        &inverse_bindposes,
                        &meshes,
                    ));
                    commands.entity(scene_ent).try_insert(calculate_local_aabb(
                        aabb_bounds.0,
                        aabb_bounds.1,
                        gt,
                    ));
                }

                if let Some(h_mat) = maybe_h_mat {
                    commands
                        .entity(scene_ent)
                        .remove::<MeshMaterial3d<StandardMaterial>>();

                    if let Some(mat) = standard_materials.get(h_mat) {
                        let instance_mat = instance_scene_materials
                            .entry(h_mat.clone_weak())
                            .or_insert_with(|| {
                                derived_scene_material(
                                    mat,
                                    h_mat,
                                    def,
                                    loaded_avatar,
                                    mat.depth_bias,
                                    &bounds_key,
                                    config.graphics.oob,
                                    &mut mat_cache,
                                    &mut scene_materials,
                                )
                            });
                        let mut mesh = commands.entity(scene_ent);
                        mesh.try_insert((
                            MeshMaterial3d(instance_mat.clone()),
                            MeshTag(def.model_tag(Some(model))),
                        ));
                        set_hover_glows(
                            &mut mesh,
                            hover_glows(Some(model)) && can_glow(mat.alpha_mode),
                        );
                    }
                }
            }

            // the joints of all the wearable's skins say which of its bones swing. The chains
            // follow the bones' parents, so the order the meshes and joints come in is no matter.
            let mut seen = HashSet::new();
            let skin_joints: Vec<Entity> = scene_spawner
                .iter_instance_entities(*instance)
                .filter_map(|scene_ent| skins.get(scene_ent).ok())
                .flat_map(|skin| skin.joints.iter().copied())
                .filter(|joint| seen.insert(*joint))
                .collect();
            spring_chains.extend(spring_bones::wearable_chains(
                &skin_joints,
                &name_and_parent,
                |joint| armature_map.get(&joint).map(|target| **target),
                |joint| {
                    local_transforms
                        .get(joint)
                        .map(|(transform, _)| transform.rotation)
                        .ok()
                },
                model.spring_bones.as_deref(),
            ));

            // remap bones
            for scene_ent in scene_spawner.iter_instance_entities(*instance) {
                if let Ok(mut skin) = skins.get_mut(scene_ent) {
                    for joint in &mut skin.joints {
                        if let Some(body_bone) = armature_map.get(joint) {
                            *joint = **body_bone;
                            continue;
                        }
                        // a bone of the wearable's own: keep the branch it is on
                        let Some(branch) = wearable_branch(
                            *joint,
                            &name_and_parent,
                            &named_ents,
                            &target_armature_entities,
                        ) else {
                            continue;
                        };
                        // once for a branch, however many joints and meshes lead to it
                        if !moved_branches.insert(branch.top) {
                            continue;
                        }
                        let under = branch.body_bone.unwrap_or_else(|| {
                            // an `avatar_*` node under the wearable's armature was set to be
                            // despawned: keep it, as the skin uses a bone under it
                            wearable_armature_to_despawn.remove(&branch.top);
                            armature_node
                        });
                        commands.entity(branch.top).try_insert(ChildOf(under));
                        if branch.body_lacks_top {
                            warn!(
                                "the body has no bone {:?}: the wearable's bones under it stay rigid under {:?}",
                                branch.name, branch.parent_name
                            );
                        }
                    }
                }
            }

            // despawn wearable's own armature
            for scene_ent in wearable_armature_to_despawn {
                commands.entity(scene_ent).despawn();
            }
        }
        commands
            .entity(avatar_ent)
            .try_insert(spring_bones::SpringChains(spring_chains));

        let wearable_models = def
            .wearables
            .iter()
            .filter(|(_, w)| w.model.is_some())
            .count();
        let wearable_texs = def
            .wearables
            .iter()
            .filter(|(_, w)| w.model.is_none())
            .count();

        debug!(
            "avatar processed, 1+{} models, {} textures. hides: {:?}, skin mats: {:?}, hair mats: {:?}, used mats: {:?}",
            wearable_models, wearable_texs, def.hides, loaded_avatar.skin_materials.len(), loaded_avatar.hair_materials.len(), instance_scene_materials.len()
        );

        commands
            .entity(avatar_ent)
            .try_insert((AvatarProcessed, Visibility::Inherited));

        // the head bone's units, in the avatar's own: the nametag's anchor applies a scene's
        // scale of the avatar, which the local transforms below the avatar leave out. A scene
        // may hold the avatar at a scale of nothing while it is built
        #[cfg(feature = "explorer")]
        let head_scale = target_armature_entities
            .get("avatar_head")
            .and_then(|head| scale_below(*head, avatar_ent, &local_transforms));
        commands
            .entity(root_player_entity.parent())
            .try_insert(AvatarMaterials(
                instance_scene_materials.values().map(|h| h.id()).collect(),
            ));
        #[cfg(feature = "explorer")]
        commands
            .entity(root_player_entity.parent())
            .try_insert(NametagHeadOffset::above_head(
                head_scale.map_or(f32::NEG_INFINITY, |scale| meshes_top * scale),
            ));

        // remove previous
        if let Ok(prev) = previous_avatar.get(root_player_entity.parent()) {
            debug!("new {avatar_ent:?} done, remove prev -> {prev:?}");
            if let Ok(mut commands) = commands.get_entity(prev.0) {
                commands.despawn();
            }
            commands
                .entity(root_player_entity.parent())
                .remove::<PreviousAvatar>();
        }

        // announce state
        reports.write(AvatarLoadReport {
            owner: root_player_entity.parent(),
            body_shape: def.body_shape.clone(),
            state: LoadingState::Finished,
        });
    }
    *run = processing_timing::AvatarProcessingRun {
        spent: started.elapsed(),
        processed,
    };
}

// derive the scene material for an avatar's gltf material, sharing the asset
// with every other avatar that derives an identical material so they can batch
fn derived_scene_material(
    mat: &StandardMaterial,
    h_mat: &MeshMaterial3d<StandardMaterial>,
    def: &AvatarRenderDefinition,
    loaded_avatar: &AvatarLoaded,
    depth_bias: f32,
    bounds_key: &BoundsBits,
    oob: f32,
    mat_cache: &mut AssetCache<AvatarMatKey, SceneMaterial>,
    scene_materials: &mut Assets<SceneMaterial>,
) -> Handle<SceneMaterial> {
    let base_color = if loaded_avatar.skin_materials.contains(&h_mat.0) {
        def.skin_color
    } else if loaded_avatar.hair_materials.contains(&h_mat.0) {
        def.hair_color
    } else {
        mat.base_color
    };

    // the emissive is the glTF factor alone (drop_emissive_strengths)
    let emissive_color_specified =
        mat.emissive.red != 0.0 || mat.emissive.green != 0.0 || mat.emissive.blue != 0.0;
    let emissive = if emissive_color_specified {
        LinearRgba {
            red: avatar_emission(mat.emissive.red),
            green: avatar_emission(mat.emissive.green),
            blue: avatar_emission(mat.emissive.blue),
            alpha: mat.emissive.alpha,
        }
    } else {
        mat.emissive
    };

    let key = AvatarMatKey::new(
        Some(h_mat.0.id()),
        mat.base_color_texture.as_ref().map(|t| t.id()),
        base_color,
        emissive,
        depth_bias,
        bounds_key.clone(),
        oob,
    );
    mat_cache.get_or_add(key, scene_materials, || SceneMaterial {
        base: StandardMaterial {
            base_color,
            emissive,
            depth_bias,
            // every avatar material culls its back faces, double-sided in the glTF or not: a
            // blended skirt does not show its inside through its front, and a mesh modelled with
            // back-to-back faces draws one layer, not two. A mirrored node's faces wind the other
            // way, so its copy of the material culls the front-wound ones
            cull_mode: Some(if is_mirrored_material(&h_mat.0) {
                Face::Front
            } else {
                Face::Back
            }),
            double_sided: false,
            ..mat.clone()
        },
        extension: SceneBound::new(&def.bounds, oob)
            .with_base_opacity(AVATAR_DITHER_BASE_OPACITY)
            .with_capsule_dither(),
    })
}

fn reparent_attach_point(
    commands: &mut Commands,
    target_armature_entities: &HashMap<String, Entity>,
    key: &str,
    attach_point: Entity,
) {
    if let Some(bone) = target_armature_entities.get(key) {
        commands.entity(*bone).try_push_children(&[attach_point]);
    } else {
        #[cfg(not(debug_assertions))]
        warn!("no {}", key,);
        #[cfg(debug_assertions)]
        warn!(
            "no {}, available: {:#?}",
            key,
            target_armature_entities.keys()
        );
    }
}

/// How far up a wearable's own bones a branch is followed to the body bone it hangs off.
const MAX_BRANCH_DEPTH: usize = 64;

/// A branch of a wearable's own bones that a skin joint is on, and where it moves to stay on
/// the avatar.
struct WearableBranch<'a> {
    /// the topmost node of the branch
    top: Entity,
    name: &'a Name,
    /// the body's bone the branch moves under, or nothing for the body's armature
    body_bone: Option<Entity>,
    /// the name of the node `top` hangs off in the wearable, in lower case
    parent_name: String,
    /// `top` is an `avatar_*` bone the body lacks. The bones under it stay rigid: no spring
    /// chain hangs off that bone.
    body_lacks_top: bool,
}

/// The branch to move for `joint`, a skin joint of a wearable that is no bone of the body. The
/// branch runs up from the joint to the node under a bone the body has, and moves under that
/// bone. A branch that starts at an `avatar_*` bone the body lacks ends the same way, or under
/// the body's armature when that bone sits directly under the wearable's. Nothing for a branch
/// that ends at another node under the wearable's armature, which has moved to the body's
/// already, and for a joint that leads to neither.
fn wearable_branch<'a>(
    joint: Entity,
    nodes: &'a Query<(&Name, &ChildOf)>,
    names: &Query<&Name>,
    body_bones: &HashMap<String, Entity>,
) -> Option<WearableBranch<'a>> {
    let mut top = joint;
    for _ in 0..MAX_BRANCH_DEPTH {
        let (name, parent) = nodes.get(top).ok()?;
        let parent_name = names.get(parent.parent()).ok()?.to_lowercase();
        let body_bone = body_bones.get(&parent_name).copied();
        if body_bone.is_some() || parent_name == "armature" {
            let body_lacks_top = name.to_lowercase().starts_with("avatar_");
            return (body_bone.is_some() || body_lacks_top).then_some(WearableBranch {
                top,
                name,
                body_bone,
                parent_name,
                body_lacks_top,
            });
        }
        top = parent.parent();
    }
    None
}

/// The height scale of `entity` in the units of its ancestor `root`: its own and that of each
/// entity between them. Nothing for an entity not below `root`.
#[cfg(any(feature = "explorer", test))]
fn scale_below(
    entity: Entity,
    root: Entity,
    transforms: &Query<(&Transform, &ChildOf)>,
) -> Option<f32> {
    let mut scale = 1.0;
    let mut current = entity;
    while current != root {
        let (transform, parent) = transforms.get(current).ok()?;
        scale *= transform.scale.y;
        current = parent.parent();
    }
    Some(scale)
}

/// How far a skinned mesh reaches above the head bone, along that bone and in its units, read
/// from the skin's bind pose. Nothing for a mesh not skinned to a head bone.
fn top_above_head(
    mesh_ent: Entity,
    mesh: &Mesh3d,
    skins: &Query<&mut SkinnedMesh>,
    names: &Query<&Name>,
    inverse_bindposes: &Assets<SkinnedMeshInverseBindposes>,
    meshes: &Assets<Mesh>,
) -> f32 {
    let top = || {
        let skin = skins.get(mesh_ent).ok()?;
        let head = skin.joints.iter().position(|joint| {
            names
                .get(*joint)
                .is_ok_and(|name| name.eq_ignore_ascii_case("avatar_head"))
        })?;
        let from_mesh = *inverse_bindposes.get(&skin.inverse_bindposes)?.get(head)?;
        let aabb = meshes.get(mesh)?.compute_aabb()?;
        let (center, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
        (0..8)
            .map(|corner| {
                let sign = Vec3::new(
                    if corner & 1 == 0 { -1.0 } else { 1.0 },
                    if corner & 2 == 0 { -1.0 } else { 1.0 },
                    if corner & 4 == 0 { -1.0 } else { 1.0 },
                );
                from_mesh.transform_point3(center + half * sign).y
            })
            .reduce(f32::max)
    };
    top().unwrap_or(f32::NEG_INFINITY)
}

fn calculate_local_aabb(
    global_min: Vec3,
    global_max: Vec3,
    global_transform: &GlobalTransform,
) -> Aabb {
    let transform_matrix = global_transform.compute_matrix();
    let inverse_transform = transform_matrix.inverse();

    let corners = [
        Vec3::new(global_min.x, global_min.y, global_min.z),
        Vec3::new(global_max.x, global_min.y, global_min.z),
        Vec3::new(global_min.x, global_max.y, global_min.z),
        Vec3::new(global_max.x, global_max.y, global_min.z),
        Vec3::new(global_min.x, global_min.y, global_max.z),
        Vec3::new(global_max.x, global_min.y, global_max.z),
        Vec3::new(global_min.x, global_max.y, global_max.z),
        Vec3::new(global_max.x, global_max.y, global_max.z),
    ];

    let mut local_min = Vec3::splat(f32::MAX);
    let mut local_max = Vec3::splat(f32::MIN);

    for corner in corners {
        let local_point = inverse_transform.transform_point3(corner);
        local_min = local_min.min(local_point);
        local_max = local_max.max(local_point);
    }

    let center = (local_min + local_max) * 0.5;
    let half_extents = (local_max - local_min) * 0.5;

    Aabb {
        center: center.into(),
        half_extents: half_extents.into(),
    }
}

/// On a player whose avatar is hidden for being farther than the `max_avatars` nearest.
#[derive(Component)]
pub struct BeyondMaxAvatars;

fn set_avatar_visibility(
    mut commands: Commands,
    mut q: Query<
        (
            &GlobalTransform,
            &mut Visibility,
            Option<&RenderLayers>,
            &ChildOf,
        ),
        With<AvatarProcessed>,
    >,
    beyond_max: Query<(), With<BeyondMaxAvatars>>,
    shown: Query<&InheritedVisibility>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    config: Res<AppConfig>,
    session: Res<SessionGraphics>,
) {
    let Ok(player_pos) = player.single().map(|gt| gt.translation()) else {
        return;
    };
    let max_avatars = session.max_avatars(&config);

    // an avatar whose player is hidden for another reason (a block or ban, an avatar modifier
    // area, its scene) draws nothing, so it takes no slot from one that would show. The cap
    // itself hides the build, below its player, so it never feeds back into this
    let counts = |player: Entity| shown.get(player).is_ok_and(|shown| shown.get());
    let default_layer = RenderLayers::layer(0);
    let mut distances = q
        .iter()
        .filter(|(_, _, maybe_layer, player)| {
            maybe_layer.is_none_or(|layer| layer.intersects(&default_layer))
                && counts(player.parent())
        })
        .map(|(t, ..)| (t.translation() - player_pos).length_squared())
        .collect::<Vec<_>>();
    // only the (max_avatars)-th distance matters; partition instead of sorting
    let cutoff = if distances.len() > max_avatars {
        *distances
            .select_nth_unstable_by(max_avatars, |a, b| {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less)
            })
            .1
    } else {
        f32::MAX
    };

    for (t, mut vis, maybe_layer, player) in q.iter_mut() {
        let is_root_layer = maybe_layer.is_none_or(|layer| layer.intersects(&default_layer));
        let new_vis = if is_root_layer && (t.translation() - player_pos).length_squared() >= cutoff
        {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        // avoid dirtying Visibility (and the propagation pass) when unchanged
        vis.set_if_neq(new_vis);
        let hidden = new_vis == Visibility::Hidden;
        if hidden != beyond_max.contains(player.parent()) {
            let mut player = commands.entity(player.parent());
            if hidden {
                player.try_insert(BeyondMaxAvatars);
            } else {
                player.try_remove::<BeyondMaxAvatars>();
            }
        }
    }
}

#[cfg(feature = "explorer")]
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_dump_avatar")]
struct DebugDumpAvatar;

#[cfg(feature = "explorer")]
fn debug_dump_avatar(
    mut input: ConsoleCommand<DebugDumpAvatar>,
    player: Query<&AvatarShape, With<PrimaryUser>>,
    ipfas: IpfsAssetServer,
    entity_definitions: Res<Assets<EntityDefinition>>,
    mut tasks: Local<Vec<Task<()>>>,
    console_relay: Res<ConsoleRelay>,
    mut wearables: CollectibleManager<Wearable>,
    mut emotes: CollectibleManager<Emote>,
    mut store: Local<HashSet<Handle<EntityDefinition>>>,
) {
    if let Some(Ok(_)) = input.take() {
        let Ok(shape) = player.single() else {
            return;
        };

        let wearable_hashes: Vec<String> = shape
            .0
            .wearables
            .iter()
            .flat_map(|w| WearableUrn::new(w).ok())
            .flat_map(|wearable| wearables.get_hash(wearable).ok())
            .collect();

        let emote_hashes: Vec<_> = shape
            .0
            .emotes
            .iter()
            .inspect(|e| {
                info!("0 {e:?}");
            })
            .flat_map(|e| EmoteUrn::new(e).ok())
            .inspect(|e| {
                info!("1 {e:?}");
            })
            .flat_map(|emote| emotes.get_hash(emote).ok())
            .inspect(|e| {
                info!("2 {e:?}");
            })
            .collect();

        for (scene_hash, folder) in wearable_hashes
            .into_iter()
            .zip(std::iter::repeat("wearables"))
            .chain(emote_hashes.into_iter().zip(std::iter::repeat("emotes")))
        {
            let h_scene = ipfas.load_hash::<EntityDefinition>(&scene_hash);
            let Some(def) = entity_definitions.get(&h_scene) else {
                input.reply_failed("can't resolve wearable handle - try again in a few seconds");
                store.insert(h_scene);
                continue;
            };

            let dump_folder = ipfas
                .ipfs()
                .cache_path()
                .to_owned()
                .unwrap()
                .join("scene_dump")
                .join(folder)
                .join(&scene_hash);
            std::fs::create_dir_all(&dump_folder).unwrap();

            // total / succeed / fail
            let count = std::sync::Arc::new(std::sync::Mutex::new((0, 0, 0)));

            for content_file in def.content.files() {
                count.lock_ignore_poison().0 += 1;
                let ipfs_path = IpfsPath::new(IpfsType::new_content_file(
                    scene_hash.to_owned(),
                    content_file.to_owned(),
                ));

                let path = PathBuf::from(&ipfs_path);

                let ipfs = ipfas.ipfs().clone();
                let content_file = content_file.clone();
                let dump_folder = dump_folder.clone();
                let count = count.clone();
                let send = console_relay.send.clone();
                tasks.push(IoTaskPool::get().spawn(async move {
                    let report = |fail: Option<String>| {
                        let mut count = count.lock_ignore_poison();
                        if let Some(fail) = fail {
                            count.2 += 1;
                            let _ = send.send(fail);
                        } else {
                            count.1 += 1;
                        }
                        if count.0 == count.1 + count.2 {
                            if count.2 == 0 {
                                let _ = send.send(format!("[ok] {} files downloaded", count.0));
                            } else {
                                let _ = send.send(format!(
                                    "[failed] {}/{} files downloaded",
                                    count.1, count.0
                                ));
                            }
                        }
                    };

                    let Ok(mut reader) = ipfs.read(&path).await else {
                        report(Some(format!(
                            "{content_file} failed: couldn't load bytes\n"
                        )));
                        return;
                    };
                    let mut bytes = Vec::default();
                    if let Err(e) = reader.read_to_end(&mut bytes).await {
                        report(Some(format!("{content_file} failed: {e}")));
                        return;
                    }

                    // the key is the deployer's string and may still carry `..`
                    let Some(file) = dump_folder.join_relative(&content_file) else {
                        report(Some(format!(
                            "{content_file} failed: escapes the dump folder"
                        )));
                        return;
                    };
                    if let Some(parent) = file.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            report(Some(format!(
                                "{content_file} failed: couldn't create parent: {e}"
                            )));
                            return;
                        }
                    }
                    if let Err(e) = std::fs::write(file, bytes) {
                        report(Some(format!("{content_file} failed: {e}")));
                        return;
                    }

                    report(None);
                }));
            }

            input.reply(format!(
                "scene hash {}, downloading {} files",
                scene_hash,
                tasks.len()
            ));
        }
    }

    tasks.retain_mut(|t| t.complete().is_none());
}

fn add_attach_points_to_avatar_shape(trigger: Trigger<OnAdd, AvatarShape>, mut commands: Commands) {
    let entity = trigger.target();

    let attach_points = AttachPoints::new(&mut commands);

    commands
        .entity(entity)
        .try_push_children(&attach_points.entities())
        .try_insert(attach_points);
}

fn remove_attach_points_from_avatar_shape(
    trigger: Trigger<OnRemove, AvatarShape>,
    mut commands: Commands,
    attach_points_query: Query<&AttachPoints>,
) {
    let entity = trigger.target();

    if let Ok(attach_points) = attach_points_query.get(entity) {
        for attach_point in attach_points.entities() {
            commands.entity(attach_point).try_despawn();
        }
        // the offset was measured on the body that goes with the shape: the next body's
        // nametag waits for its own
        commands.entity(entity).try_remove::<AttachPoints>();
        #[cfg(feature = "explorer")]
        commands.entity(entity).try_remove::<NametagHeadOffset>();
    }
}

/// Send a mesh loaded cpu-side only (a scene's glb settings) to the GPU as well.
pub(crate) fn upload_cpu_mesh(meshes: &mut Assets<Mesh>, mesh: impl Into<AssetId<Mesh>>) {
    let id = mesh.into();
    if meshes
        .get(id)
        .is_some_and(|mesh| mesh.asset_usage == RenderAssetUsages::MAIN_WORLD)
    {
        if let Some(mesh) = meshes.get_mut(id) {
            mesh.normalize_joint_weights();
            mesh.asset_usage |= RenderAssetUsages::RENDER_WORLD;
        }
    }
}

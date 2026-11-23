//! The plate a scene puts over an avatar's name with `AvatarNametag`: one line of the scene's
//! words in the scene's colours.
//!
//! Only the scene the local player stands in plates avatars: the local player, the other
//! players standing in it, and the avatars it made. While any scene asks for a plate, the
//! plates are worked out again every frame from what the local player's scene holds now, so
//! one goes with its component, its entity, its scene, or the player who walks out.

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use common::structs::PrimaryUser;
use comms::global_crdt::ForeignPlayer;
use dcl_component::{
    proto_components::{common::Color3, sdk::components::PbAvatarNametag, Color3DclToBevy},
    SceneEntityId,
};
use scene_runner::{ContainingScene, SceneEntity};

use super::{bubble::capped, NametagVoice, BACKGROUND};
use crate::{dynamic_nametag::NametagHeadOffset, AvatarSelection, AvatarShape};

/// The most characters of a scene's label a plate shows.
pub(super) const MAX_PLATE_CHARS: usize = 128;
const LABEL_COLOR: Color = Color::srgb_u8(0xFC, 0xFC, 0xFC);

/// On an avatar: the plate over its name.
#[derive(Component, Clone, Debug, PartialEq)]
#[require(NametagVoice)]
pub(super) struct NametagPlate {
    /// One line; empty for a plate with nothing written on it.
    pub(super) label: String,
    pub(super) label_color: Color,
    pub(super) background: Color,
    pub(super) border: Color,
}

/// A scene's colour as a display colour. A channel that is not a number reads 0.
fn display(color: Color3) -> Color {
    let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
    Color3 {
        r: finite(color.r),
        g: finite(color.g),
        b: finite(color.b),
    }
    .convert_srgb()
}

impl From<&PbAvatarNametag> for NametagPlate {
    fn from(scene: &PbAvatarNametag) -> Self {
        let background = scene.background_color.map_or(BACKGROUND, display);
        Self {
            label: capped(&scene.label, MAX_PLATE_CHARS),
            label_color: scene.label_color.map_or(LABEL_COLOR, display),
            background,
            // no border of its own: the plate reads as one fill
            border: scene.border_color.map_or(background, display),
        }
    }
}

/// On a scene's entity: the plate the scene asks for, for the avatar the entity stands for.
#[derive(Component, Clone, Debug)]
pub(super) struct AvatarNametag(pub(super) NametagPlate);

impl From<PbAvatarNametag> for AvatarNametag {
    fn from(scene: PbAvatarNametag) -> Self {
        Self(NametagPlate::from(&scene))
    }
}

/// A scene entity that asks for a plate, and whether it is an avatar the scene made, with a
/// body that is built.
pub(super) type Plated<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SceneEntity,
        &'static AvatarNametag,
        Has<AvatarShape>,
        Has<AvatarSelection>,
        Has<NametagHeadOffset>,
    ),
>;

/// Whether a scene's entity with this id stands for a player. Such an entity never plates an
/// avatar of the scene's own, whatever else it carries.
fn is_player_id(id: SceneEntityId) -> bool {
    id.id == SceneEntityId::PLAYER.id || SceneEntityId::FOREIGN_PLAYER_RANGE.contains(&id.id)
}

/// Give every avatar the plate the scene the local player stands in asks for it, and take the
/// plate off every other. `others` are the other players, by the id a scene knows them by:
/// they are read only when that scene asks for a plate on one of them.
pub(super) fn plate_avatars(
    commands: &mut Commands,
    containing_scene: &ContainingScene,
    local: Option<Entity>,
    others: impl Iterator<Item = (Entity, SceneEntityId)>,
    plated: &Plated,
    holders: &Query<(Entity, &NametagPlate)>,
    players: &mut HashMap<SceneEntityId, Entity>,
    owners: &mut HashSet<Entity>,
) {
    owners.clear();
    // the parcel's scene alone: a portable experience covers every parcel and plates no one
    if let Some(current) = local.and_then(|local| containing_scene.get_parcel(local)) {
        let mut others = Some(others);
        for (entity, scene_entity, wanted, shape, selection, built) in plated {
            if scene_entity.root != current {
                continue;
            }
            let owner = if scene_entity.id == SceneEntityId::PLAYER {
                local
            } else if is_player_id(scene_entity.id) {
                // the first plate on another player: who stands in this scene
                if let Some(others) = others.take() {
                    players.clear();
                    players.extend(
                        others
                            .filter(|(player, _)| {
                                containing_scene.get_parcel(*player) == Some(current)
                            })
                            .map(|(player, id)| (id, player)),
                    );
                }
                players.get(&scene_entity.id).copied()
            } else {
                (shape && selection && built).then_some(entity)
            };
            let Some(owner) = owner else {
                continue;
            };
            owners.insert(owner);
            if holders
                .get(owner)
                .map_or(true, |(_, plate)| *plate != wanted.0)
            {
                commands.entity(owner).try_insert(wanted.0.clone());
            }
        }
    }
    for (holder, _) in holders {
        if !owners.contains(&holder) {
            commands.entity(holder).try_remove::<NametagPlate>();
        }
    }
}

pub(super) fn resolve_plates(
    mut commands: Commands,
    containing_scene: ContainingScene,
    local: Query<Entity, With<PrimaryUser>>,
    others: Query<(Entity, &ForeignPlayer)>,
    plated: Plated,
    holders: Query<(Entity, &NametagPlate)>,
    mut players: Local<HashMap<SceneEntityId, Entity>>,
    mut owners: Local<HashSet<Entity>>,
) {
    if plated.is_empty() && holders.is_empty() {
        return;
    }
    plate_avatars(
        &mut commands,
        &containing_scene,
        local.single().ok(),
        others
            .iter()
            .map(|(player, foreign)| (player, foreign.scene_id)),
        &plated,
        &holders,
        &mut players,
        &mut owners,
    );
}

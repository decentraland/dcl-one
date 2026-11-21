//! `/get_reticle`: what the pointer ray reaches now, as a person aiming would find it, read from the
//! pointer pipeline without pressing or hovering anything:
//!
//! `{"available", "cursorLocked", "rayFromScreenCenter", "aimedByCommand", "pointerLocked",
//! "cursor": {"x", "y", "normalized", "onScreen"}, "screen", "ray": {"origin", "direction"}, "kind",
//! "crdtEntityId", "scene", "hitPoint", "distanceToPlayer", "distanceToCamera", "hasPointerEvents",
//! "withinRange", "hoverQualified", "hoverText", "entries"}`

use bevy::{
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_console::ConsoleCommand;
use common::structs::{CursorLocks, PointerTargetType};
use console::{reply, DoAddConsoleCommand};
use dcl_component::transform_and_parent::DclTranslation;
use scene_runner::{
    renderer_context::RendererSceneContext,
    update_scene::{
        pointer_results::{PointerRay, PointerTarget},
        synthetic_pointer::{hover_entries, SyntheticPointer},
    },
    update_world::pointer_events::PointerEvents,
    ContainerEntity, SceneEntity,
};
use serde_json::{json, Value};

use super::pointer::hit_ids;

pub(crate) struct AutomationReticlePlugin;

impl Plugin for AutomationReticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<GetReticleCommand, _>(get_reticle_cmd);
    }
}

/// What the pointer ray reaches now, as JSON: the cursor it goes through (the screen centre while
/// the cursor is locked), the ray, the kind of thing it reaches (scene_entity, ui, avatar, world or
/// nothing), its crdtEntityId, hitPoint and distances, and whether the cursor's pointer events on
/// it are within range (withinRange), a hover would fire (hoverQualified) and its hoverText.
/// Presses and hovers nothing
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/get_reticle")]
pub struct GetReticleCommand;

/// A bevy world point in Decentraland world coordinates, as `/world_state` reports them.
fn point(v: Vec3) -> Value {
    let [x, y, z] = DclTranslation::from_bevy_translation(v).0;
    json!({ "x": reply::round(x), "y": reply::round(y), "z": reply::round(z) })
}

/// The kind of thing the ray reaches: an entity of a scene, a scene's UI, an avatar, something that
/// belongs to no scene, or nothing.
fn kind(ty: Option<PointerTargetType>, in_scene: bool) -> &'static str {
    match ty {
        None => "nothing",
        Some(PointerTargetType::Ui) => "ui",
        Some(PointerTargetType::Avatar) => "avatar",
        Some(PointerTargetType::World) if in_scene => "scene_entity",
        Some(PointerTargetType::World) => "world",
    }
}

#[allow(clippy::too_many_arguments)]
fn get_reticle_cmd(
    mut input: ConsoleCommand<GetReticleCommand>,
    windows: Query<&Window, With<PrimaryWindow>>,
    target: Res<PointerTarget>,
    ray: Res<PointerRay>,
    synthetic: Res<SyntheticPointer>,
    locks: Res<CursorLocks>,
    ids: Query<(Option<&SceneEntity>, Option<&ContainerEntity>)>,
    pointer_events: Query<&PointerEvents>,
    scenes: Query<&RendererSceneContext>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    let Ok(window) = windows.single() else {
        input.reply_failed("no primary window");
        return;
    };
    let screen = reply::Screen::of(window);
    let locked = window.cursor_options.grab_mode == CursorGrabMode::Locked;
    let aimed = synthetic.0.is_some();
    let cursor = if locked {
        Some(Vec2::new(screen.width, screen.height) / 2.0)
    } else {
        window.cursor_position()
    };
    let cursor = cursor.map(|pixels| {
        let normalized = screen.to_normalized(pixels);
        json!({
            "x": reply::round(pixels.x),
            "y": reply::round(pixels.y),
            "normalized": {
                "x": reply::round_fine(normalized.x),
                "y": reply::round_fine(normalized.y),
            },
            "onScreen": (0.0..=1.0).contains(&normalized.x) && (0.0..=1.0).contains(&normalized.y),
        })
    });
    let ray_json = ray.0.map(|ray| {
        let d = *ray.direction;
        json!({
            "origin": point(ray.origin),
            "direction": {
                "x": reply::round_fine(d.x),
                "y": reply::round_fine(d.y),
                "z": reply::round_fine(-d.z),
            },
        })
    });

    let info = target.0.as_ref();
    let hit = info.and_then(|info| hit_ids(info.container, &ids));
    let entries = match (
        info,
        info.and_then(|i| pointer_events.get(i.container).ok()),
    ) {
        (Some(info), Some(pe)) => hover_entries(info, pe),
        _ => Vec::new(),
    };
    let hover_text = entries
        .iter()
        .filter(|e| e.enabled)
        .find_map(|e| e.hover_text.clone());
    let reply = json!({
        "available": ray.0.is_some(),
        "cursorLocked": locked,
        "rayFromScreenCenter": locked && !aimed,
        "aimedByCommand": aimed,
        "pointerLocked": locks.0.contains("camera"),
        "cursor": cursor,
        "screen": screen.to_json(),
        "ray": ray_json,
        "kind": kind(info.map(|i| i.ty), info.is_some_and(|i| i.in_scene)),
        "crdtEntityId": hit.and_then(|(entity, _, _)| entity.as_proto_u32()),
        "scene": hit.and_then(|(_, _, root)| scenes.get(root).ok()).map(|ctx| ctx.hash.to_string()),
        "hitPoint": info.and_then(|i| i.position).map(point),
        "distanceToPlayer": info.map(|i| reply::round(i.distance.0)),
        "distanceToCamera": info.map(|i| reply::round(i.camera_distance.0)),
        "hasPointerEvents": !entries.is_empty(),
        "withinRange": entries.iter().any(|e| e.enabled),
        "hoverQualified": entries.iter().any(|e| e.enabled && e.event == "PetHoverEnter"),
        "hoverText": hover_text,
        "entries": entries,
    });
    input.reply_ok(reply.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_names_what_the_ray_reaches() {
        assert_eq!(kind(None, false), "nothing");
        assert_eq!(kind(Some(PointerTargetType::World), true), "scene_entity");
        assert_eq!(kind(Some(PointerTargetType::World), false), "world");
        assert_eq!(kind(Some(PointerTargetType::Ui), true), "ui");
        assert_eq!(kind(Some(PointerTargetType::Avatar), false), "avatar");
    }
}

use bevy::{
    app::Propagate,
    ecs::entity::{EntityHashMap, EntityHashSet},
    math::FloatOrd,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use common::{
    inputs::InputMap,
    structs::{PointerTargetType, ToolTips, TooltipSource},
};
use comms::global_crdt::ForeignPlayer;
use dcl_component::{
    proto_components::sdk::components::{
        common::InputAction,
        pb_pointer_events::{Entry, Info},
        PbPointerEvents,
    },
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;
use scene_material::highlight::{
    Highlight, HIGHLIGHT_INVALID, HIGHLIGHT_VALID, HIGHLIGHT_WIDTH_PX,
};

use crate::{
    renderer_context::RendererSceneContext,
    update_scene::pointer_results::{
        event_category, passes_distance_check, resolve_action_winner, ActionCandidateMode,
        ActionCategory, IaToCommon, PointerTarget, PointerTargetInfo, ProximityCandidates,
    },
    SceneEntity,
};

use super::AddCrdtInterfaceExt;

pub struct PointerEventsPlugin;

/// Bevy entities the editor wants outlined as its active selection, driven by the `/highlight`
/// console command. Unioned into the highlight pass below independent of pointer hover/proximity,
/// so the selection outline persists without writing to the scene's `PointerEvents` (and so never
/// enters the scene snapshot or the save).
#[derive(Resource, Default)]
pub struct EditorHighlight(pub EntityHashSet);

impl Plugin for PointerEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Highlit>();
        app.init_resource::<EditorHighlight>();

        app.add_crdt_lww_component::<PbPointerEvents, PointerEvents>(
            SceneComponentId::POINTER_EVENTS,
            ComponentPosition::EntityOnly,
        );

        app.add_systems(
            Update,
            (hover_text, propagate_avatar_events, entity_highlighting),
        );
    }
}

#[derive(Component, Debug)]
pub struct PointerEvents {
    pub msg: HashMap<Option<Entity>, PbPointerEvents>,
}

impl From<PbPointerEvents> for PointerEvents {
    fn from(pb_pointer_events: PbPointerEvents) -> Self {
        Self {
            msg: HashMap::from_iter([(None, pb_pointer_events)]),
        }
    }
}

impl PointerEvents {
    pub fn iter_with_scene(
        &self,
        default_scene: Option<Entity>,
    ) -> impl Iterator<Item = (Entity, &Entry)> {
        self.msg.iter().flat_map(move |(scene, events)| {
            events.pointer_events.iter().map(move |e| {
                (
                    *scene
                        .as_ref()
                        .unwrap_or_else(|| default_scene.as_ref().unwrap()),
                    e,
                )
            })
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.msg
            .iter()
            .flat_map(|(_, events)| events.pointer_events.iter())
    }
}

pub fn propagate_avatar_events(
    mut commands: Commands,
    mut foreign_players: Query<(Entity, &ForeignPlayer, Option<&mut PointerEvents>)>,
    scenes: Query<Entity, With<RendererSceneContext>>,
    new_events: Query<
        (&SceneEntity, &PointerEvents),
        (Without<ForeignPlayer>, Changed<PointerEvents>),
    >,
) {
    // remove expired
    let live_scenes = scenes.iter().collect::<HashSet<_>>();
    let mut foreign_player_lookup = HashMap::new();
    for (entity, foreign_player, maybe_pes) in foreign_players.iter_mut() {
        if let Some(mut pes) = maybe_pes {
            pes.msg
                .retain(|scene, _| scene.is_none_or(|scene| live_scenes.contains(&scene)));
        }

        foreign_player_lookup.insert(foreign_player.scene_id, entity);
    }

    // add new/updated
    for (scene_entity, new_events) in new_events.iter() {
        if let Some((foreign_entity, _, maybe_pes)) = foreign_player_lookup
            .get(&scene_entity.id)
            .and_then(|e| foreign_players.get_mut(*e).ok())
        {
            if let Some(mut pes) = maybe_pes {
                pes.msg.insert(
                    Some(scene_entity.root),
                    new_events.msg.get(&None).unwrap().clone(),
                );
            } else {
                commands.entity(foreign_entity).try_insert(PointerEvents {
                    msg: HashMap::from_iter([(
                        Some(scene_entity.root),
                        new_events.msg.get(&None).unwrap().clone(),
                    )]),
                });
            }
        }
    }
}

#[derive(Component)]
pub struct HoverText;

fn format_button_label(input_map: &InputMap, info: &Info) -> String {
    input_map
        .get_input(info.button().to_common())
        .map(|b| {
            let button_str = serde_json::to_string(&b).unwrap();
            let button_str = button_str.strip_prefix("\"").unwrap_or(&button_str);
            button_str
                .strip_suffix("\"")
                .unwrap_or(button_str)
                .to_owned()
        })
        .unwrap_or_else(|| {
            if info.button() == InputAction::IaAny {
                "(ANY)"
            } else {
                "(No binding)"
            }
            .to_owned()
        })
}

#[allow(clippy::too_many_arguments)]
fn hover_text(
    pointer_events: Query<(Option<&SceneEntity>, Option<&ForeignPlayer>, &PointerEvents)>,
    hover_target: Res<PointerTarget>,
    proximity: Res<ProximityCandidates>,
    input_map: Res<InputMap>,
    mut tooltip: ResMut<ToolTips>,
) {
    let mut texts = Vec::<(String, bool)>::default();

    // Collect every `(action-category, button)` bucket present across the
    // candidate set and pre-resolve the priority winner for each. Tier-2
    // tooltips only surface on the winning entity.
    let mut buckets: HashSet<(ActionCategory, InputAction)> = HashSet::new();
    let mut collect_buckets = |entity: Entity| {
        if let Ok((_, _, pe)) = pointer_events.get(entity) {
            for entry in pe.iter() {
                let Some(category) = event_category(entry.event_type) else {
                    continue;
                };
                let button = entry
                    .event_info
                    .as_ref()
                    .and_then(|i| i.button.map(|_| i.button()))
                    .unwrap_or(InputAction::IaAny);
                buckets.insert((category, button));
            }
        }
    };
    if let Some(info) = hover_target.0.as_ref() {
        collect_buckets(info.container);
    }
    for cand in &proximity.0 {
        collect_buckets(cand.entity);
    }

    let mut winners: HashMap<(ActionCategory, InputAction), Option<Entity>> = HashMap::new();
    for &(category, button) in &buckets {
        let winner = resolve_action_winner(
            hover_target.0.as_ref(),
            &proximity,
            &pointer_events,
            category,
            button,
        )
        .map(|(info, _)| info.container);
        winners.insert((category, button), winner);
    }

    let mut process = |entity: Entity, mode: ActionCandidateMode, info: &PointerTargetInfo| {
        let Ok((_, _, pe)) = pointer_events.get(entity) else {
            return;
        };
        for entry in pe.iter().filter(|e| mode.matches_entry(e)) {
            let Some(event_info) = entry.event_info.as_ref() else {
                continue;
            };
            if !event_info.show_feedback.unwrap_or(true) {
                continue;
            }
            let Some(text) = event_info.hover_text.as_ref() else {
                continue;
            };

            // Tier-2 entries only show on the winning entity for their bucket.
            if let Some(category) = event_category(entry.event_type) {
                let button = event_info
                    .button
                    .map(|_| event_info.button())
                    .unwrap_or(InputAction::IaAny);
                let winner = winners.get(&(category, button)).copied().flatten();
                if winner != Some(entity) {
                    continue;
                }
            }

            let button_label = format_button_label(&input_map, event_info);
            let in_range =
                mode.passes_distance(Some(event_info), info.camera_distance.0, info.distance.0);
            texts.push((format!("{button_label} : {text}"), in_range));
        }
    };

    if let Some(info) = hover_target.0.as_ref() {
        process(info.container, ActionCandidateMode::Cursor, info);
    }
    for cand in &proximity.0 {
        let synthetic = PointerTargetInfo {
            container: cand.entity,
            mesh_name: None,
            distance: FloatOrd(cand.distance),
            camera_distance: FloatOrd(0.0),
            in_scene: true,
            position: None,
            normal: None,
            face: None,
            ty: PointerTargetType::World,
        };
        process(cand.entity, ActionCandidateMode::Proximity, &synthetic);
    }

    // make unique (sorted, avoiding the Vec -> HashSet -> Vec round trip)
    texts.sort_unstable();
    texts.dedup();

    // avoid marking the resource changed when the value is identical
    if tooltip
        .0
        .get(&TooltipSource::Label("pointer_events"))
        .is_none_or(|prev| prev != &texts)
    {
        tooltip
            .0
            .insert(TooltipSource::Label("pointer_events"), texts);
    }
}

#[derive(Default, Resource, Deref, DerefMut)]
struct Highlit(EntityHashSet);

/// The glow on the hover target: the valid colour within reach of one of its entries, the invalid
/// colour beyond it.
pub fn hover_highlight(target: Option<&PointerTargetInfo>, in_range: bool) -> Option<Highlight> {
    target.map(|_| Highlight {
        color: if in_range {
            HIGHLIGHT_VALID
        } else {
            HIGHLIGHT_INVALID
        }
        .into(),
        width_px: HIGHLIGHT_WIDTH_PX,
    })
}

fn valid_highlight() -> Highlight {
    Highlight {
        color: HIGHLIGHT_VALID.into(),
        width_px: HIGHLIGHT_WIDTH_PX,
    }
}

fn entity_highlighting(
    mut commands: Commands,
    pointer_events: Query<&PointerEvents, Without<ForeignPlayer>>,
    current: Query<&Propagate<Highlight>>,
    hover_target: Res<PointerTarget>,
    proximity: Res<ProximityCandidates>,
    mut highlit: ResMut<Highlit>,
    editor: Res<EditorHighlight>,
) {
    let mut highlight_pass = EntityHashMap::<Highlight>::default();

    // the entity's entries of one mode (cursor or proximity) that want feedback and a highlight
    let highlit_entries = |entity: Entity, mode: ActionCandidateMode| {
        pointer_events.get(entity).into_iter().flat_map(move |pe| {
            pe.iter().filter(move |entry| {
                mode.matches_entry(entry)
                    && entry.event_info.as_ref().is_some_and(|info| {
                        info.show_feedback != Some(false) && info.show_highlight != Some(false)
                    })
            })
        })
    };

    if let Some(ref hover_target) = hover_target.0 {
        let mut entries =
            highlit_entries(hover_target.container, ActionCandidateMode::Cursor).peekable();
        if entries.peek().is_some() {
            // green only when a click would act: the action gate wants the target in a scene and
            // a cursor entry within reach
            let in_range = hover_target.in_scene
                && entries.any(|entry| {
                    passes_distance_check(
                        entry.event_info.as_ref(),
                        hover_target.camera_distance.0,
                        hover_target.distance.0,
                    )
                });
            if let Some(highlight) = hover_highlight(Some(hover_target), in_range) {
                highlight_pass.insert(hover_target.container, highlight);
            }
        }
    }
    // a proximity candidate glows valid, unless it is also hovered: the hover colour then tells
    // whether a click would act
    for candidate in &proximity.0 {
        if highlit_entries(candidate.entity, ActionCandidateMode::Proximity)
            .next()
            .is_some()
        {
            highlight_pass
                .entry(candidate.entity)
                .or_insert_with(valid_highlight);
        }
    }
    // editor selection — always outlined, no PointerEvents / hover required.
    highlight_pass.extend(editor.0.iter().map(|entity| (*entity, valid_highlight())));

    for (entity, highlight) in &highlight_pass {
        let source = Propagate(*highlight);
        if current.get(*entity).ok() != Some(&source) {
            debug!("Highlighting {}", entity);
            commands.entity(*entity).try_insert(source);
        }
    }

    for entity in highlit.iter() {
        if !highlight_pass.contains_key(entity) {
            debug!("Highlight of {} expired.", entity);
            commands
                .entity(*entity)
                .try_remove::<Propagate<Highlight>>();
        }
    }

    **highlit = highlight_pass.into_keys().collect();
}

#[cfg(test)]
mod test {
    use bevy::app::PropagateSet;
    use dcl_component::proto_components::sdk::components::common::{
        InteractionType, PointerEventType,
    };
    use scene_material::highlight::HighlightPlugin;

    use super::*;
    use crate::update_scene::pointer_results::ProximityCandidate;

    fn test_info() -> PointerTargetInfo {
        PointerTargetInfo {
            container: Entity::PLACEHOLDER,
            mesh_name: None,
            distance: FloatOrd(0.0),
            camera_distance: FloatOrd(0.0),
            in_scene: true,
            position: None,
            normal: None,
            face: None,
            ty: PointerTargetType::World,
        }
    }

    #[test]
    fn hover_colour_follows_range() {
        let info = PointerTargetInfo {
            distance: FloatOrd(12.0),
            camera_distance: FloatOrd(14.0),
            ..test_info()
        };
        assert_eq!(
            hover_highlight(Some(&info), false).unwrap().color,
            HIGHLIGHT_INVALID.into()
        );
        assert_eq!(
            hover_highlight(Some(&info), true).unwrap().color,
            HIGHLIGHT_VALID.into()
        );
        assert_eq!(
            hover_highlight(Some(&info), true).unwrap().width_px,
            HIGHLIGHT_WIDTH_PX
        );
        assert!(hover_highlight(None, true).is_none());
    }

    fn pointer_events(max_distance: Option<f32>) -> PointerEvents {
        PbPointerEvents {
            pointer_events: vec![Entry {
                event_type: PointerEventType::PetDown as i32,
                event_info: Some(Info {
                    max_distance,
                    ..Default::default()
                }),
                ..Default::default()
            }],
        }
        .into()
    }

    fn proximity_events() -> PointerEvents {
        PbPointerEvents {
            pointer_events: vec![Entry {
                event_type: PointerEventType::PetDown as i32,
                event_info: Some(Info::default()),
                interaction_type: Some(InteractionType::Proximity as i32),
            }],
        }
        .into()
    }

    fn highlight_app() -> App {
        let mut app = App::new();
        app.add_plugins(HighlightPlugin)
            .init_resource::<PointerTarget>()
            .init_resource::<ProximityCandidates>()
            .init_resource::<Highlit>()
            .init_resource::<EditorHighlight>()
            .add_systems(
                Update,
                entity_highlighting.before(PropagateSet::<Highlight>::default()),
            );
        app
    }

    fn hover(app: &mut App, entity: Option<Entity>, distance: f32) {
        app.world_mut().resource_mut::<PointerTarget>().0 =
            entity.map(|container| PointerTargetInfo {
                container,
                distance: FloatOrd(distance),
                camera_distance: FloatOrd(distance),
                ..test_info()
            });
    }

    fn highlights(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&Highlight>().iter(world).count()
            + world.query::<&Propagate<Highlight>>().iter(world).count()
    }

    #[test]
    fn hover_target_glows_until_released() {
        let mut app = highlight_app();
        let target = app.world_mut().spawn(pointer_events(None)).id();
        let mesh = app.world_mut().spawn(ChildOf(target)).id();

        // 12 m is beyond the default 10 m reach
        hover(&mut app, Some(target), 12.0);
        app.update();
        let invalid = Highlight {
            color: HIGHLIGHT_INVALID.into(),
            width_px: HIGHLIGHT_WIDTH_PX,
        };
        assert_eq!(
            app.world().get::<Propagate<Highlight>>(target).map(|p| p.0),
            Some(invalid)
        );
        assert_eq!(app.world().get::<Highlight>(mesh), Some(&invalid));

        // within reach the same target turns valid
        hover(&mut app, Some(target), 5.0);
        app.update();
        assert_eq!(
            app.world().get::<Highlight>(mesh).map(|h| h.color),
            Some(HIGHLIGHT_VALID.into())
        );

        // the entity's own max distance widens the reach
        app.world_mut()
            .entity_mut(target)
            .insert(pointer_events(Some(20.0)));
        hover(&mut app, Some(target), 12.0);
        app.update();
        assert_eq!(
            app.world().get::<Highlight>(mesh).map(|h| h.color),
            Some(HIGHLIGHT_VALID.into())
        );

        hover(&mut app, None, 0.0);
        app.update();
        assert_eq!(highlights(&mut app), 0);

        // despawned while targeted: nothing panics and nothing is left behind
        hover(&mut app, Some(target), 5.0);
        app.update();
        // the source and the mesh glow, the source carries the propagation
        assert_eq!(highlights(&mut app), 3);
        app.world_mut().entity_mut(target).despawn();
        app.update();
        app.update();
        assert_eq!(highlights(&mut app), 0);
        assert!(app.world().resource::<Highlit>().is_empty());
    }

    fn glow(app: &mut App, entity: Entity) -> Option<LinearRgba> {
        app.world()
            .get::<Propagate<Highlight>>(entity)
            .map(|p| p.0.color)
    }

    /// The glow says what a click would do: a target outside every scene, or one whose only
    /// entries are proximity ones, is not clickable.
    #[test]
    fn hover_glow_follows_the_click_gate() {
        let mut app = highlight_app();
        let target = app.world_mut().spawn(pointer_events(Some(20.0))).id();

        // in reach but outside every scene: a click does nothing
        hover(&mut app, Some(target), 5.0);
        app.world_mut()
            .resource_mut::<PointerTarget>()
            .0
            .as_mut()
            .unwrap()
            .in_scene = false;
        app.update();
        assert_eq!(glow(&mut app, target), Some(HIGHLIGHT_INVALID.into()));

        // only proximity entries: the cursor never acts, so hovering shows nothing
        let near = app.world_mut().spawn(proximity_events()).id();
        hover(&mut app, Some(near), 1.0);
        app.update();
        assert_eq!(glow(&mut app, near), None);

        // as a proximity candidate it glows valid
        app.world_mut()
            .resource_mut::<ProximityCandidates>()
            .0
            .push(ProximityCandidate {
                entity: near,
                distance: 1.0,
                nearest_point: Vec3::ZERO,
                entity_position: Vec3::ZERO,
            });
        app.update();
        assert_eq!(glow(&mut app, near), Some(HIGHLIGHT_VALID.into()));

        // a hovered candidate keeps the hover's verdict: out of the cursor's reach stays pink
        app.world_mut()
            .entity_mut(near)
            .insert(pointer_events(None));
        hover(&mut app, Some(near), 12.0);
        app.update();
        assert_eq!(glow(&mut app, near), Some(HIGHLIGHT_INVALID.into()));
    }
}

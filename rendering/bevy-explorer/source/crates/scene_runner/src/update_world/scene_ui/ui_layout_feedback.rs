// UiLayoutFeedback (experimental, bevy only): where each named screen UI node landed after layout,
// written back to its scene so an agent can measure the UI through the CRDT snapshot.

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
    ui::{CalculatedClip, ComputedNode},
};
use dcl_component::{
    proto_components::sdk::components::{PbUiLayoutFeedback, UiLayoutRect},
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::CrdtType;

use super::{UiLink, UiTransform};
use crate::{
    initialize_scene::SuperUserScene, renderer_context::RendererSceneContext, SceneEntity,
};

#[allow(clippy::type_complexity)]
pub fn write_ui_layout_feedback(
    // the root's UiLink goes when its UI is torn down; the context stays to clear the feedback
    mut scenes: Query<(Option<&UiLink>, &mut RendererSceneContext), Without<SuperUserScene>>,
    named: Query<(&SceneEntity, &UiLink, &UiTransform)>,
    ui_nodes: Query<(
        &ComputedNode,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&CalculatedClip>,
    )>,
    hierarchy: Query<(&Node, Option<&ChildOf>)>,
    mut written: Local<HashMap<Entity, HashSet<SceneEntityId>>>,
) {
    let mut seen: HashMap<Entity, HashSet<SceneEntityId>> = HashMap::new();

    for (scene_entity, link, transform) in named.iter() {
        if transform.element_id.is_none() || !link.is_window_ui {
            continue;
        }
        let Ok((Some(root_link), mut context)) = scenes.get_mut(scene_entity.root) else {
            continue;
        };
        let (Ok((root, root_global, ..)), Ok((computed, global, visibility, clip))) = (
            ui_nodes.get(root_link.ui_entity),
            ui_nodes.get(link.ui_entity),
        ) else {
            continue;
        };

        // bevy_ui works in physical px from the window's corner; the scene's canvas is logical px
        // from its UI root's corner (a box inside the window when the scene UI is constrained)
        let scale = computed.inverse_scale_factor();
        let origin = (root_global.translation().truncate() - root.size() / 2.0) * scale;
        let rect = Rect::from_center_size(global.translation().truncate(), computed.size());
        let shown = clip.map_or(rect, |clip| rect.intersect(clip.clip));
        let visible =
            visibility.get() && displayed(&hierarchy, link.ui_entity) && !shown.is_empty();
        let to_canvas = |r: Rect| {
            // a tenth of a unit, so sub-pixel jitter doesn't rewrite the component every frame
            let round = |v: f32| (v * 10.0).round() / 10.0;
            UiLayoutRect {
                x: round(r.min.x * scale - origin.x),
                y: round(r.min.y * scale - origin.y),
                width: round(r.width() * scale),
                height: round(r.height() * scale),
            }
        };

        context.update_crdt_if_different(
            SceneComponentId::UI_LAYOUT_FEEDBACK,
            CrdtType::LWW_ENT,
            scene_entity.id,
            &PbUiLayoutFeedback {
                rect: Some(to_canvas(rect)),
                visible_rect: visible.then(|| to_canvas(shown)),
                visible,
                draw_order: computed.stack_index(),
            },
        );
        seen.entry(scene_entity.root)
            .or_default()
            .insert(scene_entity.id);
    }

    // a node that lost its element_id, its UiTransform or its UI root stops being reported (a
    // deleted one takes it along)
    for (root, ids) in written.iter() {
        let Ok((_, mut context)) = scenes.get_mut(*root) else {
            continue;
        };
        let still = seen.get(root);
        let stale: Vec<SceneEntityId> = ids
            .iter()
            .filter(|id| {
                context.bevy_entity(**id).is_some() && still.is_none_or(|s| !s.contains(*id))
            })
            .copied()
            .collect();
        for id in stale {
            context.clear_crdt(SceneComponentId::UI_LAYOUT_FEEDBACK, CrdtType::LWW_ENT, id);
        }
    }
    *written = seen;
}

// a node inside a display:none ancestor keeps its last computed size, so walk up
fn displayed(hierarchy: &Query<(&Node, Option<&ChildOf>)>, mut ui_entity: Entity) -> bool {
    loop {
        let Ok((node, parent)) = hierarchy.get(ui_entity) else {
            return true;
        };
        if node.display == Display::None {
            return false;
        }
        match parent {
            Some(parent) => ui_entity = parent.parent(),
            None => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::SystemId;
    use dcl_component::{
        proto_components::sdk::components::PbUiTransform, DclReader, SceneEntityId,
    };
    use dcl_types::SceneId;

    use super::*;

    fn named_id() -> SceneEntityId {
        SceneEntityId::new(512, 0)
    }

    fn ui_node(world: &mut World, center: Vec2, size: Vec2, parent: Option<Entity>) -> Entity {
        let mut node = world.spawn((
            Node::default(),
            ComputedNode {
                size,
                // a 2x window: physical px are twice the canvas units
                inverse_scale_factor: 0.5,
                ..Default::default()
            },
            GlobalTransform::from_translation(center.extend(0.0)),
            InheritedVisibility::VISIBLE,
        ));
        if let Some(parent) = parent {
            node.insert(ChildOf(parent));
        }
        node.id()
    }

    fn context(root: Entity) -> RendererSceneContext {
        RendererSceneContext::new(
            SceneId::DUMMY,
            "hash".to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            vec![],
            Default::default(),
            vec![],
            root,
            0.0,
            false,
            "sdk_version",
            false,
            false,
        )
    }

    fn feedback(world: &mut World, root: Entity) -> Option<PbUiLayoutFeedback> {
        let context = world.get::<RendererSceneContext>(root).unwrap();
        context
            .crdt_store
            .get(
                SceneComponentId::UI_LAYOUT_FEEDBACK,
                CrdtType::LWW_ENT,
                named_id(),
            )
            .map(|bytes| DclReader::new(bytes).read().unwrap())
    }

    // one registered system across runs, so its Local keeps what it wrote
    fn run(world: &mut World, system: SystemId) {
        world.run_system(system).unwrap();
    }

    #[test]
    fn reports_canvas_rects_and_clears_stale_feedback() {
        let mut world = World::new();
        let system = world.register_system(write_ui_layout_feedback);
        // the scene's UI root is inset by (100, 50) canvas units: (200, 100) physical
        let root_node = ui_node(
            &mut world,
            Vec2::new(1000.0, 600.0),
            Vec2::new(1600.0, 1000.0),
            None,
        );
        let node = ui_node(
            &mut world,
            Vec2::new(300.0, 200.0),
            Vec2::new(100.0, 40.0),
            Some(root_node),
        );
        // drawn above the root, as ui_stack_system numbers them
        world.get_mut::<ComputedNode>(node).unwrap().stack_index = 1;
        let root = world.spawn_empty().id();
        let mut scene = context(root);
        let named = world.spawn_empty().id();
        scene.associate_bevy_entity(named_id(), named);
        world.entity_mut(root).insert((
            UiLink {
                ui_entity: root_node,
                ..Default::default()
            },
            scene,
        ));
        world.entity_mut(named).insert((
            SceneEntity {
                root,
                scene_id: SceneId::DUMMY,
                id: named_id(),
            },
            UiLink {
                ui_entity: node,
                ..Default::default()
            },
            UiTransform::from(PbUiTransform {
                element_id: Some("score".to_owned()),
                ..Default::default()
            }),
        ));

        run(&mut world, system);
        let written = feedback(&mut world, root).expect("a named node is reported");
        let rect = written.rect.unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (25.0, 40.0, 50.0, 20.0)
        );
        assert!(written.visible);
        assert_eq!(written.draw_order, 1);

        world.get_mut::<Node>(root_node).unwrap().display = Display::None;
        run(&mut world, system);
        let hidden = feedback(&mut world, root).unwrap();
        assert!(!hidden.visible, "a display:none ancestor hides the node");
        assert_eq!(hidden.visible_rect, None);

        world.get_mut::<Node>(root_node).unwrap().display = Display::Flex;
        world.entity_mut(named).remove::<UiTransform>();
        run(&mut world, system);
        assert_eq!(
            feedback(&mut world, root),
            None,
            "a live node without UiTransform is cleared"
        );

        world
            .entity_mut(named)
            .insert(UiTransform::from(PbUiTransform {
                element_id: Some("score".to_owned()),
                ..Default::default()
            }));
        run(&mut world, system);
        assert!(feedback(&mut world, root).is_some());
        world.entity_mut(root).remove::<UiLink>();
        run(&mut world, system);
        assert_eq!(
            feedback(&mut world, root),
            None,
            "a torn-down UI root clears its nodes"
        );
    }
}

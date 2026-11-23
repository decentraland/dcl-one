//! The hover glow: a hovered avatar's opaque and alpha-tested meshes glow cyan-green through the
//! shared highlight pass (`scene_material::highlight`), fading in and out over a tenth of a second.
//! The glow is a `Propagate<Highlight>` on the player container, which the highlight plugin's
//! hierarchy propagation carries to the avatar's meshes, a rebuilt avatar's new ones included. A
//! mesh that never glows carries `PropagateOver<Highlight>`; the nametag and the ghost stop the
//! glow with `PropagateStop<Highlight>`.
//!
//! Only the explorer (`colliders`) drives the glow; the renderer alone still marks the meshes.

#[cfg(feature = "explorer")]
use bevy::app::Propagate;
use bevy::{
    app::{Inherited, PropagateOver},
    prelude::*,
};
use scene_material::highlight::Highlight;

#[cfg(feature = "explorer")]
const AVATAR_GLOW_COLOR: Color = Color::srgb(0.0, 1.0, 0.52395105);
/// the glow's opacity once it has faded in
#[cfg(feature = "explorer")]
const AVATAR_GLOW_OPACITY: f32 = 0.8;
/// A share of the view's height, as `Highlight::width`: 3.5 pixels at 1080p.
#[cfg(feature = "explorer")]
const AVATAR_GLOW_WIDTH: f32 = 3.5 / 1080.0;
#[cfg(feature = "explorer")]
const AVATAR_GLOW_FADE_SECONDS: f32 = 0.1;

/// On the player container: `target` 1.0 while hovered, 0.0 after; `alpha` moves toward it at a
/// constant rate, a full fade taking `AVATAR_GLOW_FADE_SECONDS`. Removed when both are 0.
#[cfg(feature = "explorer")]
#[derive(Component, Debug, PartialEq, Clone, Copy)]
pub(crate) struct HoverGlow {
    target: f32,
    alpha: f32,
}

#[cfg(feature = "explorer")]
fn step_glow(glow: HoverGlow, dt: f32) -> HoverGlow {
    let step = dt / AVATAR_GLOW_FADE_SECONDS;
    let alpha = if glow.alpha < glow.target {
        (glow.alpha + step).min(glow.target)
    } else {
        (glow.alpha - step).max(glow.target)
    };
    HoverGlow { alpha, ..glow }
}

/// The highlight of a glow faded to `alpha`. The highlight pass blends its colour as
/// premultiplied, so the colour is scaled by the opacity it carries.
#[cfg(feature = "explorer")]
fn glow_highlight(alpha: f32) -> Highlight {
    let opacity = AVATAR_GLOW_OPACITY * alpha;
    let color = AVATAR_GLOW_COLOR.to_linear();
    Highlight {
        color: LinearRgba::new(
            color.red * opacity,
            color.green * opacity,
            color.blue * opacity,
            opacity,
        ),
        width: AVATAR_GLOW_WIDTH,
    }
}

/// The glow follows the opaque draw: opaque and alpha-tested meshes take it, blended ones never.
pub(crate) fn can_glow(alpha_mode: AlphaMode) -> bool {
    matches!(
        alpha_mode,
        AlphaMode::Opaque | AlphaMode::Mask(_) | AlphaMode::AlphaToCoverage
    )
}

/// Marks whether `mesh` takes part in its avatar's glow. The avatar build calls it for every mesh
/// it dresses, both ways, so a mesh entity kept across a rebuild follows the wearable it now shows.
/// Propagation reads `PropagateOver` only when the glow changes, so under a steady glow the mesh's
/// `Highlight` is put on or taken off here.
pub(crate) fn set_hover_glows(mesh: &mut EntityCommands, glows: bool) {
    if glows {
        mesh.try_remove::<PropagateOver<Highlight>>();
        mesh.queue_silenced(|mut mesh: EntityWorldMut| {
            if let Some(glow) = mesh.get::<Inherited<Highlight>>().map(|glow| glow.0) {
                mesh.insert(glow);
            }
        });
    } else {
        mesh.try_insert(PropagateOver::<Highlight>::default());
        mesh.try_remove::<Highlight>();
    }
}

/// fades `container`'s glow toward `target`, from nothing when a glow starts
#[cfg(feature = "explorer")]
pub(crate) fn set_glow_target(
    commands: &mut Commands,
    glows: &mut Query<&mut HoverGlow>,
    container: Entity,
    target: f32,
) {
    match glows.get_mut(container) {
        Ok(mut glow) => glow.target = target,
        Err(_) if target > 0.0 => {
            commands
                .entity(container)
                .try_insert(HoverGlow { target, alpha: 0.0 });
        }
        Err(_) => (),
    }
}

/// steps each glow and puts its highlight on the container for propagation to carry to the
/// meshes; a finished glow takes the highlight and itself away
#[cfg(feature = "explorer")]
pub(crate) fn apply_hover_glow(
    mut commands: Commands,
    time: Res<Time>,
    mut glows: Query<(Entity, &mut HoverGlow, Option<&Propagate<Highlight>>)>,
) {
    for (container, mut glow, current) in &mut glows {
        glow.set_if_neq(step_glow(*glow, time.delta_secs()));
        if glow.alpha == 0.0 && glow.target == 0.0 {
            commands
                .entity(container)
                .try_remove::<(HoverGlow, Propagate<Highlight>)>();
            continue;
        }
        let source = Propagate(glow_highlight(glow.alpha));
        if current != Some(&source) {
            commands.entity(container).try_insert(source);
        }
    }
}

#[cfg(all(test, feature = "explorer"))]
mod tests {
    use std::time::Duration;

    use bevy::{
        app::{PropagateSet, PropagateStop},
        ecs::system::RunSystemOnce,
        time::TimeUpdateStrategy,
    };
    use scene_material::highlight::HighlightPlugin;

    use super::*;

    #[test]
    fn glow_fades_in_and_out_over_a_tenth_of_a_second() {
        let g = step_glow(
            HoverGlow {
                target: 1.0,
                alpha: 0.0,
            },
            0.05,
        );
        assert!((g.alpha - 0.5).abs() < 1e-6);
        let g = step_glow(
            HoverGlow {
                target: 1.0,
                alpha: 0.5,
            },
            1.0,
        );
        assert_eq!(g.alpha, 1.0);
        let g = step_glow(
            HoverGlow {
                target: 0.0,
                alpha: 1.0,
            },
            0.025,
        );
        assert!((g.alpha - 0.75).abs() < 1e-6);
        let g = step_glow(
            HoverGlow {
                target: 0.0,
                alpha: 0.5,
            },
            1.0,
        );
        assert_eq!(g.alpha, 0.0);
    }

    #[test]
    fn the_highlight_carries_the_glow_colour_at_the_faded_opacity() {
        let half = glow_highlight(0.5);
        assert!((half.color.alpha - 0.4).abs() < 1e-6);
        assert_eq!(half.width, AVATAR_GLOW_WIDTH);
        assert!((AVATAR_GLOW_WIDTH * 1080.0 - 3.5).abs() < 1e-6);

        // the highlight pass blends a premultiplied colour
        let full = glow_highlight(1.0);
        let linear = AVATAR_GLOW_COLOR.to_linear();
        assert!((full.color.alpha - AVATAR_GLOW_OPACITY).abs() < 1e-6);
        assert!((full.color.red - linear.red * AVATAR_GLOW_OPACITY).abs() < 1e-6);
        assert!((full.color.green - linear.green * AVATAR_GLOW_OPACITY).abs() < 1e-6);
        assert!((full.color.blue - linear.blue * AVATAR_GLOW_OPACITY).abs() < 1e-6);
        assert!((half.color.green - linear.green * 0.4).abs() < 1e-6);

        // a faded-out glow adds nothing to the frame
        assert_eq!(glow_highlight(0.0).color, LinearRgba::NONE);
    }

    #[test]
    fn only_opaque_and_alpha_tested_materials_glow() {
        assert!(can_glow(AlphaMode::Opaque));
        assert!(can_glow(AlphaMode::Mask(0.5)));
        assert!(can_glow(AlphaMode::AlphaToCoverage));
        assert!(!can_glow(AlphaMode::Blend));
        assert!(!can_glow(AlphaMode::Premultiplied));
        assert!(!can_glow(AlphaMode::Add));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, HighlightPlugin))
            .add_systems(
                Update,
                apply_hover_glow.before(PropagateSet::<Highlight>::default()),
            );
        // the first update starts the clock
        app.update();
        app
    }

    fn tick(app: &mut App, seconds: f32) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            seconds,
        )));
        app.update();
    }

    /// moves the glow from the avatar hovered before to `to`, as `update_avatar_collider_actions`
    /// does when the pointer target changes
    fn hover(app: &mut App, from: Option<Entity>, to: Option<Entity>) {
        app.world_mut()
            .run_system_once(
                move |mut commands: Commands, mut glows: Query<&mut HoverGlow>| {
                    if let Some(old) = from {
                        set_glow_target(&mut commands, &mut glows, old, 0.0);
                    }
                    if let Some(new) = to {
                        set_glow_target(&mut commands, &mut glows, new, 1.0);
                    }
                },
            )
            .unwrap();
    }

    fn mark(app: &mut App, mesh: Entity, glows: bool) {
        let mut commands = app.world_mut().commands();
        set_hover_glows(&mut commands.entity(mesh), glows);
        app.world_mut().flush();
    }

    /// a built avatar under `container`, its meshes marked as the avatar build marks them: an
    /// opaque body, an alpha-tested wearable and a blended one
    fn build_avatar(app: &mut App, container: Entity) -> (Entity, [Entity; 3]) {
        let avatar = app.world_mut().spawn(ChildOf(container)).id();
        let meshes = [AlphaMode::Opaque, AlphaMode::Mask(0.5), AlphaMode::Blend].map(|mode| {
            let mesh = app.world_mut().spawn(ChildOf(avatar)).id();
            mark(app, mesh, can_glow(mode));
            mesh
        });
        (avatar, meshes)
    }

    fn player(app: &mut App) -> (Entity, Entity, [Entity; 3]) {
        let container = app.world_mut().spawn_empty().id();
        let (avatar, meshes) = build_avatar(app, container);
        (container, avatar, meshes)
    }

    fn glow(app: &App, entity: Entity) -> Option<HoverGlow> {
        app.world().get::<HoverGlow>(entity).copied()
    }

    fn highlight(app: &App, entity: Entity) -> Option<Highlight> {
        app.world().get::<Highlight>(entity).copied()
    }

    fn highlights(app: &mut App) -> usize {
        app.world_mut()
            .query::<&Highlight>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn hovering_another_avatar_fades_the_first_out_and_the_second_in() {
        let mut app = app();
        let (a, _, a_meshes) = player(&mut app);
        let (b, _, b_meshes) = player(&mut app);

        hover(&mut app, None, Some(a));
        tick(&mut app, 0.05);
        let a_glow = glow(&app, a).unwrap();
        assert!((a_glow.alpha - 0.5).abs() < 1e-6);
        assert_eq!(
            highlight(&app, a_meshes[0]),
            Some(glow_highlight(a_glow.alpha))
        );

        hover(&mut app, Some(a), Some(b));
        tick(&mut app, 0.001);
        let a_glow = glow(&app, a).unwrap();
        let b_glow = glow(&app, b).unwrap();
        assert_eq!(a_glow.target, 0.0);
        assert!((a_glow.alpha - 0.49).abs() < 1e-6, "{a_glow:?}");
        assert_eq!(b_glow.target, 1.0);
        assert!((b_glow.alpha - 0.01).abs() < 1e-6, "{b_glow:?}");
        assert_eq!(
            highlight(&app, a_meshes[0]),
            Some(glow_highlight(a_glow.alpha))
        );
        assert_eq!(
            highlight(&app, b_meshes[0]),
            Some(glow_highlight(b_glow.alpha))
        );

        // A fades out completely and lets go of its meshes; B stays lit
        tick(&mut app, 0.1);
        assert_eq!(glow(&app, a), None);
        assert!(a_meshes.iter().all(|mesh| highlight(&app, *mesh).is_none()));
        assert_eq!(glow(&app, b).unwrap().alpha, 1.0);
        assert_eq!(highlight(&app, b_meshes[0]), Some(glow_highlight(1.0)));
    }

    #[test]
    fn despawning_a_glowing_avatar_leaves_no_highlight() {
        let mut app = app();
        let (a, avatar, meshes) = player(&mut app);

        hover(&mut app, None, Some(a));
        tick(&mut app, 0.05);
        // the container, the avatar and its two glowing meshes
        assert!(meshes[..2]
            .iter()
            .all(|mesh| highlight(&app, *mesh).is_some()));
        assert!(highlight(&app, avatar).is_some());
        assert_eq!(highlights(&mut app), 4);

        hover(&mut app, Some(a), None);
        tick(&mut app, 0.01);
        assert_eq!(highlights(&mut app), 4);
        app.world_mut().entity_mut(a).despawn();
        tick(&mut app, 0.01);
        assert_eq!(highlights(&mut app), 0);
    }

    #[test]
    fn a_rebuilt_avatar_glows_at_the_current_alpha_next_tick() {
        let mut app = app();
        let (b, avatar, _) = player(&mut app);

        hover(&mut app, None, Some(b));
        tick(&mut app, 0.03);

        // as a rebuild does: the old avatar goes, a new one is built in its place
        app.world_mut().entity_mut(avatar).despawn();
        let (_, meshes) = build_avatar(&mut app, b);
        assert!(meshes.iter().all(|mesh| highlight(&app, *mesh).is_none()));

        tick(&mut app, 0.03);
        let alpha = glow(&app, b).unwrap().alpha;
        assert!((alpha - 0.6).abs() < 1e-6, "{alpha}");
        assert_eq!(highlight(&app, meshes[0]), Some(glow_highlight(alpha)));
        assert_eq!(highlight(&app, meshes[1]), Some(glow_highlight(alpha)));
        assert_eq!(highlight(&app, meshes[2]), None);
    }

    #[test]
    fn a_rebuilt_avatar_glows_under_a_steady_glow() {
        let mut app = app();
        let (b, avatar, _) = player(&mut app);

        hover(&mut app, None, Some(b));
        tick(&mut app, 0.2);

        app.world_mut().entity_mut(avatar).despawn();
        let (_, meshes) = build_avatar(&mut app, b);
        tick(&mut app, 0.01);
        assert_eq!(highlight(&app, meshes[0]), Some(glow_highlight(1.0)));
        assert_eq!(highlight(&app, meshes[1]), Some(glow_highlight(1.0)));
        assert_eq!(highlight(&app, meshes[2]), None);
    }

    #[test]
    fn a_wearable_that_is_not_outline_compatible_never_glows() {
        let mut app = app();
        let (a, _, [opaque, mask, blend]) = player(&mut app);
        mark(&mut app, mask, false);

        hover(&mut app, None, Some(a));
        for _ in 0..4 {
            tick(&mut app, 0.05);
            assert!(highlight(&app, opaque).is_some());
            assert_eq!(highlight(&app, mask), None);
            assert_eq!(highlight(&app, blend), None);
        }
    }

    /// a mesh entity kept across a rebuild follows the wearable it now shows, both ways, under a
    /// steady glow and mid-fade
    #[test]
    fn a_reused_mesh_follows_its_current_wearable() {
        let mut app = app();
        let (a, _, [opaque, mask, _]) = player(&mut app);
        mark(&mut app, mask, false);

        hover(&mut app, None, Some(a));
        tick(&mut app, 0.2);
        assert_eq!(highlight(&app, opaque), Some(glow_highlight(1.0)));
        assert_eq!(highlight(&app, mask), None);

        mark(&mut app, opaque, false);
        mark(&mut app, mask, true);
        tick(&mut app, 0.01);
        assert_eq!(highlight(&app, opaque), None);
        assert_eq!(highlight(&app, mask), Some(glow_highlight(1.0)));

        hover(&mut app, Some(a), None);
        tick(&mut app, 0.05);
        mark(&mut app, opaque, true);
        mark(&mut app, mask, false);
        tick(&mut app, 0.01);
        let alpha = glow(&app, a).unwrap().alpha;
        assert_eq!(highlight(&app, opaque), Some(glow_highlight(alpha)));
        assert_eq!(highlight(&app, mask), None);
    }

    /// the nametag and the ghost hang under the player container and never glow
    #[test]
    fn a_stopped_child_never_glows() {
        let mut app = app();
        let (a, _, _) = player(&mut app);
        let tag = app
            .world_mut()
            .spawn((ChildOf(a), PropagateStop::<Highlight>::default()))
            .id();
        let tag_mesh = app.world_mut().spawn(ChildOf(tag)).id();

        hover(&mut app, None, Some(a));
        tick(&mut app, 0.05);
        assert_eq!(highlight(&app, tag), None);
        assert_eq!(highlight(&app, tag_mesh), None);
    }
}

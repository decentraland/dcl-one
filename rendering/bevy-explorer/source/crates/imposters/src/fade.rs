use bevy::prelude::*;
use boimp::render::Imposter;

/// unity: LODGroup.crossFadeAnimationDuration default (not set in the repo); confirm 0.5 s
/// against the Unity manual before changing.
pub const CROSS_FADE_SECONDS: f32 = 0.5;

/// A running or finished alpha fade on an imposter tile entity. The tile's `Imposter` material
/// is per tile, so the fade can write its alpha without touching other tiles.
#[derive(Component)]
pub struct ImposterFade {
    pub from: f32,
    pub to: f32,
    pub started: f32,
}

/// Alpha `elapsed` seconds after `started`, linear from `from` to `to`.
pub fn fade_alpha(fade: &ImposterFade, now: f32) -> f32 {
    let t = ((now - fade.started) / CROSS_FADE_SECONDS).clamp(0.0, 1.0);
    fade.from + (fade.to - fade.from) * t
}

/// Start a fade toward `to` from the current alpha (reverses a running fade without a jump).
pub fn retarget(
    fade: Option<&ImposterFade>,
    current_alpha: f32,
    to: f32,
    now: f32,
) -> ImposterFade {
    // a running fade reports its own alpha, so the caller's value only matters for a first fade
    let from = fade.map_or(current_alpha, |f| fade_alpha(f, now));
    ImposterFade {
        from,
        to,
        started: now,
    }
}

/// Writes each fading tile's alpha into its own `Imposter` materials and despawns tiles that
/// finished fading out. The asset is only touched when the alpha changes, so a finished tile
/// costs no material re-prepare.
pub fn fade_imposters(
    mut commands: Commands,
    time: Res<Time>,
    tiles: Query<(Entity, &ImposterFade, Option<&Children>)>,
    materials: Query<&MeshMaterial3d<Imposter>>,
    mut assets: ResMut<Assets<Imposter>>,
) {
    let now = time.elapsed_secs();
    for (entity, fade, children) in &tiles {
        let alpha = fade_alpha(fade, now);
        if fade.to == 0.0 && alpha == 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        for material in children
            .into_iter()
            .flatten()
            .filter_map(|c| materials.get(*c).ok())
        {
            if assets
                .get(&material.0)
                .is_some_and(|i| i.data.alpha != alpha)
            {
                if let Some(imposter) = assets.get_mut(&material.0) {
                    imposter.data.alpha = alpha;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use boimp::render::ImposterData;

    use super::*;

    #[test]
    fn fade_is_linear_over_half_a_second() {
        let f = ImposterFade {
            from: 0.0,
            to: 1.0,
            started: 10.0,
        };
        assert_eq!(fade_alpha(&f, 10.0), 0.0);
        assert!((fade_alpha(&f, 10.25) - 0.5).abs() < 1e-6);
        assert_eq!(fade_alpha(&f, 11.0), 1.0);
    }

    #[test]
    fn retarget_reverses_from_the_current_alpha() {
        let f = retarget(None, 0.0, 1.0, 0.0);
        let mid = fade_alpha(&f, 0.2);
        let back = retarget(Some(&f), mid, 0.0, 0.2);
        assert_eq!(back.from, mid);
        assert_eq!(fade_alpha(&back, 0.2 + CROSS_FADE_SECONDS), 0.0);
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Imposter>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, fade_imposters);
        app
    }

    fn spawn_tile(app: &mut App, fade: ImposterFade, alpha: f32) -> (Entity, Handle<Imposter>) {
        let imposter = Imposter {
            data: ImposterData {
                center_and_scale: Vec4::ZERO,
                packed_tile_offset: UVec2::ZERO,
                packed_tile_size: UVec2::ZERO,
                grid_size: 0,
                base_tile_size: 0,
                flags: 0,
                alpha,
                multisample_amount: 0.0,
            },
            pixels: Handle::default(),
            indices: Handle::default(),
            indices_hi: Handle::default(),
            depth_palette: Handle::default(),
            alpha_mode: AlphaMode::Blend,
            vram_bytes: 0,
        };
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Imposter>>()
            .add(imposter);
        let tile = app
            .world_mut()
            .spawn(fade)
            .with_child(MeshMaterial3d(handle.clone()))
            .id();
        (tile, handle)
    }

    fn alpha(app: &App, handle: &Handle<Imposter>) -> f32 {
        app.world()
            .resource::<Assets<Imposter>>()
            .get(handle)
            .unwrap()
            .data
            .alpha
    }

    fn run(app: &mut App, ticks: usize) {
        for _ in 0..ticks {
            app.update();
        }
    }

    #[test]
    fn fade_in_reaches_full_alpha_and_keeps_the_tile() {
        let mut app = app();
        let (tile, handle) = spawn_tile(&mut app, retarget(None, 0.0, 1.0, 0.0), 0.0);
        run(&mut app, 7);
        assert_eq!(alpha(&app, &handle), 1.0);
        assert!(app.world().get_entity(tile).is_ok());
    }

    #[test]
    fn fade_out_despawns_the_tile_at_zero() {
        let mut app = app();
        let (tile, _handle) = spawn_tile(&mut app, retarget(None, 1.0, 0.0, 0.0), 1.0);
        run(&mut app, 3);
        assert!(app.world().get_entity(tile).is_ok());
        run(&mut app, 4);
        assert!(app.world().get_entity(tile).is_err());
    }

    #[test]
    fn requiring_a_tile_mid_fade_out_brings_it_back_to_full_alpha() {
        let mut app = app();
        let (tile, handle) = spawn_tile(&mut app, retarget(None, 1.0, 0.0, 0.0), 1.0);
        run(&mut app, 3);
        let now = app.world().resource::<Time>().elapsed_secs();
        let current = alpha(&app, &handle);
        assert!(current > 0.0 && current < 1.0);
        let back = retarget(app.world().get::<ImposterFade>(tile), current, 1.0, now);
        assert_eq!(back.from, current);
        app.world_mut().entity_mut(tile).insert(back);
        run(&mut app, 7);
        assert_eq!(alpha(&app, &handle), 1.0);
        assert!(app.world().get_entity(tile).is_ok());
    }
}

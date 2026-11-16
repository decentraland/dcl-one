//! Reproducible cloud captures without changing scene lighting or hiding clouds.
use bevy::prelude::*;
use bevy_atmosphere::system_param::AtmosphereMut;
use bevy_console::ConsoleCommand;
use console::DoAddConsoleCommand;
use serde_json::json;

use crate::nishita_cloud::{init_noise_seeded, NishitaCloud};

pub(crate) struct CloudMotionPlugin;
impl Plugin for CloudMotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CloudMotion>()
            .add_console_command::<CloudMotionCommand, _>(cloud_motion_command);
    }
}

struct SavedClouds {
    noise: Handle<Image>,
    time: f32,
    rotation: f32,
    tick: u32,
}

#[derive(Resource, Default)]
pub(crate) struct CloudMotion {
    saved: Option<SavedClouds>,
    fixed_noise: Option<Handle<Image>>,
}

impl CloudMotion {
    pub(crate) fn enabled(&self) -> bool {
        self.saved.is_none()
    }

    fn set(&mut self, enabled: bool, sky: &mut NishitaCloud, images: &mut Assets<Image>) {
        if enabled {
            if let Some(saved) = self.saved.take() {
                sky.noise_texture = saved.noise;
                sky.time = saved.time;
                sky.scene_cloud_rotation = saved.rotation;
            }
        } else {
            let saved = self.saved.get_or_insert_with(|| SavedClouds {
                noise: sky.noise_texture.clone(),
                time: sky.time,
                rotation: sky.scene_cloud_rotation,
                tick: sky.tick,
            });
            // Re-pinning resets the refresh counter, but retains the original live phase.
            saved.tick = sky.tick;
            sky.noise_texture = self
                .fixed_noise
                .get_or_insert_with(|| images.add(init_noise_seeded(512, 1)))
                .clone();
            sky.time = 0.0;
            sky.scene_cloud_rotation = 0.0;
        }
    }

    fn pinned_frames(&self, tick: u32) -> u32 {
        self.saved
            .as_ref()
            .map_or(0, |saved| tick.wrapping_sub(saved.tick))
    }
}

/// false selects seed 1 and phase zero; true restores the prior daily noise and phase.
/// Coverage, color, authored speed and scene time are retained. Query without an argument.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/cloud_motion")]
struct CloudMotionCommand {
    enabled: Option<bool>,
}

fn cloud_motion_command(
    mut command: ConsoleCommand<CloudMotionCommand>,
    mut motion: ResMut<CloudMotion>,
    mut sky: AtmosphereMut<NishitaCloud>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(Ok(args)) = command.take() else {
        return;
    };
    if let Some(enabled) = args.enabled {
        motion.set(enabled, &mut sky, &mut images);
    }
    command.reply_ok(
        json!({"enabled": motion.enabled(), "pinnedFrames": motion.pinned_frames(sky.tick)})
            .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinning_reuses_noise_resets_phase_and_restores_live_clouds() {
        let mut images = Assets::<Image>::default();
        let original = images.add(init_noise_seeded(8, 23));
        let mut sky = NishitaCloud {
            noise_texture: original.clone(),
            time: 77.0,
            scene_cloud_rotation: 1.2,
            tick: 17,
            cloudy: 0.42,
            scene_cloud_color: Vec4::ONE,
            scene_cloud_opacity: 0.6,
            ..Default::default()
        };
        let mut motion = CloudMotion::default();
        assert!(motion.enabled());
        motion.set(false, &mut sky, &mut images);
        let pinned = sky.noise_texture.clone();
        assert_ne!(pinned, original);
        assert_eq!((sky.time, sky.scene_cloud_rotation), (0.0, 0.0));
        assert_eq!((sky.cloudy, sky.scene_cloud_opacity), (0.42, 0.6));
        assert_eq!(sky.scene_cloud_color, Vec4::ONE);
        sky.tick += 64;
        assert_eq!(motion.pinned_frames(sky.tick), 64);
        motion.set(false, &mut sky, &mut images);
        assert_eq!(sky.noise_texture, pinned);
        assert_eq!(motion.pinned_frames(sky.tick), 0);
        motion.set(true, &mut sky, &mut images);
        assert!(motion.enabled());
        assert_eq!(sky.noise_texture, original);
        assert_eq!((sky.time, sky.scene_cloud_rotation), (77.0, 1.2));
    }

    #[test]
    fn explicit_noise_seed_is_repeatable_and_distinct() {
        let first = init_noise_seeded(16, 1);
        assert_eq!(first.data, init_noise_seeded(16, 1).data);
        assert_ne!(first.data, init_noise_seeded(16, 2).data);
    }
    #[test]
    fn pinned_frame_keeps_authored_environment_and_resume_advances() {
        use crate::{
            environment::SceneEnvironment,
            sky_envmap::{SkyAmbient, SkyEnvmapSettings},
            CloudCover, FogDensity, ShadowBiasOverride,
        };
        use bevy_atmosphere::prelude::AtmosphereModel;
        use common::structs::{AppConfig, SceneGlobalLight, SceneLoadDistance, TimeOfDay};
        let mut images = Assets::<Image>::default();
        let original = images.add(init_noise_seeded(8, 23));
        let mut sky = NishitaCloud {
            noise_texture: original.clone(),
            time: 77.0,
            scene_cloud_rotation: 1.2,
            ..Default::default()
        };
        let mut motion = CloudMotion::default();
        motion.set(false, &mut sky, &mut images);
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        let mut app = App::new();
        app.insert_resource(time)
            .insert_resource(motion)
            .init_resource::<AppConfig>()
            .insert_resource(TimeOfDay { time: 0.0 })
            .insert_resource(SceneLoadDistance {
                load: 100.0,
                unload: 20.0,
                load_imposter: 0.0,
            })
            .insert_resource(SceneGlobalLight {
                dir_color: Color::WHITE,
                dir_illuminance: 1000.0,
                sun_illuminance: 1000.0,
                dir_direction: Vec3::NEG_Y,
                sun_direction: Vec3::NEG_Y,
                ambient_color: Color::WHITE,
                ambient_brightness: 1.0,
                ..default()
            })
            .insert_resource(AtmosphereModel::new(sky))
            .insert_resource(CloudCover {
                cover: 0.375,
                speed: 5.0,
                density_cap: 0.8,
                shadow: 0.05,
                scale: 1.5,
                steps: 44,
                lacunarity: 2.0,
                sharpness: 50.0,
                sharp_mid: 0.1,
            })
            .init_resource::<AmbientLight>()
            .init_resource::<SkyEnvmapSettings>()
            .init_resource::<SkyAmbient>()
            .init_resource::<FogDensity>()
            .init_resource::<ShadowBiasOverride>()
            .insert_resource(SceneEnvironment {
                cloud_speed: Some(-0.1),
                cloud_color: Some(Vec3::X),
                cloud_opacity: Some(0.6),
                ..default()
            })
            .add_systems(Update, crate::apply_global_light);
        for _ in 0..3 {
            app.update();
        }
        let sky = app
            .world()
            .resource::<AtmosphereModel>()
            .to_ref::<NishitaCloud>()
            .unwrap();
        assert_eq!((sky.time, sky.scene_cloud_rotation), (0.0, 0.0));
        assert_eq!(sky.cloudy, 0.375);
        assert_eq!(sky.scene_cloud_color, Vec3::X.extend(1.0));
        assert_eq!(sky.scene_cloud_opacity, 0.6);
        assert_eq!(app.world().resource::<TimeOfDay>().time, 0.0);
        assert_eq!(
            app.world().resource::<SceneEnvironment>().cloud_speed,
            Some(-0.1)
        );

        // A scene texture still owns the visible sky; procedural cloud overrides stay unused.
        app.world_mut().resource_mut::<SceneEnvironment>().texture = Some(original.clone());
        app.update();
        let sky = app
            .world()
            .resource::<AtmosphereModel>()
            .to_ref::<NishitaCloud>()
            .unwrap();
        assert_eq!(sky.scene_cloud_color, Vec4::ZERO);
        assert_eq!(sky.scene_cloud_opacity, 1.0);
        assert_eq!((sky.time, sky.scene_cloud_rotation), (0.0, 0.0));
        assert_eq!(
            app.world().resource::<SceneEnvironment>().texture.as_ref(),
            Some(&original)
        );

        app.world_mut().resource_mut::<SceneEnvironment>().texture = None;
        let mut motion = app.world_mut().remove_resource::<CloudMotion>().unwrap();
        motion.set(
            true,
            &mut app
                .world_mut()
                .resource_mut::<AtmosphereModel>()
                .to_mut::<NishitaCloud>()
                .unwrap(),
            &mut images,
        );
        app.insert_resource(motion);
        app.update();
        let sky = app
            .world()
            .resource::<AtmosphereModel>()
            .to_ref::<NishitaCloud>()
            .unwrap();
        assert_eq!(sky.noise_texture, original);
        assert_eq!(sky.time, 77.0); // authored angular speed retains the procedural phase
        assert!((sky.scene_cloud_rotation - 1.1).abs() < 0.0001);
        assert_eq!(sky.scene_cloud_opacity, 0.6);
    }
}

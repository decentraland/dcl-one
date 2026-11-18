use bevy::{
    ecs::system::{
        lifetimeless::{SQuery, SRes, Write},
        SystemParamItem,
    },
    pbr::{CascadeShadowConfig, ShadowFilteringMethod},
    platform::collections::HashSet,
    prelude::*,
};
use common::structs::{sun_cascades, AppConfig, PrimaryCameraRes, ShadowSetting, StageLight};

use super::{AppSetting, EnumAppSetting, IntAppSetting};

impl EnumAppSetting for ShadowSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::Low, Self::High]
    }

    fn name(&self) -> String {
        match self {
            ShadowSetting::Off => "Off",
            ShadowSetting::Low => "Low",
            ShadowSetting::High => "High",
        }
        .to_owned()
    }
}

impl AppSetting for ShadowSetting {
    type Param = (
        SRes<AppConfig>,
        SRes<PrimaryCameraRes>,
        SQuery<(
            Write<DirectionalLight>,
            Write<CascadeShadowConfig>,
            Has<StageLight>,
        )>,
    );

    fn title() -> String {
        "Shadow settings".to_owned()
    }

    fn description(&self) -> String {
        format!("How shadows are rendered in the world.\n\n{}", 
        match self {
            ShadowSetting::Off => "Off: No shadows are rendered. Fastest",
            ShadowSetting::Low => "Low: A single shadow map covering the shadow distance, with hardware 2x2 filtering. Fast, but outlines are blocky, particularly at high shadow distances.",
            ShadowSetting::High => "High: Four cascaded soft shadow maps for sharper shadows near the player and better quality at higher shadow distances, but more GPU intensive.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.shadow_settings = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.shadow_settings
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }

    fn apply(
        &self,
        (config, cam_res, mut lights): SystemParamItem<Self::Param>,
        mut commands: Commands,
        cameras: &HashSet<Entity>,
    ) {
        apply_shadows(*self, config.graphics.shadow_distance, &mut lights);

        let res = &(config, cam_res, lights);
        for &cam in cameras {
            self.apply_to_camera(res, commands.reborrow(), cam);
        }
    }

    fn apply_to_camera(
        &self,
        _: &SystemParamItem<Self::Param>,
        mut commands: Commands,
        camera_entity: Entity,
    ) {
        let Ok(mut cmds) = commands.get_entity(camera_entity) else {
            return;
        };
        match self {
            ShadowSetting::Off => (),
            ShadowSetting::Low => {
                cmds.try_insert(ShadowFilteringMethod::Hardware2x2);
            }
            ShadowSetting::High => {
                cmds.try_insert(ShadowFilteringMethod::Gaussian);
            }
        }
    }
}

// The sun's shadows for a quality and a distance (settings of their own, each applying this). A
// stage light keeps its own cascades.
fn apply_shadows(
    setting: ShadowSetting,
    distance: f32,
    lights: &mut Query<(
        &mut DirectionalLight,
        &mut CascadeShadowConfig,
        Has<StageLight>,
    )>,
) {
    let sun = sun_cascades(setting, distance);
    for (mut light, mut cascades, stage) in lights.iter_mut() {
        light.shadows_enabled = sun.is_some();
        if let (Some(config), false) = (&sun, stage) {
            *cascades = config.clone();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ShadowDistanceSetting(i32);

impl IntAppSetting for ShadowDistanceSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        0
    }

    fn max() -> i32 {
        300
    }
}

impl AppSetting for ShadowDistanceSetting {
    type Param = (
        SRes<AppConfig>,
        SQuery<(
            Write<DirectionalLight>,
            Write<CascadeShadowConfig>,
            Has<StageLight>,
        )>,
    );

    fn apply(
        &self,
        (config, mut lights): SystemParamItem<Self::Param>,
        _: Commands,
        _: &HashSet<Entity>,
    ) {
        apply_shadows(ShadowSetting::load(&config), self.0 as f32, &mut lights);
    }

    fn title() -> String {
        "Shadow Distance".to_owned()
    }

    fn description(&self) -> String {
        "Shadow Distance\n\nDistance up to which to render shadows. To ensure that shadows are rendered even for large scenes which are at the edge of the loaded area, this can be set to a higher value (e.g. 2x) than the scene load distance. Higher values increase GPU time slightly, and may result in lower quality shadows at closer distances (particularly with Low shadow quality).".to_owned()
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.graphics.shadow_distance as i32)
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.shadow_distance = self.0 as f32
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LightCountSetting(i32);

impl IntAppSetting for LightCountSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        0
    }

    fn max() -> i32 {
        128
    }
}

impl AppSetting for LightCountSetting {
    type Param = ();

    fn title() -> String {
        "Light Count".to_owned()
    }

    fn description(&self) -> String {
        "Light Count\n\nMaximum number of scene lights (excluding the global sun light) that will be enabled at once. Each enabled light adds GPU cost even without shadows; reduce this if scenes with many lights cause slowdown. Lights from currently active scenes, and those closest/brightest relative to the player, are enabled first.".to_owned()
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.graphics.light_count as i32)
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.light_count = self.0 as usize
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ShadowCasterCountSetting(i32);

impl IntAppSetting for ShadowCasterCountSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        0
    }

    fn max() -> i32 {
        64
    }
}

impl AppSetting for ShadowCasterCountSetting {
    type Param = ();

    fn title() -> String {
        "Shadow Caster Count".to_owned()
    }

    fn description(&self) -> String {
        "Shadow Caster Count\n\nMaximum number of scene lights (excluding the global sun light) that will cast shadows. Lights that cast shadows are expensive in GPU time and memory, the cost scales linearly with the number of shadow-casting lights. Reduce this if scenes with a large number of shadow-casting lights cause slowdown.\n\nOnly spot lights within 20 m of the player cast shadows, nearest first.".to_owned()
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.graphics.shadow_caster_count as i32)
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.shadow_caster_count = self.0 as usize
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn stage_cascades() -> CascadeShadowConfig {
        CascadeShadowConfig {
            bounds: vec![20.0],
            overlap_proportion: 0.2,
            minimum_distance: 0.1,
        }
    }

    fn apply(world: &mut World, setting: ShadowSetting, distance: f32) {
        world
            .run_system_once(
                move |mut lights: Query<(
                    &mut DirectionalLight,
                    &mut CascadeShadowConfig,
                    Has<StageLight>,
                )>| apply_shadows(setting, distance, &mut lights),
            )
            .unwrap();
    }

    #[test]
    fn a_stage_light_follows_shadows_on_or_off_and_keeps_its_cascades() {
        let mut world = World::new();
        let sun = world
            .spawn((DirectionalLight::default(), CascadeShadowConfig::default()))
            .id();
        let stage = world
            .spawn((DirectionalLight::default(), stage_cascades(), StageLight))
            .id();

        apply(&mut world, ShadowSetting::Off, 100.0);
        for light in [sun, stage] {
            assert!(
                !world
                    .get::<DirectionalLight>(light)
                    .unwrap()
                    .shadows_enabled
            );
        }

        apply(&mut world, ShadowSetting::High, 100.0);
        for light in [sun, stage] {
            assert!(
                world
                    .get::<DirectionalLight>(light)
                    .unwrap()
                    .shadows_enabled
            );
        }
        let sun_bounds = &world.get::<CascadeShadowConfig>(sun).unwrap().bounds;
        assert_eq!(sun_bounds.len(), 4);
        assert_eq!(
            world.get::<CascadeShadowConfig>(stage).unwrap().bounds,
            vec![20.0]
        );
    }
}

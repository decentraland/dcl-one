//! The display size exclusive fullscreen switches the monitor to, chosen from the video modes of
//! the monitor the window is on.

use std::{
    cmp::Reverse,
    sync::{Arc, RwLock},
    time::Duration,
};

use bevy::{
    prelude::*,
    time::common_conditions::on_real_timer,
    window::{Monitor, MonitorSelection, PrimaryWindow, VideoMode, VideoModeSelection, WindowMode},
    winit::WinitWindows,
};
use common::structs::{AppConfig, FullscreenSize, WindowSetting};

use super::{NamedVariant, SettingCategory, SettingInfo, Settings};

const DESCRIPTION: &str = "Resolution\n\nThe display size in Fullscreen window mode. Larger sizes are sharper and cost more GPU time. Only Fullscreen uses it: a window or a borderless fullscreen window keeps the monitor's own resolution.";

#[derive(Default)]
struct ModeList {
    /// One mode per size, largest first.
    modes: Vec<VideoMode>,
    /// The monitor's mode when it was enumerated, before this app switched it.
    desktop: Option<VideoMode>,
}

impl ModeList {
    fn position(&self, size: UVec2) -> Option<usize> {
        self.modes.iter().position(|m| m.physical_size == size)
    }

    /// Falls back to the desktop mode when this monitor lacks the stored size.
    fn selected(&self, saved: Option<FullscreenSize>) -> Option<usize> {
        saved
            .and_then(|s| self.position(UVec2::new(s.width, s.height)))
            .or_else(|| self.desktop.and_then(|d| self.position(d.physical_size)))
    }

    /// Whether the choices open with "Desktop": the monitor's current mode, kept as it is. Offered
    /// when no listed mode is the desktop's (an X11 output rotated to portrait reports its size
    /// turned while its modes keep theirs), so an unsaved size still has a choice to show.
    fn desktop_choice(&self) -> bool {
        !self.modes.is_empty()
            && self
                .desktop
                .and_then(|d| self.position(d.physical_size))
                .is_none()
    }

    /// The choice shown for `saved`, `None` until the monitor is read.
    fn index(&self, saved: Option<FullscreenSize>) -> Option<usize> {
        if self.desktop_choice() {
            Some(self.selected(saved).map_or(0, |ix| ix + 1))
        } else {
            self.selected(saved)
        }
    }

    /// The size saved for choice `index`; "Desktop" saves none.
    fn size_at(&self, index: usize) -> anyhow::Result<Option<FullscreenSize>> {
        let index = match (self.desktop_choice(), index) {
            (true, 0) => return Ok(None),
            (true, ix) => ix - 1,
            (false, ix) => ix,
        };
        let mode = self
            .modes
            .get(index)
            .ok_or(anyhow::anyhow!("invalid variant index"))?;
        Ok(Some(FullscreenSize {
            width: mode.physical_size.x,
            height: mode.physical_size.y,
        }))
    }

    /// The video mode exclusive fullscreen asks for. Only a mode the monitor reported is ever
    /// named: winit ignores the switch when an exact match is missing.
    fn fullscreen_mode(&self, saved: Option<FullscreenSize>) -> VideoModeSelection {
        self.selected(saved)
            .map(|ix| VideoModeSelection::Specific(self.modes[ix]))
            .unwrap_or(VideoModeSelection::Current)
    }
}

/// The modes of the primary window's monitor, shared with the setting's closures.
#[derive(Resource, Default)]
struct MonitorModes {
    monitor: Option<Entity>,
    list: Arc<RwLock<ModeList>>,
}

fn offered(size: UVec2) -> bool {
    let (w, h) = (u64::from(size.x), u64::from(size.y));
    let aspect = [(16, 9), (16, 10), (9, 16), (10, 16)]
        .iter()
        .any(|&(a, b)| w * b == h * a);
    aspect && w.min(h) > 1024
}

fn pick_mode(all: &[VideoMode], size: UVec2, preferred_refresh: Option<u32>) -> Option<VideoMode> {
    let at_size = || all.iter().filter(move |m| m.physical_size == size);
    let refresh = preferred_refresh
        .filter(|r| at_size().any(|m| m.refresh_rate_millihertz == *r))
        .or_else(|| at_size().map(|m| m.refresh_rate_millihertz).max())?;
    at_size()
        .filter(|m| m.refresh_rate_millihertz == refresh)
        .max_by_key(|m| m.bit_depth)
        .copied()
}

fn list_modes(all: &[VideoMode], desktop: Option<VideoMode>) -> ModeList {
    let mut sizes: Vec<UVec2> = all
        .iter()
        .map(|m| m.physical_size)
        .filter(|s| offered(*s))
        .chain(desktop.map(|d| d.physical_size))
        .collect();
    sizes.sort_by_key(|s| Reverse((u64::from(s.x) * u64::from(s.y), s.x)));
    sizes.dedup();
    let preferred_refresh = desktop.map(|d| d.refresh_rate_millihertz);
    ModeList {
        modes: sizes
            .into_iter()
            .filter_map(|size| pick_mode(all, size, preferred_refresh))
            .collect(),
        desktop,
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn label(size: UVec2) -> String {
    if offered(size) {
        let g = gcd(size.x, size.y);
        format!("{}x{} ({}:{})", size.x, size.y, size.x / g, size.y / g)
    } else {
        format!("{}x{}", size.x, size.y)
    }
}

/// The choices shown for `list`. Until the monitor is read (or when it reports no usable mode)
/// the one choice is the desktop, shown greyed out by the negative value: a list keeps the panel
/// on its dropdown, where no choices would read as an off/on switch.
fn variants(list: &ModeList) -> Vec<NamedVariant> {
    let named = |name: String| NamedVariant {
        name,
        description: DESCRIPTION.to_owned(),
    };
    let desktop = (list.modes.is_empty() || list.desktop_choice()).then(|| named("Desktop".into()));
    desktop
        .into_iter()
        .chain(list.modes.iter().map(|m| named(label(m.physical_size))))
        .collect()
}

/// `Monitor` keeps the size and rate it had when enumerated, before any exclusive switch.
fn desktop_mode(monitor: &Monitor) -> Option<VideoMode> {
    let size = UVec2::new(monitor.physical_width, monitor.physical_height);
    monitor
        .video_modes
        .iter()
        .filter(|m| {
            m.physical_size == size
                && Some(m.refresh_rate_millihertz) == monitor.refresh_rate_millihertz
        })
        .max_by_key(|m| m.bit_depth)
        .copied()
}

pub(super) fn register(app: &mut App, settings: &mut Settings, config: &AppConfig) {
    let state = MonitorModes::default();
    let variants_list = state.list.clone();
    let load_list = state.list.clone();
    let save_list = state.list.clone();
    let disabled = |config: &AppConfig| config.graphics.window != WindowSetting::Fullscreen;

    settings.add_listed_setting(
        SettingInfo {
            name: "Resolution".to_owned(),
            category: SettingCategory::Graphics.to_string(),
            description: DESCRIPTION.to_owned(),
            min_value: 0.0,
            max_value: 1.0,
            named_variants: variants(&ModeList::default()),
            step_size: 1.0,
            value: -1.0,
            default: -1.0,
            disabled: disabled(config),
        },
        move || variants(&variants_list.read().unwrap()),
        // negative (shown greyed out) until the monitor is known
        move |config: &AppConfig| {
            load_list
                .read()
                .unwrap()
                .index(config.graphics.fullscreen_size)
                .map_or(-1.0, |ix| ix as f32)
        },
        move |config: &mut AppConfig, index: usize| {
            config.graphics.fullscreen_size = save_list.read().unwrap().size_at(index)?;
            Ok(())
        },
        disabled,
    );

    app.insert_resource(state);
    app.add_systems(
        Update,
        (
            // every frame until the monitor is known, then once a second to follow the window
            read_monitor_modes.run_if(
                (|state: Res<MonitorModes>| state.monitor.is_none())
                    .or(on_real_timer(Duration::from_secs(1))),
            ),
            apply_window_mode
                .run_if(resource_changed::<AppConfig>.or(resource_changed::<MonitorModes>)),
            refresh_listed_settings.run_if(resource_changed::<MonitorModes>),
        )
            .chain(),
    );
}

fn read_monitor_modes(
    windows: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    monitors: Query<(Entity, &Monitor)>,
    mut state: ResMut<MonitorModes>,
) {
    let Some(current) = windows
        .single()
        .ok()
        .and_then(|window| winit_windows.get_window(window))
        .and_then(|window| window.current_monitor())
    else {
        return;
    };
    let (name, position) = (current.name(), current.position());
    let Some((entity, monitor)) = monitors
        .iter()
        .find(|(_, m)| m.name == name && m.physical_position == IVec2::new(position.x, position.y))
    else {
        return;
    };
    if state.monitor == Some(entity) {
        return;
    }
    let list = list_modes(&monitor.video_modes, desktop_mode(monitor));
    debug!(
        "monitor {:?}: fullscreen sizes {:?}, desktop {:?}",
        monitor.name,
        list.modes
            .iter()
            .map(|m| format!(
                "{} @ {} mHz",
                label(m.physical_size),
                m.refresh_rate_millihertz
            ))
            .collect::<Vec<_>>(),
        list.desktop
    );
    *state.list.write().unwrap() = list;
    state.monitor = Some(entity);
}

fn apply_window_mode(
    config: Res<AppConfig>,
    state: Res<MonitorModes>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let mode = match config.graphics.window {
        WindowSetting::Fullscreen => WindowMode::Fullscreen(
            MonitorSelection::Current,
            state
                .list
                .read()
                .unwrap()
                .fullscreen_mode(config.graphics.fullscreen_size),
        ),
        WindowSetting::Windowed => WindowMode::Windowed,
        WindowSetting::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
    };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if window.mode != mode {
        debug!("window mode {mode:?}");
        window.mode = mode;
    }
}

fn refresh_listed_settings(mut settings: ResMut<Settings>, config: Res<AppConfig>) {
    settings.refresh(&config);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(w: u32, h: u32, hz: u32, depth: u16) -> VideoMode {
        VideoMode {
            physical_size: UVec2::new(w, h),
            bit_depth: depth,
            refresh_rate_millihertz: hz * 1000,
        }
    }

    #[test]
    fn keeps_wide_and_tall_sizes_over_1024_largest_first() {
        let all = [
            mode(1280, 720, 60, 32),
            mode(1600, 900, 60, 32),
            mode(1920, 1080, 60, 32),
            mode(1920, 1200, 60, 32),
            mode(1366, 768, 60, 32),
            mode(2560, 1440, 60, 32),
            mode(1080, 1920, 60, 32),
            mode(1280, 1024, 60, 32),
        ];
        let list = list_modes(&all, None);
        let sizes: Vec<_> = list.modes.iter().map(|m| label(m.physical_size)).collect();
        assert_eq!(
            sizes,
            [
                "2560x1440 (16:9)",
                "1920x1200 (8:5)",
                "1920x1080 (16:9)",
                "1080x1920 (9:16)"
            ]
        );
    }

    #[test]
    fn desktop_size_is_always_offered_and_selected_when_unset() {
        let desktop = mode(3024, 1964, 120, 32);
        let all = [desktop, mode(1920, 1080, 60, 32), mode(1512, 982, 120, 32)];
        let list = list_modes(&all, Some(desktop));
        assert_eq!(list.modes, [desktop, mode(1920, 1080, 60, 32)]);
        assert_eq!(label(desktop.physical_size), "3024x1964");
        assert_eq!(list.selected(None), Some(0));
        assert_eq!(
            list.fullscreen_mode(None),
            VideoModeSelection::Specific(desktop)
        );
    }

    #[test]
    fn prefers_the_desktop_refresh_rate_then_the_highest_at_the_deepest_colour() {
        let desktop = mode(2560, 1440, 144, 32);
        let all = [
            desktop,
            mode(1920, 1080, 60, 32),
            mode(1920, 1080, 144, 16),
            mode(1920, 1080, 144, 32),
            mode(2560, 1600, 60, 32),
            mode(2560, 1600, 120, 30),
            mode(2560, 1600, 120, 32),
        ];
        let list = list_modes(&all, Some(desktop));
        assert_eq!(
            list.modes,
            [
                mode(2560, 1600, 120, 32),
                desktop,
                mode(1920, 1080, 144, 32)
            ]
        );
    }

    #[test]
    fn a_stored_size_missing_from_the_monitor_falls_back_to_the_desktop_mode() {
        let desktop = mode(1920, 1080, 60, 32);
        let list = list_modes(&[desktop, mode(2560, 1440, 60, 32)], Some(desktop));
        let saved = |width, height| Some(FullscreenSize { width, height });
        assert_eq!(
            list.fullscreen_mode(saved(2560, 1440)),
            VideoModeSelection::Specific(mode(2560, 1440, 60, 32))
        );
        assert_eq!(
            list.fullscreen_mode(saved(3840, 2160)),
            VideoModeSelection::Specific(desktop)
        );
        assert_eq!(
            ModeList::default().fullscreen_mode(saved(2560, 1440)),
            VideoModeSelection::Current
        );
    }

    #[test]
    fn a_desktop_size_matching_no_listed_mode_is_offered_as_desktop() {
        // a portrait output: its size is turned, its modes are not
        let all = [mode(1920, 1080, 60, 32), mode(2560, 1440, 60, 32)];
        let saved = |width, height| Some(FullscreenSize { width, height });
        for desktop in [None, Some(mode(1080, 1920, 60, 32))] {
            let list = list_modes(&all, desktop);
            let names: Vec<_> = variants(&list).into_iter().map(|v| v.name).collect();
            assert_eq!(names, ["Desktop", "2560x1440 (16:9)", "1920x1080 (16:9)"]);
            assert_eq!(list.index(None), Some(0));
            assert_eq!(list.index(saved(1920, 1080)), Some(2));
            assert_eq!(list.index(saved(3840, 2160)), Some(0));
            assert_eq!(list.size_at(0).unwrap(), None);
            assert_eq!(list.size_at(2).unwrap(), saved(1920, 1080));
            assert!(list.size_at(3).is_err());
            assert_eq!(list.fullscreen_mode(None), VideoModeSelection::Current);
        }
        // a listed desktop needs no extra choice
        let desktop = mode(1920, 1080, 60, 32);
        let list = list_modes(&all, Some(desktop));
        assert!(!list.desktop_choice());
        assert_eq!(list.index(None), Some(1));
        assert_eq!(list.size_at(0).unwrap(), saved(2560, 1440));
        assert_eq!(ModeList::default().index(None), None);
    }

    #[test]
    fn the_setting_lists_the_monitor_and_greys_out_outside_fullscreen() {
        let mut app = App::new();
        let mut settings = Settings {
            settings: Vec::new(),
        };
        let mut config = AppConfig::default();
        register(&mut app, &mut settings, &config);
        let info = |settings: &Settings| settings.settings[0].info.clone();
        assert_eq!(info(&settings).value, -1.0);
        assert!(info(&settings).disabled);
        // before the monitor is read: one greyed-out choice, so the panel shows a dropdown
        let pending = |settings: &Settings| {
            let info = info(settings);
            assert_eq!(info.named_variants.len(), 1);
            assert_eq!(info.named_variants[0].name, "Desktop");
            assert_eq!(info.max_value, 1.0);
            assert_eq!(info.value, -1.0);
        };
        pending(&settings);
        settings.refresh(&config);
        pending(&settings);
        assert!(settings.set_value(&mut config, "Resolution", 0.0).is_err());

        let desktop = mode(1920, 1080, 60, 32);
        *app.world().resource::<MonitorModes>().list.write().unwrap() =
            list_modes(&[desktop, mode(2560, 1440, 60, 32)], Some(desktop));
        config.graphics.window = WindowSetting::Fullscreen;
        settings.refresh(&config);
        let shown = info(&settings);
        assert_eq!(shown.named_variants.len(), 2);
        assert_eq!(shown.max_value, 2.0);
        assert_eq!(shown.value, 1.0);
        assert_eq!(shown.default, 1.0);
        assert!(!shown.disabled);

        settings.set_value(&mut config, "Resolution", 0.0).unwrap();
        assert_eq!(
            config.graphics.fullscreen_size,
            Some(FullscreenSize {
                width: 2560,
                height: 1440
            })
        );
        assert!(settings.set_value(&mut config, "Resolution", 2.0).is_err());
        // a failed set leaves the setting usable
        settings.set_value(&mut config, "Resolution", 1.0).unwrap();
        assert_eq!(
            config.graphics.fullscreen_size,
            Some(FullscreenSize {
                width: 1920,
                height: 1080
            })
        );
    }
}

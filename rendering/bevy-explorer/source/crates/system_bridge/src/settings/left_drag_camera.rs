use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum LeftDragCameraSetting {
    Off,
    On,
}

impl EnumAppSetting for LeftDragCameraSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            LeftDragCameraSetting::Off => "Off",
            LeftDragCameraSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for LeftDragCameraSetting {
    type Param = ();

    fn title() -> String {
        "Left-Drag Camera".to_owned()
    }

    fn description(&self) -> String {
        format!(
            "Whether holding the left mouse button and dragging turns the camera.\n\nA click without moving is still a click, and the right button works as before.\n\n{}",
            match self {
                LeftDragCameraSetting::Off => "Off: only the right button turns the camera.",
                LeftDragCameraSetting::On => {
                    "On: dragging with the left button also turns the camera while it is held."
                }
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.left_drag_camera = *self == LeftDragCameraSetting::On;
    }

    fn load(config: &AppConfig) -> Self {
        if config.left_drag_camera {
            Self::On
        } else {
            Self::Off
        }
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Gameplay
    }
}

use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum SpringBonesSetting {
    Off,
    On,
}

impl EnumAppSetting for SpringBonesSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            SpringBonesSetting::Off => "Off",
            SpringBonesSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for SpringBonesSetting {
    type Param = ();

    fn title() -> String {
        "Jiggle Bones".to_owned()
    }

    fn description(&self) -> String {
        "Physics-based secondary motion on avatar accessories such as hair, tails and capes.\n\nOn: the extra bones of wearables swing as the avatar moves, on the nearest avatars. Off: they stay rigid.".to_owned()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.spring_bones = matches!(self, SpringBonesSetting::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.graphics.spring_bones {
            Self::On
        } else {
            Self::Off
        }
    }
}

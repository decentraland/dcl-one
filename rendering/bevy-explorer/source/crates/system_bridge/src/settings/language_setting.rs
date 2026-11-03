use bevy::{ecs::system::lifetimeless::SResMut, platform::collections::HashSet, prelude::*};
use common::structs::{AppConfig, PlayerLanguage};
use system_api_types::languages::TRANSLATION_LANGUAGES;

use super::{AppSetting, EnumAppSetting, SettingCategory};

/// An index into `TRANSLATION_LANGUAGES`; `None` follows the system language.
#[derive(Debug, PartialEq, Eq)]
pub struct LanguageSetting(Option<usize>);

impl LanguageSetting {
    fn code(&self) -> Option<&'static str> {
        self.0.map(|i| TRANSLATION_LANGUAGES[i].code)
    }
}

impl EnumAppSetting for LanguageSetting {
    fn variants() -> Vec<Self> {
        std::iter::once(Self(None))
            .chain((0..TRANSLATION_LANGUAGES.len()).map(|i| Self(Some(i))))
            .collect()
    }

    fn name(&self) -> String {
        match self.0 {
            Some(i) => TRANSLATION_LANGUAGES[i].native.to_owned(),
            None => match platform::system_language() {
                Some(tag) => format!("System ({tag})"),
                None => "System".to_owned(),
            },
        }
    }
}

impl AppSetting for LanguageSetting {
    type Param = SResMut<PlayerLanguage>;

    fn title() -> String {
        "Language".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Chat
    }

    fn description(&self) -> String {
        "The language chat messages are translated into, also reported to scenes so they can show their own text in it.\n\nSystem follows your device's language.".to_owned()
    }

    fn save(&self, config: &mut AppConfig) {
        config.language = self.code().map(str::to_owned);
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.language.as_deref().and_then(|language| {
            TRANSLATION_LANGUAGES
                .iter()
                .position(|l| l.code.eq_ignore_ascii_case(language))
        }))
    }

    fn apply(&self, mut language: ResMut<PlayerLanguage>, _: Commands, _: &HashSet<Entity>) {
        let resolved = self
            .code()
            .map(str::to_owned)
            .or_else(platform::system_language);
        language.set_if_neq(PlayerLanguage(resolved));
    }
}

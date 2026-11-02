//! The chat translation server's target languages (autotranslate-server `GET /languages`), each
//! with its English name and its name in itself (CLDR), so a player finds theirs without reading
//! English. Kept as a table rather than asked of `Intl.DisplayNames`: Chromium's trimmed locale
//! data lacks some. The engine's Language setting offers them; the HUD translates into them.
//! scripts/gen-ts-bindings.sh writes the table into react-web/src/engine/generated as
//! `languageTable.ts`.

use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, ts_rs::TS)]
#[ts(export)]
pub struct TranslationLanguage {
    /// the server's code, a BCP-47 tag
    pub code: &'static str,
    pub name: &'static str,
    pub native: &'static str,
}

/// In order of `native`, the order the Language setting lists them.
pub const TRANSLATION_LANGUAGES: &[TranslationLanguage] = &[
    TranslationLanguage {
        code: "az",
        name: "Azerbaijani",
        native: "Azərbaycan",
    },
    TranslationLanguage {
        code: "ca",
        name: "Catalan",
        native: "Català",
    },
    TranslationLanguage {
        code: "da",
        name: "Danish",
        native: "Dansk",
    },
    TranslationLanguage {
        code: "de",
        name: "German",
        native: "Deutsch",
    },
    TranslationLanguage {
        code: "et",
        name: "Estonian",
        native: "Eesti",
    },
    TranslationLanguage {
        code: "en",
        name: "English",
        native: "English",
    },
    TranslationLanguage {
        code: "es",
        name: "Spanish",
        native: "Español",
    },
    TranslationLanguage {
        code: "eo",
        name: "Esperanto",
        native: "Esperanto",
    },
    TranslationLanguage {
        code: "eu",
        name: "Basque",
        native: "Euskara",
    },
    TranslationLanguage {
        code: "tl",
        name: "Tagalog",
        native: "Filipino",
    },
    TranslationLanguage {
        code: "fr",
        name: "French",
        native: "Français",
    },
    TranslationLanguage {
        code: "ga",
        name: "Irish",
        native: "Gaeilge",
    },
    TranslationLanguage {
        code: "gl",
        name: "Galician",
        native: "Galego",
    },
    TranslationLanguage {
        code: "id",
        name: "Indonesian",
        native: "Indonesia",
    },
    TranslationLanguage {
        code: "it",
        name: "Italian",
        native: "Italiano",
    },
    TranslationLanguage {
        code: "lv",
        name: "Latvian",
        native: "Latviešu",
    },
    TranslationLanguage {
        code: "lt",
        name: "Lithuanian",
        native: "Lietuvių",
    },
    TranslationLanguage {
        code: "hu",
        name: "Hungarian",
        native: "Magyar",
    },
    TranslationLanguage {
        code: "ms",
        name: "Malay",
        native: "Melayu",
    },
    TranslationLanguage {
        code: "nl",
        name: "Dutch",
        native: "Nederlands",
    },
    TranslationLanguage {
        code: "nb",
        name: "Norwegian",
        native: "Norsk bokmål",
    },
    TranslationLanguage {
        code: "pl",
        name: "Polish",
        native: "Polski",
    },
    TranslationLanguage {
        code: "pt",
        name: "Portuguese",
        native: "Português",
    },
    TranslationLanguage {
        code: "pt-BR",
        name: "Portuguese (Brazil)",
        native: "Português (Brasil)",
    },
    TranslationLanguage {
        code: "ro",
        name: "Romanian",
        native: "Română",
    },
    TranslationLanguage {
        code: "sq",
        name: "Albanian",
        native: "Shqip",
    },
    TranslationLanguage {
        code: "sk",
        name: "Slovak",
        native: "Slovenčina",
    },
    TranslationLanguage {
        code: "sl",
        name: "Slovenian",
        native: "Slovenščina",
    },
    TranslationLanguage {
        code: "fi",
        name: "Finnish",
        native: "Suomi",
    },
    TranslationLanguage {
        code: "sv",
        name: "Swedish",
        native: "Svenska",
    },
    TranslationLanguage {
        code: "tr",
        name: "Turkish",
        native: "Türkçe",
    },
    TranslationLanguage {
        code: "cs",
        name: "Czech",
        native: "Čeština",
    },
    TranslationLanguage {
        code: "el",
        name: "Greek",
        native: "Ελληνικά",
    },
    TranslationLanguage {
        code: "bg",
        name: "Bulgarian",
        native: "Български",
    },
    TranslationLanguage {
        code: "ky",
        name: "Kyrgyz",
        native: "Кыргызча",
    },
    TranslationLanguage {
        code: "ru",
        name: "Russian",
        native: "Русский",
    },
    TranslationLanguage {
        code: "uk",
        name: "Ukrainian",
        native: "Українська",
    },
    TranslationLanguage {
        code: "he",
        name: "Hebrew",
        native: "עברית",
    },
    TranslationLanguage {
        code: "ur",
        name: "Urdu",
        native: "اردو",
    },
    TranslationLanguage {
        code: "ar",
        name: "Arabic",
        native: "العربية",
    },
    TranslationLanguage {
        code: "fa",
        name: "Persian",
        native: "فارسی",
    },
    TranslationLanguage {
        code: "hi",
        name: "Hindi",
        native: "हिन्दी",
    },
    TranslationLanguage {
        code: "bn",
        name: "Bengali",
        native: "বাংলা",
    },
    TranslationLanguage {
        code: "th",
        name: "Thai",
        native: "ไทย",
    },
    TranslationLanguage {
        code: "ja",
        name: "Japanese",
        native: "日本語",
    },
    TranslationLanguage {
        code: "zh-Hans",
        name: "Chinese (Simplified)",
        native: "简体中文",
    },
    TranslationLanguage {
        code: "zh-Hant",
        name: "Chinese (Traditional)",
        native: "繁體中文",
    },
    TranslationLanguage {
        code: "ko",
        name: "Korean",
        native: "한국어",
    },
];

#[cfg(test)]
mod tests {
    use super::TRANSLATION_LANGUAGES;

    /// Writes the table's VALUES next to the ts-rs types (`TS_RS_EXPORT_DIR`, set by
    /// scripts/gen-ts-bindings.sh — a no-op under a plain `cargo test`).
    #[test]
    fn export_language_table() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else {
            return;
        };
        let json = serde_json::to_string_pretty(TRANSLATION_LANGUAGES).unwrap();
        let ts = format!(
            "// GENERATED by scripts/gen-ts-bindings.sh from crates/system_api_types/src/languages.rs — do not edit.\n\
             import type {{ TranslationLanguage }} from './TranslationLanguage'\n\n\
             export const LANGUAGES: readonly TranslationLanguage[] = {json}\n"
        );
        std::fs::write(std::path::Path::new(&dir).join("languageTable.ts"), ts).unwrap();
    }
}

export const SITES_LANGUAGES = [
  { code: "en", label: "English", flag: "🇬🇧" },
  { code: "es", label: "Español", flag: "🇪🇸" },
  { code: "fr", label: "Français", flag: "🇫🇷" },
  { code: "zh", label: "中文", flag: "🇨🇳" },
  { code: "ko", label: "한국어", flag: "🇰🇷" },
  { code: "ja", label: "日本語", flag: "🇯🇵" },
] as const;

export type SitesLocale = (typeof SITES_LANGUAGES)[number]["code"];

export function sitesLanguage(code: string | null | undefined) {
  return (
    SITES_LANGUAGES.find((language) => language.code === code) ??
    SITES_LANGUAGES[0]
  );
}

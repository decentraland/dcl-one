import { createContext } from "react";
import enFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1ec-1f1e7.svg";
import esFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1ea-1f1f8.svg";
import frFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1eb-1f1f7.svg";
import zhFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1e8-1f1f3.svg";
import koFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1f0-1f1f7.svg";
import jaFlag from "@dcl/one-ui/apps/sites/assets/twemoji-1f1ef-1f1f5.svg";

export const SITES_LANGUAGES = [
  { code: "en", label: "English", flag: enFlag },
  { code: "es", label: "Español", flag: esFlag },
  { code: "fr", label: "Français", flag: frFlag },
  { code: "zh", label: "中文", flag: zhFlag },
  { code: "ko", label: "한국어", flag: koFlag },
  { code: "ja", label: "日本語", flag: jaFlag },
] as const;

export type SitesLocale = (typeof SITES_LANGUAGES)[number]["code"];

export function sitesLanguage(code: string | null | undefined) {
  return (
    SITES_LANGUAGES.find((language) => language.code === code) ??
    SITES_LANGUAGES[0]
  );
}

export const SitesLocaleContext = createContext<{
  locale: SitesLocale;
  onSelect: (locale: SitesLocale) => void;
} | null>(null);

import { DEFAULT_LOCALE, type Locale } from "../locales";
import dclOneEnglish from "./dcl-one.en.json";
import enOnly from "./en-only.json";

export type Catalog = Record<string, string>;

// One chunk per locale: a visitor downloads only the language they use.
const loaders: Record<Locale, () => Promise<{ default: Catalog }>> = {
  en: () => import("./en.json"),
  es: () => import("./es.json"),
  fr: () => import("./fr.json"),
  zh: () => import("./zh.json"),
  ko: () => import("./ko.json"),
  ja: () => import("./ja.json"),
};

export type LocaleMessages = {
  locale: Locale;
  messages: Catalog;
  // English text for the keys this locale documents as untranslated (en-only.json).
  fallback: Catalog;
};

/**
 * Keys dcl-one adds that source Sites never had (accessible and legal labels, adapted link text).
 * They are English in every locale until translated; the import script does not touch them.
 */
export const DCL_ONE_KEYS: Catalog = dclOneEnglish;

export async function loadLocaleMessages(
  locale: Locale,
): Promise<LocaleMessages> {
  const messages = { ...(await loaders[locale]()).default, ...DCL_ONE_KEYS };
  const missing = (enOnly as Partial<Record<Locale, string[]>>)[locale] ?? [];
  if (locale === DEFAULT_LOCALE || missing.length === 0)
    return { locale, messages, fallback: {} };
  const english = (await loaders.en()).default;
  return {
    locale,
    messages,
    fallback: Object.fromEntries(
      missing.map((key) => [key, english[key] ?? key]),
    ),
  };
}

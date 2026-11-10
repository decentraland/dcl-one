import { parseCookies } from "@/core/lib/telemetry/session";

// The six locales source Sites ships for these pages (contract §9). English is the fallback.
export const LOCALES = ["en", "es", "fr", "zh", "ko", "ja"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

// The persisted choice. The cookie mirrors it so the server renders the same locale the browser
// shows; localStorage keeps the key source Sites used, for visitors who chose before the cookie.
export const LOCALE_COOKIE = "dcl-locale";
export const LOCALE_STORAGE_KEY = "dcl-locale";

// The legacy SitesChrome switcher on the other Sites pages: three languages, its own storage key
// and cookie. A choice here is mirrored there only when that switcher can show it.
export const LEGACY_LOCALES: readonly Locale[] = ["en", "es", "zh"];
export const LEGACY_STORAGE_KEY = "dcl:lang";
export const LEGACY_COOKIE = "lang";

/** A supported locale from a tag such as "es", "es-AR" or "zh_Hant", else undefined. */
export function toLocale(value: string | null | undefined): Locale | undefined {
  const base = value?.trim().toLowerCase().split(/[-_]/)[0];
  return LOCALES.find((locale) => locale === base);
}

/** The first supported locale of an Accept-Language header, by quality. */
export function localeFromAcceptLanguage(
  header: string | null | undefined,
): Locale | undefined {
  if (!header) return undefined;
  const ranked = header
    .split(",")
    .map((part, index) => {
      const [tag = "", ...params] = part.trim().split(";");
      const q = params.map((p) => p.trim()).find((p) => p.startsWith("q="));
      const quality = q ? Number(q.slice(2)) : 1;
      return { tag, quality: Number.isFinite(quality) ? quality : 0, index };
    })
    .filter((entry) => entry.quality > 0)
    .sort((a, b) => b.quality - a.quality || a.index - b.index);
  for (const entry of ranked) {
    const locale = toLocale(entry.tag);
    if (locale) return locale;
  }
  return undefined;
}

/** The locale a request renders in: the persisted choice, then the browser languages, then English. */
export function resolveRequestLocale(request: Request): Locale {
  return (
    toLocale(parseCookies(request.headers.get("cookie"))[LOCALE_COOKIE]) ??
    localeFromAcceptLanguage(request.headers.get("accept-language")) ??
    DEFAULT_LOCALE
  );
}

export function serializeLocaleCookie(locale: Locale, secure: boolean): string {
  return [
    `${LOCALE_COOKIE}=${locale}`,
    "Path=/",
    "SameSite=Lax",
    "Max-Age=31536000",
  ]
    .concat(secure ? ["Secure"] : [])
    .join("; ");
}

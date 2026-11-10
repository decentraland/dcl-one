import { useSyncExternalStore } from "react";
import { useMatches } from "react-router";

import { DEFAULT_LOCALE, toLocale, type Locale } from "./locales";

type MatchData =
  { locale?: unknown; messages?: { locale?: unknown } } | null | undefined;

/**
 * The language of the server-rendered document: the locale the deepest localized route resolved
 * (its loader returns `messages`), else English for the pages that are not localized.
 */
export function documentLocale(
  matches: readonly { data?: unknown; loaderData?: unknown }[],
): Locale {
  for (let i = matches.length - 1; i >= 0; i -= 1) {
    const match = matches[i]!;
    const data = (match.loaderData ?? match.data) as MatchData;
    const locale = toLocale(
      typeof data?.messages?.locale === "string"
        ? data.messages.locale
        : undefined,
    );
    if (locale) return locale;
  }
  return DEFAULT_LOCALE;
}

// The locale a mounted LocaleProvider shows after a switch in this browser. The root owns
// <html lang> and reads it, so a page that is not localized never inherits a switched language.
let active: Locale | null = null;
const listeners = new Set<() => void>();

export function setActiveLocale(locale: Locale | null): void {
  if (active === locale) return;
  active = locale;
  for (const listener of listeners) listener();
}

export function activeLocale(): Locale | null {
  return active;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The document language: a mounted provider's current locale, else what the route resolved. */
export function useDocumentLocale(): Locale {
  const routeLocale = documentLocale(useMatches());
  const providerLocale = useSyncExternalStore(
    subscribe,
    activeLocale,
    () => null,
  );
  return providerLocale ?? routeLocale;
}

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { loadLocaleMessages, type LocaleMessages } from "./catalogs";
import { setActiveLocale } from "./document-locale";
import { formatMessage, type MessageValues } from "./format";
import {
  LEGACY_COOKIE,
  LEGACY_LOCALES,
  LEGACY_STORAGE_KEY,
  LOCALE_COOKIE,
  LOCALE_STORAGE_KEY,
  serializeLocaleCookie,
  toLocale,
  type Locale,
} from "./locales";

type LocaleContextValue = {
  locale: Locale;
  /** The catalog text for `key`, with `{name}` values and `<tag>` handlers applied. */
  t: (key: string, values?: MessageValues) => ReactNode;
  /** Plain-text form, for attributes such as aria-label. */
  text: (key: string, values?: Record<string, string | number>) => string;
  /** Switches without a reload; the latest request wins and a failed load can be retried. */
  setLocale: (locale: Locale) => Promise<void>;
};

const LocaleContext = createContext<LocaleContextValue | null>(null);

type Loader = (locale: Locale) => Promise<LocaleMessages>;

function readStored(): Locale | undefined {
  try {
    return toLocale(localStorage.getItem(LOCALE_STORAGE_KEY));
  } catch {
    return undefined;
  }
}

function persist(locale: Locale): void {
  const legacy = LEGACY_LOCALES.includes(locale);
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    if (legacy) localStorage.setItem(LEGACY_STORAGE_KEY, locale);
  } catch {
    // Storage blocked: the cookie still carries the choice.
  }
  document.cookie = serializeLocaleCookie(
    locale,
    location.protocol === "https:",
  );
  // The legacy switcher's cookie, as it writes it.
  if (legacy)
    document.cookie = `${LEGACY_COOKIE}=${locale}; path=/; max-age=31536000; samesite=lax`;
}

function hasLocaleCookie(): boolean {
  return document.cookie
    .split(";")
    .some((part) => part.trim().startsWith(`${LOCALE_COOKIE}=`));
}

/**
 * Renders with the locale and messages the server resolved, so hydration matches. After
 * hydration, a choice stored only in the browser (before the cookie existed) is applied once.
 */
export function LocaleProvider({
  initial,
  load = loadLocaleMessages,
  children,
}: {
  initial: LocaleMessages;
  load?: Loader;
  children: ReactNode;
}) {
  const [current, setCurrent] = useState(initial);
  const requested = useRef<Locale>(initial.locale);

  const setLocale = useCallback(
    async (next: Locale) => {
      requested.current = next;
      const loaded = await load(next);
      if (requested.current !== next) return;
      setCurrent(loaded);
      persist(next);
    },
    [load],
  );

  // The root's <html lang> follows this page's language (whichever way it was resolved), and drops
  // back to the route's own once this provider unmounts, e.g. on a page that is not localized.
  useEffect(() => {
    setActiveLocale(current.locale);
  }, [current.locale]);
  useEffect(() => () => setActiveLocale(null), []);

  useEffect(() => {
    if (hasLocaleCookie()) return;
    const stored = readStored();
    if (stored && stored !== initial.locale)
      void setLocale(stored).catch(() => undefined);
    else if (stored) persist(stored);
  }, [initial.locale, setLocale]);

  const value = useMemo<LocaleContextValue>(() => {
    const raw = (key: string) =>
      current.messages[key] ?? current.fallback[key] ?? key;
    return {
      locale: current.locale,
      t: (key, values) => formatMessage(raw(key), values),
      text: (key, values) => String(formatMessage(raw(key), values)),
      setLocale,
    };
  }, [current, setLocale]);

  return (
    <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>
  );
}

export function useLocale(): LocaleContextValue {
  const value = useContext(LocaleContext);
  if (!value) throw new Error("useLocale must be used inside LocaleProvider");
  return value;
}

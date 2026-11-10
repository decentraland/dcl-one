import { describe, expect, it } from "vitest";

import { LOCALES } from "./locales";
import en from "./catalogs/en.json";
import enOnly from "./catalogs/en-only.json";
import keys from "./catalogs/keys.json";
import { DCL_ONE_KEYS, loadLocaleMessages } from "./catalogs";

const catalogs = import.meta.glob<{ default: Record<string, string> }>(
  "./catalogs/*.json",
  { eager: true },
);

describe("locale catalogs", () => {
  it("give English text for every listed key", () => {
    expect(Object.keys(en).sort()).toEqual([...keys].sort());
  });

  it.each(LOCALES.filter((locale) => locale !== "en"))(
    "%s has every English key except its documented en-only ones",
    (locale) => {
      const catalog = catalogs[`./catalogs/${locale}.json`]!.default;
      const allowed = new Set(
        (enOnly as Record<string, string[]>)[locale] ?? [],
      );
      const missing = Object.keys(en).filter(
        (key) => !(key in catalog) && !allowed.has(key),
      );
      expect(missing).toEqual([]);
    },
  );

  it("keeps dcl-one's own keys apart from the imported source keys", () => {
    expect(
      Object.keys(DCL_ONE_KEYS).filter((key) => keys.includes(key)),
    ).toEqual([]);
  });

  it("gives every locale dcl-one's own keys in English", async () => {
    for (const locale of LOCALES) {
      const { messages } = await loadLocaleMessages(locale);
      for (const [key, english] of Object.entries(DCL_ONE_KEYS))
        expect(messages[key]).toBe(english);
    }
  });

  it("supplies English for a locale's en-only keys", async () => {
    for (const locale of LOCALES) {
      const { messages, fallback } = await loadLocaleMessages(locale);
      for (const key of (enOnly as Record<string, string[]>)[locale] ?? []) {
        expect(messages[key]).toBeUndefined();
        expect(fallback[key]).toBe((en as Record<string, string>)[key]);
      }
    }
  });
});

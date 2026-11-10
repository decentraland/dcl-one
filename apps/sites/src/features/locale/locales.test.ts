import { describe, expect, it } from "vitest";

import {
  localeFromAcceptLanguage,
  resolveRequestLocale,
  serializeLocaleCookie,
  toLocale,
} from "./locales";

const request = (headers: Record<string, string>) =>
  new Request("https://example.com/", { headers });

describe("toLocale", () => {
  it("maps regional and script tags to their supported base locale", () => {
    expect(toLocale("es-AR")).toBe("es");
    expect(toLocale("zh_Hant")).toBe("zh");
    expect(toLocale("JA")).toBe("ja");
  });

  it("rejects unsupported or empty values", () => {
    expect(toLocale("de")).toBeUndefined();
    expect(toLocale("")).toBeUndefined();
    expect(toLocale(undefined)).toBeUndefined();
  });
});

describe("localeFromAcceptLanguage", () => {
  it("picks the highest-quality supported language", () => {
    expect(localeFromAcceptLanguage("de-DE,de;q=0.9,fr;q=0.8,es;q=0.7")).toBe(
      "fr",
    );
    expect(localeFromAcceptLanguage("es;q=0.5, ko;q=0.9")).toBe("ko");
  });

  it("ignores languages refused with q=0", () => {
    expect(localeFromAcceptLanguage("fr;q=0, de")).toBeUndefined();
  });
});

describe("resolveRequestLocale", () => {
  it("prefers the persisted cookie over the browser languages", () => {
    expect(
      resolveRequestLocale(
        request({ cookie: "dcl-locale=ja", "accept-language": "fr" }),
      ),
    ).toBe("ja");
  });

  it("falls back to Accept-Language when the cookie is invalid or unsupported", () => {
    expect(
      resolveRequestLocale(
        request({ cookie: "dcl-locale=xx", "accept-language": "es-AR" }),
      ),
    ).toBe("es");
  });

  it("falls back to English with neither", () => {
    expect(resolveRequestLocale(request({ "accept-language": "de" }))).toBe(
      "en",
    );
  });

  it("renders the locale chosen before a reload", () => {
    const cookie = serializeLocaleCookie("fr", true).split(";")[0]!;
    expect(
      resolveRequestLocale(request({ cookie, "accept-language": "en" })),
    ).toBe("fr");
  });
});

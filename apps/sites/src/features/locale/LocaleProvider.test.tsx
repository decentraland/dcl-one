// @vitest-environment jsdom
import "./test-setup";
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { activeLocale } from "./document-locale";
import { LocaleProvider, useLocale } from "./LocaleProvider";
import type { LocaleMessages } from "./catalogs";
import type { Locale } from "./locales";

const KEY = "page.home.hero.title";
const messages = (
  locale: Locale,
  text = `title-${locale}`,
): LocaleMessages => ({
  locale,
  messages: { [KEY]: text },
  fallback: {},
});

let api: ReturnType<typeof useLocale>;
function Probe() {
  api = useLocale();
  return <p>{api.t(KEY)}</p>;
}

function clearCookie() {
  document.cookie = "dcl-locale=; Path=/; Max-Age=0";
}

beforeEach(() => {
  localStorage.clear();
  clearCookie();
  document.documentElement.lang = "en";
});

afterEach(() => {
  clearCookie();
});

describe("LocaleProvider", () => {
  it("reports a server-resolved locale as the document language without any switch", () => {
    // The server resolved Spanish from a cookie or Accept-Language; nothing is stored here.
    render(
      <LocaleProvider initial={messages("es")} load={vi.fn()}>
        <Probe />
      </LocaleProvider>,
    );
    expect(activeLocale()).toBe("es");
  });

  it("stops naming the document language once it unmounts", () => {
    const { unmount } = render(
      <LocaleProvider initial={messages("fr")} load={vi.fn()}>
        <Probe />
      </LocaleProvider>,
    );
    expect(activeLocale()).toBe("fr");
    unmount();
    expect(activeLocale()).toBeNull();
  });

  it("renders the server-resolved locale on first render", () => {
    render(
      <LocaleProvider initial={messages("es")} load={vi.fn()}>
        <Probe />
      </LocaleProvider>,
    );
    expect(screen.getByText("title-es")).toBeInTheDocument();
  });

  it("switches without a reload and persists the choice in the cookie, storage and the document language", async () => {
    const load = vi.fn(async (locale: Locale) => messages(locale));
    render(
      <LocaleProvider initial={messages("en")} load={load}>
        <Probe />
      </LocaleProvider>,
    );

    await act(() => api.setLocale("fr"));

    expect(screen.getByText("title-fr")).toBeInTheDocument();
    expect(document.cookie).toContain("dcl-locale=fr");
    expect(localStorage.getItem("dcl-locale")).toBe("fr");
    expect(activeLocale()).toBe("fr");
  });

  it("keeps only the latest request when an earlier one resolves later", async () => {
    let releaseFrench: (value: LocaleMessages) => void = () => undefined;
    const load = vi.fn((locale: Locale) =>
      locale === "fr"
        ? new Promise<LocaleMessages>((resolve) => (releaseFrench = resolve))
        : Promise.resolve(messages(locale)),
    );
    render(
      <LocaleProvider initial={messages("en")} load={load}>
        <Probe />
      </LocaleProvider>,
    );

    let french!: Promise<void>;
    act(() => {
      french = api.setLocale("fr");
    });
    await act(() => api.setLocale("ja"));
    await act(async () => {
      releaseFrench(messages("fr"));
      await french;
    });

    expect(screen.getByText("title-ja")).toBeInTheDocument();
    expect(localStorage.getItem("dcl-locale")).toBe("ja");
  });

  it("stays on the current language when a load fails, and a retry succeeds", async () => {
    const load = vi
      .fn<(locale: Locale) => Promise<LocaleMessages>>()
      .mockRejectedValueOnce(new Error("chunk failed"))
      .mockImplementation(async (locale) => messages(locale));
    render(
      <LocaleProvider initial={messages("en")} load={load}>
        <Probe />
      </LocaleProvider>,
    );

    await act(() =>
      expect(api.setLocale("ko")).rejects.toThrow("chunk failed"),
    );
    expect(screen.getByText("title-en")).toBeInTheDocument();

    await act(() => api.setLocale("ko"));
    expect(screen.getByText("title-ko")).toBeInTheDocument();
  });

  it("applies a choice stored before the cookie existed, then writes the cookie", async () => {
    localStorage.setItem("dcl-locale", "ja");
    const load = vi.fn(async (locale: Locale) => messages(locale));

    await act(async () => {
      render(
        <LocaleProvider initial={messages("en")} load={load}>
          <Probe />
        </LocaleProvider>,
      );
    });

    expect(load).toHaveBeenCalledWith("ja");
    expect(screen.getByText("title-ja")).toBeInTheDocument();
    expect(document.cookie).toContain("dcl-locale=ja");
  });

  it("ignores an unsupported stored value", async () => {
    localStorage.setItem("dcl-locale", "xx");
    const load = vi.fn(async (locale: Locale) => messages(locale));

    await act(async () => {
      render(
        <LocaleProvider initial={messages("es")} load={load}>
          <Probe />
        </LocaleProvider>,
      );
    });

    expect(load).not.toHaveBeenCalled();
    expect(screen.getByText("title-es")).toBeInTheDocument();
  });

  it("does not override the cookie the server already honored", async () => {
    document.cookie = "dcl-locale=fr; Path=/";
    localStorage.setItem("dcl-locale", "ja");
    const load = vi.fn(async (locale: Locale) => messages(locale));

    await act(async () => {
      render(
        <LocaleProvider initial={messages("fr")} load={load}>
          <Probe />
        </LocaleProvider>,
      );
    });

    expect(load).not.toHaveBeenCalled();
  });

  it("falls back to English for an en-only key", () => {
    render(
      <LocaleProvider
        initial={{
          locale: "es",
          messages: {},
          fallback: { [KEY]: "title-en-fallback" },
        }}
        load={vi.fn()}
      >
        <Probe />
      </LocaleProvider>,
    );
    expect(screen.getByText("title-en-fallback")).toBeInTheDocument();
  });
});

// @vitest-environment jsdom
import "@/features/locale/test-setup";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import SitesChrome from "@ui/web/frames/SitesChrome";

const runDownload = vi.fn();

vi.mock("@/features/download/success", async (actual) => ({
  ...(await actual<typeof import("@/features/download/success")>()),
  runDownload: (...args: unknown[]) => runDownload(...args),
}));
const track = vi.fn();
vi.mock("@/core/lib/telemetry/track", () => ({
  track: (...args: unknown[]) => track(...args),
}));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => null }));

import { useDownloadSuccess } from "@/features/download/useDownloadSuccess";
import type { LocaleMessages } from "@/features/locale/catalogs";
import { LocaleProvider, useLocale } from "@/features/locale/LocaleProvider";
import { resolveRequestLocale, type Locale } from "@/features/locale/locales";

import SitesPageChrome from "./sites-page-chrome";

const SESSION = { sid: "sid-1", story: "discover" };
const messages = (locale: Locale): LocaleMessages => ({
  locale,
  messages: {},
  fallback: {},
});
const load = vi.fn(async (locale: Locale) => messages(locale));

// What the server would render on a reload: the locale its cookie names.
const reloadLocale = () =>
  resolveRequestLocale(
    new Request("http://localhost/", { headers: { cookie: document.cookie } }),
  );

function LocaleProbe() {
  return <p>{`locale:${useLocale().locale}`}</p>;
}

function renderNewPage(locale: Locale, children = <LocaleProbe />) {
  return render(
    <LocaleProvider initial={messages(locale)} load={load}>
      <SitesPageChrome session={SESSION} header="none">
        {children}
      </SitesPageChrome>
    </LocaleProvider>,
  );
}

function renderLegacyPage() {
  return render(
    <SitesChrome hideNav>
      <p>legacy</p>
    </SitesChrome>,
  );
}

async function pickInFooter(name: RegExp | string) {
  const user = userEvent.setup();
  await user.click(
    screen.getByRole("button", {
      name: /^component\.landing\.footer\.language\.trigger|Change language/,
    }),
  );
  await user.click(screen.getByRole("option", { name }));
}

function clearCookies() {
  for (const name of ["lang", "dcl-locale"])
    document.cookie = `${name}=; path=/; max-age=0`;
}

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  clearCookies();
  load.mockClear();
  track.mockClear();
  runDownload.mockReset().mockResolvedValue({ status: "success" });
});

afterEach(() => clearCookies());

describe("SitesPageChrome", () => {
  describe("footer clicks, as source's footer reports them", () => {
    // jsdom does not navigate; keep it from trying.
    const stop = (event: Event) => event.preventDefault();
    beforeEach(() => document.addEventListener("click", stop));
    afterEach(() => document.removeEventListener("click", stop));

    it("sends a Landing Footer Link Click named by the link's shown label", async () => {
      renderNewPage("en");

      const footer = screen.getByRole("contentinfo");
      await userEvent.click(
        within(footer).getAllByRole("link", {
          name: "component.landing.footer.resources.blog",
        })[0]!,
      );

      expect(track).toHaveBeenCalledWith(
        "Click",
        {
          place: "Landing Footer Link",
          event: "click",
          link: "component.landing.footer.resources.blog",
        },
        SESSION,
      );
    });

    it("sends a Landing Footer Social Click named by the platform", async () => {
      renderNewPage("en");

      await userEvent.click(
        within(screen.getByRole("contentinfo")).getAllByRole("link", {
          name: "GitHub",
        })[0]!,
      );

      expect(track).toHaveBeenCalledWith(
        "Click",
        { place: "Landing Footer Social", event: "click", platform: "GitHub" },
        SESSION,
      );
    });

    it("sends nothing for the legal links, as source", async () => {
      renderNewPage("en");

      await userEvent.click(
        within(screen.getByRole("contentinfo")).getByRole("link", {
          name: "component.landing.footer.legal.privacy",
        }),
      );

      expect(track).not.toHaveBeenCalled();
    });
  });

  it("links help and support to the dcl-one docs page and the legal pages to their routes", () => {
    renderNewPage("en");

    const footer = screen.getByRole("contentinfo");
    const support = within(footer).getAllByRole("link", {
      name: "component.landing.footer.getting_started.contact_support",
    });
    expect(support[0]).toHaveAttribute(
      "href",
      expect.stringMatching(/\/docs\/player\/faqs\/contact-and-support\/$/),
    );
    expect(
      within(footer).getByRole("link", {
        name: "component.landing.footer.legal.privacy",
      }),
    ).toHaveAttribute("href", "/privacy");
  });

  describe("when the footer switcher picks a language", () => {
    it("persists it canonically and mirrors it to the legacy chrome when that has it", async () => {
      renderNewPage("en");
      await pickInFooter("Español");

      expect(localStorage.getItem("dcl-locale")).toBe("es");
      expect(document.cookie).toContain("dcl-locale=es");
      expect(localStorage.getItem("dcl:lang")).toBe("es");
      expect(document.cookie).toContain("lang=es");
    });

    it("leaves the legacy choice alone for a language the legacy chrome lacks", async () => {
      localStorage.setItem("dcl:lang", "zh");
      renderNewPage("en");
      await pickInFooter("Français");

      expect(localStorage.getItem("dcl-locale")).toBe("fr");
      expect(localStorage.getItem("dcl:lang")).toBe("zh");
    });
  });

  describe("when moving between localized and legacy pages", () => {
    it("new → legacy → new keeps a canonical French choice through the legacy fallback and a reload", async () => {
      const page = renderNewPage("en");
      await pickInFooter("Français");
      page.unmount();

      const legacy = renderLegacyPage();
      expect(
        screen.getByRole("button", { name: "Change language: English" }),
      ).toBeInTheDocument();
      expect(localStorage.getItem("dcl-locale")).toBe("fr");
      legacy.unmount();

      expect(reloadLocale()).toBe("fr");
      load.mockClear();
      renderNewPage(reloadLocale());
      await act(async () => undefined);
      expect(load).not.toHaveBeenCalled();
      expect(screen.getByText("locale:fr")).toBeInTheDocument();
    });

    it("an explicit legacy selection becomes the localized pages' language on reload and hydration", async () => {
      const legacy = renderLegacyPage();
      const user = userEvent.setup();
      await user.click(
        screen.getByRole("button", { name: "Change language: English" }),
      );
      await user.click(screen.getByRole("option", { name: /Espa/ }));
      legacy.unmount();

      expect(reloadLocale()).toBe("es");
      renderNewPage(reloadLocale());
      await act(async () => undefined);
      // The cookie already matches what the server rendered: no client switch after hydration.
      expect(load).not.toHaveBeenCalled();
      expect(screen.getByText("locale:es")).toBeInTheDocument();
    });

    it("a localized choice the legacy chrome has shows there", async () => {
      const page = renderNewPage("en");
      await pickInFooter("中文");
      page.unmount();

      renderLegacyPage();
      expect(
        screen.getByRole("button", { name: "Change language: 中文" }),
      ).toBeInTheDocument();
    });
  });

  // The legacy footer switcher (apps/ui3 SitesChrome) on the pages not migrated yet shares the language
  // storage with these pages; it has three languages of its own.
  describe("the legacy switcher's shared language storage", () => {
    it("lists its three languages and writes both its own and the localized pages' choice", async () => {
      renderLegacyPage();

      const user = userEvent.setup();
      await user.click(
        screen.getByRole("button", { name: "Change language: English" }),
      );
      expect(
        within(screen.getByRole("listbox", { name: "Language" })).getAllByRole(
          "option",
        ),
      ).toHaveLength(3);
      await user.click(screen.getByRole("option", { name: /Espa/ }));

      expect(localStorage.getItem("dcl:lang")).toBe("es");
      expect(localStorage.getItem("dcl-locale")).toBe("es");
      expect(document.cookie).toContain("lang=es");
      expect(document.cookie).toContain("dcl-locale=es");
    });

    it("reads the localized pages' cookie when storage has no choice", () => {
      document.cookie = "dcl-locale=es; path=/";
      renderLegacyPage();

      expect(
        screen.getByRole("button", { name: /Change language: Espa/ }),
      ).toBeInTheDocument();
    });

    it("shows English for a language it lacks, without overwriting that choice or the page language", () => {
      document.documentElement.lang = "fr";
      localStorage.setItem("dcl:lang", "es");
      localStorage.setItem("dcl-locale", "fr");
      document.cookie = "dcl-locale=fr; path=/";
      renderLegacyPage();

      expect(
        screen.getByRole("button", { name: "Change language: English" }),
      ).toBeInTheDocument();
      expect(localStorage.getItem("dcl-locale")).toBe("fr");
      expect(document.cookie).toContain("dcl-locale=fr");
      expect(document.documentElement.lang).toBe("fr");
    });
  });

  describe("on the download success page", () => {
    it("does not download again when the visitor switches language", async () => {
      function Success() {
        return (
          <p>
            {
              useDownloadSuccess(
                "?os=Windows&place=download-page",
                { sid: "sid-1", story: "download" },
                undefined,
              ).state.phase
            }
          </p>
        );
      }
      renderNewPage("en", <Success />);
      await screen.findByText("saved");
      expect(runDownload).toHaveBeenCalledTimes(1);

      await pickInFooter("Español");
      await waitFor(() =>
        expect(localStorage.getItem("dcl-locale")).toBe("es"),
      );
      await pickInFooter("日本語");
      await waitFor(() =>
        expect(localStorage.getItem("dcl-locale")).toBe("ja"),
      );

      expect(runDownload).toHaveBeenCalledTimes(1);
    });
  });
});

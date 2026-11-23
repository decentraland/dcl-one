// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createLegacyTrackScope, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

import { isSameTabNavigation, navbarLinkClick, type NavbarClick } from "./chrome-clicks";

// The navbar and footer Clicks through the real transport and gates. Each test loads a fresh document, whose
// analytics is ready once its first Sites page view leaves.

const BROWSER_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

function browser(userAgent = BROWSER_UA) {
  vi.stubGlobal("navigator", { userAgent, language: "en-US", maxTouchPoints: 0, sendBeacon });
}

async function loadDocument(path = "/") {
  window.history.replaceState(null, "", path);
  vi.stubGlobal("performance", { ...performance, getEntriesByType: () => [] });
  vi.resetModules();
  const telemetry = await import("@/core/lib/telemetry/track");
  const chrome = await import("./chrome-clicks");
  const user = await import("@/core/lib/telemetry/tracked-user");
  const legacy = await import("@/core/lib/telemetry/legacy-click");
  // The navbar's own scope, as `useChromeClicks` holds it for the mounted chrome.
  SESSION.scope = legacy.createLegacyTrackScope();
  return { page: telemetry.page, dropTrackedUser: user.dropTrackedUser, ...chrome };
}

type Sent = Record<string, unknown> & { properties: Record<string, unknown> };

async function clicks(): Promise<Sent[]> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.filter((b) => b.type === "track" && b.event === "Click");
}

const SESSION: LegacySession = { sid: "sid-1", scope: createLegacyTrackScope() };
const EVENTS: NavbarClick = { action: "events", link: { section: "discover", href: "/events" } };

beforeEach(() => {
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  browser();
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  sendBeacon.mockClear();
  restoreTrackedUser();
  vi.resetModules();
  window.history.replaceState(null, "", "/");
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
});

describe("trackNavbarClick", () => {
  it("sends the legacy navbar Click once analytics is ready, with no utm_* and not deferred", async () => {
    const { page, trackNavbarClick } = await loadDocument("/events?utm_source=newsletter");
    page("/events", {}, { sid: "sid-1" });
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackNavbarClick(EVENTS, { ...SESSION, userId: "0xAbC" });

    const [click] = await clicks();
    expect(click).toMatchObject({ anonymousId: "sid-1", userId: "0xabc" });
    expect(click!.properties).toEqual({
      place: "Landing Navbar",
      event: "click",
      action: "events",
      section: "discover",
      href: "/events",
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: false,
    });
  });

  it("sends a click without a link (Sign in) with no section or href", async () => {
    const { page, trackNavbarClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackNavbarClick({ action: "sign_in" }, SESSION);

    const [click] = await clicks();
    expect(click!.properties).toMatchObject({ place: "Landing Navbar", event: "click", action: "sign_in" });
    expect(click!.properties).not.toHaveProperty("section");
    expect(click!.properties).not.toHaveProperty("href");
  });

  it("sends a click that replaces the page before analytics is ready at once, deferred", async () => {
    const { trackNavbarClick } = await loadDocument("/");
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackNavbarClick(EVENTS, SESSION);

    const [click] = await clicks();
    expect(click!.properties).toMatchObject({
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_000,
      track_deferred: true,
    });
  });

  it("holds a click that keeps the page until analytics is ready, then sends it deferred", async () => {
    const { page, trackNavbarClick } = await loadDocument("/");
    const now = vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);
    trackNavbarClick(EVENTS, SESSION, { leavesPage: false });
    expect(await clicks()).toEqual([]);

    now.mockReturnValue(1_700_000_000_400);
    page("/", {}, { sid: "sid-1" });
    const sent = await clicks();
    expect(sent).toHaveLength(1);
    expect(sent[0]!.properties).toMatchObject({
      track_called_at: 1_700_000_000_000,
      track_delivered_at: 1_700_000_000_400,
      track_deferred: true,
    });
  });

  it("drops a held click when the navbar unmounts before analytics is ready", async () => {
    const { page, trackNavbarClick } = await loadDocument("/");
    trackNavbarClick(EVENTS, SESSION, { leavesPage: false });
    SESSION.scope.close();
    page("/", {}, { sid: "sid-1" });
    expect(await clicks()).toEqual([]);
  });

  it("sends no userId once a sign-out has started", async () => {
    const { page, trackNavbarClick, dropTrackedUser } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    dropTrackedUser();
    trackNavbarClick(EVENTS, { ...SESSION, userId: "0xabc" });
    expect((await clicks())[0]).not.toHaveProperty("userId");
  });

  it("sends nothing for a bot, whether the click leaves the page or not", async () => {
    browser(BOT_UA);
    const { page, trackNavbarClick } = await loadDocument("/");
    trackNavbarClick(EVENTS, SESSION);
    trackNavbarClick(EVENTS, SESSION, { leavesPage: false });
    page("/", {}, { sid: "sid-1" });
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing in a session that started on an exempt page", async () => {
    const { page, trackNavbarClick } = await loadDocument("/privacy");
    page("/privacy", {}, { sid: "sid-1" });
    trackNavbarClick(EVENTS, SESSION);
    trackNavbarClick(EVENTS, SESSION, { leavesPage: false });
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing without a write key", async () => {
    delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
    const { trackNavbarClick } = await loadDocument("/");
    trackNavbarClick(EVENTS, SESSION);
    expect(sendBeacon).not.toHaveBeenCalled();
  });
});

describe("footer clicks", () => {
  it("sends the social icon Click with the platform and no deferral fields once analytics is ready", async () => {
    const { page, trackFooterSocialClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackFooterSocialClick("Discord", { ...SESSION, userId: "0xAbC" });

    const [click] = await clicks();
    expect(click).toMatchObject({ anonymousId: "sid-1", userId: "0xabc" });
    expect(click!.properties).toEqual({ place: "Landing Footer Social", event: "click", platform: "Discord" });
  });

  it("sends the link Click with the label it shows", async () => {
    const { page, trackFooterLinkClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackFooterLinkClick("Preguntas frecuentes", SESSION);

    const [click] = await clicks();
    expect(click!.properties).toEqual({ place: "Landing Footer Link", event: "click", link: "Preguntas frecuentes" });
  });

  it("drops a click made before analytics is ready, and does not send it later", async () => {
    const { page, trackFooterLinkClick, trackFooterSocialClick } = await loadDocument("/");
    trackFooterSocialClick("X", SESSION);
    trackFooterLinkClick("FAQs", SESSION);
    page("/", {}, { sid: "sid-1" });
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing for a bot", async () => {
    browser(BOT_UA);
    const { page, trackFooterSocialClick } = await loadDocument("/");
    page("/", {}, { sid: "sid-1" });
    trackFooterSocialClick("X", SESSION);
    expect(await clicks()).toEqual([]);
  });
});

describe("navbarLinkClick", () => {
  const signedOut = { signedIn: false, signInHref: "/shop/my-items" };
  const signedIn = { signedIn: true, profileHref: "/profile/0xabc", signInHref: "/shop/my-items" };

  it.each([
    ["/", "logo", "home", "/"],
    ["/events", "events", "discover", "/events"],
    ["/places", "places", "discover", "/places"],
    ["/shop/overview", "shop_all", "shop", "https://decentraland.org/shop"],
    ["/shop/items?category=wearable", "wearables", "shop", "https://decentraland.org/shop/items?category=wearable"],
    ["/shop/items?category=emote", "emotes", "shop", "https://decentraland.org/shop/items?category=emote"],
    ["/shop/names", "names", "shop", "https://decentraland.org/shop/items?category=names"],
    ["/shop/lands", "land", "shop", "https://decentraland.org/marketplace/lands"],
    ["https://store.decentraland.org/", "merch", "shop", "https://store.decentraland.org/"],
    ["/create", "create_in_decentraland", "create", "https://decentraland.org/create/"],
    ["/create/collections", "publish_wearables_emotes", "create", "https://decentraland.org/create/collections"],
    ["/download/creator-hub", "publish_land", "create", "https://decentraland.org/builder/land"],
    ["/blog", "learn", "learn", "https://decentraland.org/blog/"],
    ["/shop/credits", "credits", "credits", "/shop/credits"],
  ])("maps %s to the legacy %s entry", (href, action, section, legacyHref) => {
    expect(navbarLinkClick(href, signedOut)).toEqual({ action, link: { section, href: legacyHref } });
  });

  it.each([
    ["/profile/0xabc", "view_profile", "https://decentraland.org/profile"],
    ["/shop/my-items", "my_assets", "https://decentraland.org/marketplace/account"],
    ["/account", "account_settings", "https://decentraland.org/account"],
    ["/shop/settings", "marketplace_authorizations", "https://decentraland.org/marketplace/settings"],
  ])("maps the account menu's %s to the legacy %s entry", (href, action, legacyHref) => {
    expect(navbarLinkClick(href, signedIn)).toEqual({ action, link: { section: "user_menu", href: legacyHref } });
  });

  it("reads Sign in as a link as the legacy sign_in, with no link", () => {
    expect(navbarLinkClick("/shop/my-items", signedOut)).toEqual({ action: "sign_in" });
  });

  it.each(["/account", "/shop/settings", "/profile/0xabc"])("does not read %s as the account menu when signed out", (href) => {
    expect(navbarLinkClick(href, { signedIn: false })).toBeNull();
  });

  it.each(["/docs/player/", "/docs/creator/", "/profile/0xdef", "/blog/post", "", "toString", "__proto__"])(
    "has no legacy entry for %j",
    (href) => {
      expect(navbarLinkClick(href, signedIn)).toBeNull();
    },
  );
});

describe("isSameTabNavigation", () => {
  function anchor(attributes: Record<string, string> = {}) {
    const element = document.createElement("a");
    for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
    return element;
  }
  const keys = { ctrlKey: false, metaKey: false, shiftKey: false, altKey: false };

  it.each<Record<string, string>>([{}, { target: "_self" }, { target: "_TOP" }, { target: "" }])("is true for a same-tab link %j", (attributes) => {
    expect(isSameTabNavigation({ currentTarget: anchor(attributes), ...keys })).toBe(true);
  });

  it.each<Record<string, string>>([{ target: "_blank" }, { target: "named" }, { download: "" }])("is false for a link %j", (attributes) => {
    expect(isSameTabNavigation({ currentTarget: anchor(attributes), ...keys })).toBe(false);
  });

  it.each(["ctrlKey", "metaKey", "shiftKey", "altKey"] as const)("is false with %s held", (key) => {
    expect(isSameTabNavigation({ currentTarget: anchor(), ...keys, [key]: true })).toBe(false);
  });
});

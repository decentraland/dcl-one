// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { sitesFooterCopy } from "@dcl/one-ui/apps/sites/chrome/footer-copy";
import { ChromeAuthContext, type ChromeAuth } from "@dcl/one-ui/components/AppChrome/chrome-auth";
import { createShopClient } from "@shop/api/shop";
import { RootDataRouter } from "@/core/lib/telemetry/root-router.test-utils";
import { sitesShopClient } from "@/data/lib/auth/shop-client";

import ChromeNavBridge from "./ChromeNavBridge";
import SitesLayout from "./SitesLayout";

// The navbar and footer as a visitor clicks them, through the real Segment transport. The document's analytics
// readiness (its first page view, covered in chrome-clicks.test.ts) is driven here.

const { readiness } = vi.hoisted(() => ({ readiness: { ready: true, pending: [] as Array<() => void> } }));
vi.mock("@/core/lib/telemetry/track", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/core/lib/telemetry/track")>()),
  legacyAnalyticsReady: () => readiness.ready,
  whenLegacyAnalyticsReady: (fire: () => void) => (readiness.ready ? fire() : readiness.pending.push(fire)),
}));
vi.mock("@/data/lib/auth/shop-client", () => ({ sitesShopClient: vi.fn() }));
vi.mock("@/data/lib/shop/marketplace-access", () => ({ useMarketplaceHere: () => true }));

vi.hoisted(() => Object.assign(window, {
  __vite_plugin_react_preamble_installed__: true,
  $RefreshSig$: () => (type: unknown) => type,
  $RefreshReg$: () => {},
}));

function becomeReady() {
  readiness.ready = true;
  const queued = readiness.pending;
  readiness.pending = [];
  for (const fire of queued) fire();
}

const BROWSER_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const BOT_UA = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
const sendBeacon = vi.fn((_url: string, _body: Blob) => true);

type Sent = Record<string, unknown> & { properties: Record<string, unknown> };

async function clicks(): Promise<Sent[]> {
  const bodies = await Promise.all(sendBeacon.mock.calls.map(async ([, blob]) => JSON.parse(await blob.text())));
  return bodies.filter((b) => b.type === "track" && b.event === "Click");
}

async function onlyClick(): Promise<Sent> {
  const sent = await clicks();
  expect(sent).toHaveLength(1);
  return sent[0]!;
}

const account = "0x000000000000000000000000000000000000abcd";
const signedOut: ChromeAuth = { signedIn: false, account: "", mana: "", name: "", committee: false };

function show({
  path = "/events",
  auth = signedOut,
  analyticsUserId = null as string | null,
  footerCopy = sitesFooterCopy.en,
} = {}) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <ChromeAuthContext value={auth}>
        <RootDataRouter initialEntries={[path]} root={{ sid: "sid-1", analyticsUserId }}>
          <ChromeNavBridge>
            <SitesLayout footerCopy={footerCopy}>Page</SitesLayout>
          </ChromeNavBridge>
        </RootDataRouter>
      </ChromeAuthContext>
    </QueryClientProvider>,
  );
}

const header = () => within(screen.getByRole("banner", { name: "Decentraland" }));
const footer = () => within(screen.getByRole("contentinfo"));

let user: ReturnType<typeof userEvent.setup>;
const transport = vi.fn();

beforeEach(() => {
  user = userEvent.setup();
  readiness.ready = true;
  readiness.pending = [];
  (window as unknown as { __DCL_PUBLIC__: Record<string, string> }).__DCL_PUBLIC__ = { SEGMENT_WRITE_KEY: "key-test" };
  vi.stubGlobal("navigator", { ...navigator, userAgent: BROWSER_UA, language: "en-US", sendBeacon });
  transport.mockReset();
  transport.mockResolvedValue({ status: 200, body: JSON.stringify({ usd: { balanceCents: 500 } }) });
  vi.mocked(sitesShopClient).mockReturnValue(createShopClient({
    base: "https://sites.test",
    mode: "canonical",
    signer: { address: account, sign: async () => ({}) },
    transport,
  }));
  window.matchMedia = vi.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  sendBeacon.mockClear();
  delete (window as unknown as { __DCL_PUBLIC__?: unknown }).__DCL_PUBLIC__;
});

describe("the navbar", () => {
  it("sends the logo click as the legacy navbar Click", async () => {
    show({ analyticsUserId: "0xAbC" });
    await user.click(header().getByRole("link", { name: "Decentraland home" }));

    const click = await onlyClick();
    expect(click).toMatchObject({ anonymousId: "sid-1", userId: "0xabc" });
    expect(click.properties).toMatchObject({
      place: "Landing Navbar",
      event: "click",
      action: "logo",
      section: "home",
      href: "/",
      track_deferred: false,
    });
  });

  it("sends a dropdown item with its legacy action, section and href", async () => {
    show();
    await user.click(header().getByRole("button", { name: "Shop" }));
    await user.click(header().getByRole("link", { name: "Emotes" }));

    expect((await onlyClick()).properties).toMatchObject({
      action: "emotes",
      section: "shop",
      href: "https://decentraland.org/shop/items?category=emote",
    });
  });

  it("holds a new-tab link clicked before analytics is ready until it is", async () => {
    readiness.ready = false;
    show();
    await user.click(header().getByRole("button", { name: "Shop" }));
    const merch = header().getByRole("link", { name: "Merch" });
    merch.addEventListener("click", (event) => event.preventDefault());
    await user.click(merch);
    expect(await clicks()).toEqual([]);

    becomeReady();
    expect((await onlyClick()).properties).toMatchObject({ action: "merch", section: "shop", track_deferred: true });
  });

  it("sends a same-tab link clicked before analytics is ready at once", async () => {
    readiness.ready = false;
    show();
    await user.click(header().getByRole("button", { name: "Discover" }));
    await user.click(header().getByRole("link", { name: "Places" }));

    expect((await onlyClick()).properties).toMatchObject({ action: "places", section: "discover", track_deferred: true });
  });

  it("sends a middle click once, held like a new tab", async () => {
    readiness.ready = false;
    show();
    await user.click(header().getByRole("button", { name: "Discover" }));
    fireEvent(header().getByRole("link", { name: "Events" }), new MouseEvent("auxclick", { bubbles: true, button: 1 }));
    expect(await clicks()).toEqual([]);

    becomeReady();
    expect((await onlyClick()).properties).toMatchObject({ action: "events", track_deferred: true });
  });

  it("sends nothing for a right click", async () => {
    show();
    fireEvent(header().getByRole("link", { name: "Decentraland home" }), new MouseEvent("auxclick", { bubbles: true, button: 2 }));
    expect(await clicks()).toEqual([]);
  });

  it("sends Sign in with no section or href, and still signs in", async () => {
    const onSignIn = vi.fn();
    show({ auth: { ...signedOut, onSignIn } });
    await user.click(header().getByRole("button", { name: "Sign in" }));

    const { properties } = await onlyClick();
    expect(properties).toMatchObject({ place: "Landing Navbar", event: "click", action: "sign_in" });
    expect(properties).not.toHaveProperty("section");
    expect(properties).not.toHaveProperty("href");
    expect(onSignIn).toHaveBeenCalledOnce();
  });

  it("sends a link with no legacy entry as nothing", async () => {
    show();
    await user.click(header().getByRole("button", { name: "Open menu" }));
    const drawer = within(screen.getByRole("dialog"));
    await user.click(drawer.getByRole("button", { name: "Learn" }));
    const getStarted = drawer.getByRole("link", { name: "Get Started" });
    getStarted.addEventListener("click", (event) => event.preventDefault());
    await user.click(getStarted);
    expect(await clicks()).toEqual([]);
  });

  it("sends the account menu's links with the legacy user_menu entries", async () => {
    show({ auth: { ...signedOut, signedIn: true, account, name: "Ada" } });
    await user.click(header().getByRole("button", { name: "Account: Ada" }));
    await user.click(screen.getByRole("link", { name: "View Profile" }));

    expect((await onlyClick()).properties).toMatchObject({
      action: "view_profile",
      section: "user_menu",
      href: "https://decentraland.org/profile",
    });
  });

  it("sends the credits chip as the legacy credits entry", async () => {
    show({ auth: { ...signedOut, signedIn: true, account, name: "Ada" } });
    await user.click(await header().findByRole("link", { name: "50 credits" }));

    expect((await onlyClick()).properties).toMatchObject({ action: "credits", section: "credits", href: "/shop/credits" });
  });

  it("sends nothing on a page the legacy site never served", async () => {
    show({ path: "/shop/overview" });
    await user.click(header().getByRole("link", { name: "Decentraland home" }));
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing for a bot", async () => {
    vi.stubGlobal("navigator", { ...navigator, userAgent: BOT_UA, sendBeacon });
    show();
    await user.click(header().getByRole("link", { name: "Decentraland home" }));
    expect(await clicks()).toEqual([]);
  });
});

describe("the footer", () => {
  it("sends a social icon as the legacy footer social Click", async () => {
    show({ analyticsUserId: "0xabc" });
    await user.click(footer().getAllByRole("link", { name: "Discord" })[0]!);

    const click = await onlyClick();
    expect(click).toMatchObject({ anonymousId: "sid-1", userId: "0xabc" });
    expect(click.properties).toEqual({ place: "Landing Footer Social", event: "click", platform: "Discord" });
  });

  it("sends a link with the label shown in the visitor's language", async () => {
    show({ footerCopy: sitesFooterCopy.es });
    const link = footer().getAllByRole("link", { name: sitesFooterCopy.es.getting_started.contact_support })[0]!;
    await user.click(link);

    expect((await onlyClick()).properties).toEqual({
      place: "Landing Footer Link",
      event: "click",
      link: sitesFooterCopy.es.getting_started.contact_support,
    });
  });

  it("drops a click made before analytics is ready", async () => {
    readiness.ready = false;
    show();
    await user.click(footer().getAllByRole("link", { name: "GitHub" })[0]!);
    becomeReady();
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing on a page the legacy site never served", async () => {
    show({ path: "/docs" });
    await user.click(footer().getAllByRole("link", { name: "GitHub" })[0]!);
    expect(await clicks()).toEqual([]);
  });

  it("sends nothing for the legal links, as the legacy footer did", async () => {
    show();
    const privacy = footer().getByRole("link", { name: "Privacy Policy" });
    privacy.addEventListener("click", (event) => event.preventDefault());
    await user.click(privacy);
    expect(await clicks()).toEqual([]);
  });
});

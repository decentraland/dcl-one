import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { CreatorHubHost } from "@ui/creatorhub/explorer-host";
import type { AuthIdentity } from "@/data/lib/auth/types";

const explorer: AuthIdentity = {
  signer: `0x${"b".repeat(40)}`,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"1".repeat(64)}` },
  expiration: new Date(Date.now() + 86400000).toISOString(),
  authChain: [],
};
const website: AuthIdentity = { ...explorer, signer: `0x${"a".repeat(40)}` };
const THIRDWEB = "dcl:auth:thirdweb:v1";

let storage: Map<string, string>;

function framedBy(host: CreatorHubHost | undefined) {
  const parent = { __dclCreatorHubHost: host, postMessage: vi.fn() };
  return Object.assign(new EventTarget(), {
    parent,
    document,
    location: { href: "https://node.example/create/publish", origin: "https://node.example", hostname: "node.example", protocol: "https:" },
    open: vi.fn(),
    localStorage: {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
    },
  });
}

function fakeHost(identity: AuthIdentity | null) {
  const listeners = new Set<(identity: AuthIdentity | null) => void>();
  const provider = { request: vi.fn() };
  const host = {
    version: 1 as const,
    identity: vi.fn(() => identity),
    subscribe: vi.fn((listener: (identity: AuthIdentity | null) => void) => {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    }),
    signIn: vi.fn(),
    walletProvider: vi.fn(() => provider),
    openLink: vi.fn(),
  };
  return { host, listeners, provider };
}

const walletCookie = () => /^dcl_wallet=([^;]*)/.exec(document.cookie)?.[1] || null;

beforeEach(() => {
  vi.resetModules();
  storage = new Map([
    ["dcl:auth:identity:v1", JSON.stringify(website)],
    [THIRDWEB, JSON.stringify({ token: "website-token", address: website.signer })],
  ]);
  vi.stubGlobal("document", Object.assign(new EventTarget(), { cookie: "", visibilityState: "visible" }));
});
afterEach(() => vi.unstubAllGlobals());

it("signs a framed hub as the Explorer account, with the Explorer's sign-in and wallet", async () => {
  const { host, provider } = fakeHost(explorer);
  const win = framedBy(host);
  vi.stubGlobal("window", win);
  const { adoptExplorerHost, explorerHost } = await import("./explorer-host");
  const session = await import("@/data/lib/auth/session");
  const signin = await import("../auth/signin-store");
  const wallet = await import("@/data/lib/auth/wallet");

  expect(adoptExplorerHost(win as unknown as Window)).toBe(host);
  expect(explorerHost()).toBe(host);
  // A copy in this document's realm, not the website's own wallet.
  expect(session.getIdentity()).toEqual(explorer);
  expect(session.getIdentity()).not.toBe(explorer);
  signin.openSignIn();
  expect(host.signIn).toHaveBeenCalledOnce();
  expect(wallet.walletProvider()).toBe(provider);
});

it("follows the Explorer's account switches until the document unloads", async () => {
  const { host, listeners } = fakeHost(null);
  const win = framedBy(host);
  vi.stubGlobal("window", win);
  const { adoptExplorerHost, followExplorerIdentity } = await import("./explorer-host");
  const session = await import("@/data/lib/auth/session");

  adoptExplorerHost(win as unknown as Window);
  expect(session.getIdentity()).toBeNull();
  const reload = vi.fn();
  followExplorerIdentity(win as unknown as Window, reload);
  for (const listener of listeners) listener(explorer);
  expect(session.getIdentity()?.signer).toBe(explorer.signer);
  expect(reload).toHaveBeenCalledOnce();
  win.dispatchEvent(new Event("pagehide"));
  expect(listeners.size).toBe(0);
});

it("leaves an unframed page, or one framed by anything else, on the website session", async () => {
  for (const win of [framedBy(undefined), Object.assign(new EventTarget(), { localStorage: framedBy(undefined).localStorage })]) {
    vi.resetModules();
    if (!("parent" in win)) Object.assign(win, { parent: win });
    vi.stubGlobal("window", win);
    const { adoptExplorerHost, explorerHost } = await import("./explorer-host");
    const session = await import("@/data/lib/auth/session");
    expect(adoptExplorerHost(win as unknown as Window)).toBeNull();
    expect(explorerHost()).toBeNull();
    expect(session.getIdentity()?.signer).toBe(website.signer);
  }
});

it("points the wallet cookie back at the website's own session on an unframed page", async () => {
  document.cookie = `dcl_wallet=${explorer.signer}; Path=/; Max-Age=86400`;
  vi.stubGlobal("window", framedBy(undefined));
  const session = await import("@/data/lib/auth/session");
  session.resyncWalletCookie();
  expect(walletCookie()).toBe(website.signer);
  expect(document.cookie).toMatch(/Max-Age=86\d{3}/);

  vi.resetModules();
  storage.delete("dcl:auth:identity:v1");
  const signedOut = await import("@/data/lib/auth/session");
  signedOut.resyncWalletCookie();
  expect(document.cookie).toMatch(/^dcl_wallet=; Path=\/; Max-Age=0/);
});

it("says whether the wallet cookie named another wallet, only once it points at this session", async () => {
  vi.stubGlobal("window", framedBy(undefined));
  const session = await import("@/data/lib/auth/session");
  expect(session.resyncWalletCookie()).toBe(false);
  expect(session.resyncWalletCookie()).toBe(false);
  document.cookie = `dcl_wallet=${explorer.signer}; Path=/`;
  expect(session.resyncWalletCookie()).toBe(true);
  expect(walletCookie()).toBe(website.signer);

  // Cookies the browser refuses: no write takes effect, so nothing asks for a reload.
  let jar = `dcl_wallet=${explorer.signer}`;
  Object.defineProperty(document, "cookie", { get: () => jar, set: () => {}, configurable: true });
  expect(session.resyncWalletCookie()).toBe(false);
  jar = "";
  expect(session.resyncWalletCookie()).toBe(false);
});

it("takes the wallet cookie back for a website tab whenever it is used after the Explorer pointed it elsewhere", async () => {
  const win = framedBy(undefined);
  vi.stubGlobal("window", win);
  const session = await import("@/data/lib/auth/session");
  const { scopeWalletCookie } = await import("@ui/creatorhub/explorer-host");
  session.resyncWalletCookie();
  const off = session.claimWalletCookieOnUse(win as unknown as Window);
  const explorerTab = { cookie: "", location: { protocol: "https:" } };
  Object.defineProperty(explorerTab, "cookie", { get: () => document.cookie, set: (value: string) => { document.cookie = value; } });

  for (const use of [
    () => win.dispatchEvent(new Event("focus")),
    () => win.dispatchEvent(new Event("pointerdown")),
    () => win.dispatchEvent(new Event("pageshow")),
    () => document.dispatchEvent(new Event("visibilitychange")),
  ]) {
    // The Explorer's Create panel opens on another account, then the website tab is used again.
    scopeWalletCookie(explorerTab as unknown as Document, explorer);
    expect(walletCookie()).toBe(explorer.signer);
    use();
    expect(walletCookie()).toBe(website.signer);
  }
  // A guest Explorer clears it; a hidden tab leaves it to the tab in use.
  scopeWalletCookie(explorerTab as unknown as Document, null);
  Object.assign(document, { visibilityState: "hidden" });
  document.dispatchEvent(new Event("visibilitychange"));
  expect(walletCookie()).toBeNull();

  off();
  scopeWalletCookie(explorerTab as unknown as Document, explorer);
  win.dispatchEvent(new Event("focus"));
  expect(walletCookie()).toBe(explorer.signer);
});

it("keeps Escape inside the framed hub and takes the wallet cookie back before the hub is used", async () => {
  const { host } = fakeHost(explorer);
  const win = framedBy(host);
  vi.stubGlobal("window", win);
  const { adoptExplorerHost, installHubFrame } = await import("./explorer-host");
  adoptExplorerHost(win as unknown as Window);
  const off = installHubFrame(win as unknown as Window, host, vi.fn());

  // The hub's dialogs, menus and editors close on Escape without consuming it: it must not close the panel.
  win.dispatchEvent(Object.assign(new Event("keydown", { cancelable: true }), { key: "Escape" }));
  expect(win.parent.postMessage).not.toHaveBeenCalled();

  // A website tab loading meanwhile pointed the cookie at the website account.
  document.cookie = `dcl_wallet=${website.signer}; Path=/`;
  win.dispatchEvent(new Event("pointerdown"));
  expect(walletCookie()).toBe(explorer.signer);
  document.cookie = `dcl_wallet=${website.signer}; Path=/`;
  win.dispatchEvent(new Event("focus"));
  expect(walletCookie()).toBe(explorer.signer);

  off();
  document.cookie = `dcl_wallet=${website.signer}; Path=/`;
  win.dispatchEvent(new Event("pointerdown"));
  expect(walletCookie()).toBe(website.signer);
});

it("sends a location that leaves /create to the Explorer once and steps back to the last hub page", async () => {
  const { host } = fakeHost(explorer);
  const { hubLocationKeeper } = await import("./explorer-host");
  const keep = hubLocationKeeper(host);
  const base = "https://node.example/create/scenes";
  const at = (path: string, key: string) => {
    const url = new URL(path, base);
    return { pathname: url.pathname, search: url.search, hash: url.hash, key };
  };

  expect(keep(at("/create/scenes?sort=recent", "a"), base)).toBeNull();
  expect(keep(at("/shop/names", "b"), base)).toBe("/create/scenes?sort=recent");
  expect(host.openLink).toHaveBeenCalledWith("https://node.example/shop/names");
  // The same location rendering again is not a second request.
  expect(keep(at("/shop/names", "b"), base)).toBeNull();
  expect(host.openLink).toHaveBeenCalledOnce();

  const fresh = hubLocationKeeper(host);
  expect(fresh(at("/play/?position=3,4", "c"), base)).toBe("/create");
  expect(host.openLink).toHaveBeenLastCalledWith("https://node.example/play/?position=3,4");
});

it("hands the hub's own new-tab links, like Jump in after publishing, to the Explorer", async () => {
  const { host } = fakeHost(explorer);
  const win = framedBy(host);
  vi.stubGlobal("window", win);
  const { adoptExplorerHost, openHubLink } = await import("./explorer-host");
  openHubLink("/play/?realm=boedo.dcl.eth");
  expect(win.open).toHaveBeenCalledWith("/play/?realm=boedo.dcl.eth", "_blank", "noopener,noreferrer");
  adoptExplorerHost(win as unknown as Window);
  openHubLink("/play/?realm=boedo.dcl.eth");
  expect(host.openLink).toHaveBeenCalledWith("https://node.example/play/?realm=boedo.dcl.eth");
  expect(win.open).toHaveBeenCalledOnce();
});

it("leaves sign-out to the Explorer and switches accounts through its sign-in", async () => {
  const fetch = vi.fn(() => Promise.resolve(new Response(null)));
  vi.stubGlobal("fetch", fetch);
  const { host } = fakeHost(explorer);
  const win = framedBy(host);
  vi.stubGlobal("window", win);
  const { adoptExplorerHost, hubAccountActions } = await import("./explorer-host");
  const { useAuth } = await import("@/data/lib/auth/context");
  const signOut = vi.fn();
  expect(hubAccountActions(signOut)).toEqual({ onSignOut: signOut });

  adoptExplorerHost(win as unknown as Window);
  const actions = hubAccountActions(signOut);
  expect(actions.onSignOut).toBeUndefined();
  actions.onSwitchAccount?.();
  expect(host.signIn).toHaveBeenCalledOnce();

  // Anything that still disconnects (the Shop's switch account) leaves the website's session alone.
  let disconnect = () => {};
  renderToString(createElement(() => {
    disconnect = useAuth().disconnect;
    return null;
  }));
  expect(() => disconnect()).not.toThrow();
  expect(storage.has(THIRDWEB)).toBe(true);
  expect(storage.has("dcl:auth:identity:v1")).toBe(true);
  expect(fetch).not.toHaveBeenCalled();
});

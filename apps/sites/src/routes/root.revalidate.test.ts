import { createMemoryRouter, type ShouldRevalidateFunctionArgs } from "react-router";
import { expect, it, vi } from "vitest";

import type { CreatorHubHost } from "@web-core/creator-hub-host";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { adoptExplorerHost, followExplorerIdentity, reloadingForAccount } from "@/features/components/chrome/explorer-host";

import { shouldRevalidate } from "./root";

const next: AuthIdentity = {
  signer: `0x${"a".repeat(40)}`,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"1".repeat(64)}` },
  expiration: new Date(Date.now() + 86400000).toISOString(),
  authChain: [],
};

/** A navigation from one path to another, with the answer React Router would give by default. */
function args(from: string, to: string, defaultShouldRevalidate: boolean, formMethod?: ShouldRevalidateFunctionArgs["formMethod"]): ShouldRevalidateFunctionArgs {
  return { currentUrl: new URL(`https://sites.test${from}`), currentParams: {}, nextUrl: new URL(`https://sites.test${to}`), nextParams: {}, formMethod, defaultShouldRevalidate };
}

it("reloads root's data too (the wallet and its committee seat) when the Explorer switches accounts", async () => {
  const listeners = new Set<(identity: AuthIdentity | null) => void>();
  const host: CreatorHubHost = {
    version: 1,
    identity: () => null,
    subscribe(listener) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    signIn: () => {},
    walletProvider: () => null,
    openLink: () => {},
  };
  const win = Object.assign(new EventTarget(), { parent: { __dclCreatorHubHost: host } }) as unknown as Window;
  expect(adoptExplorerHost(win)).toBe(host);

  const rootLoader = vi.fn(() => null);
  const pageLoader = vi.fn(() => null);
  const router = createMemoryRouter(
    [{ id: "root", path: "/", loader: rootLoader, shouldRevalidate, children: [{ path: "create/preview", loader: pageLoader }] }],
    { initialEntries: ["/create/preview"] },
  );
  await vi.waitFor(() => expect(router.state.initialized).toBe(true));
  // Another visit to the same page reloads only the page's own data.
  await router.navigate("/create/preview", { replace: true });
  expect(pageLoader).toHaveBeenCalledTimes(2);
  expect(rootLoader).toHaveBeenCalledOnce();

  followExplorerIdentity(win, () => router.navigate("/create/preview", { replace: true }));
  for (const listener of listeners) listener(next);
  expect(reloadingForAccount()).toBe(true);
  await vi.waitFor(() => expect(reloadingForAccount()).toBe(false));
  expect(pageLoader).toHaveBeenCalledTimes(3);
  expect(rootLoader).toHaveBeenCalledTimes(2);

  await router.navigate("/create/preview", { replace: true });
  expect(rootLoader).toHaveBeenCalledTimes(2);
  router.dispose();
});

it("reloads root's data when a visitor enters the Creator Hub, which alone reads the committee seat", () => {
  expect(shouldRevalidate(args("/profile/me", "/create/preview", false))).toBe(true);
  expect(shouldRevalidate(args("/profile/me", "/Create/preview", false))).toBe(true);
  expect(shouldRevalidate(args("/create/preview", "/create/collections", false))).toBe(false);
  expect(shouldRevalidate(args("/create/preview", "/blog", false))).toBe(false);
});

it("keeps root's data between Shop pages and reloads it when the visitor leaves the Shop", () => {
  expect(shouldRevalidate(args("/shop/overview", "/shop/item/0x0768847951fddd4e73e670800da26d953a77059b/0", true))).toBe(false);
  expect(shouldRevalidate(args("/shop/item/0x0768847951fddd4e73e670800da26d953a77059b/0", "/shop/cart", true))).toBe(false);
  expect(shouldRevalidate(args("/shop", "/shop/overview", true))).toBe(false);
  expect(shouldRevalidate(args("/shop/cart", "/shop/overview", true, "POST"))).toBe(true);
  expect(shouldRevalidate(args("/shop/cart", "/places", true))).toBe(true);
  expect(shouldRevalidate(args("/places", "/shop/overview", true))).toBe(true);
  expect(shouldRevalidate(args("/shop/overview", "/shopping", true))).toBe(true);
  expect(shouldRevalidate(args("/shop/cart", "/create/scenes", true))).toBe(true);
});

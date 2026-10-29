import { createMemoryRouter } from "react-router";
import { expect, it, vi } from "vitest";

import type { CreatorHubHost } from "@ui/creatorhub/explorer-host";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { adoptExplorerHost, followExplorerIdentity, reloadingForAccount } from "@/features/components/chrome/explorer-host";

import { shouldRevalidate } from "./root";

const next: AuthIdentity = {
  signer: `0x${"a".repeat(40)}`,
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"1".repeat(64)}` },
  expiration: new Date(Date.now() + 86400000).toISOString(),
  authChain: [],
};

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
    [{ id: "root", path: "/", loader: rootLoader, shouldRevalidate, children: [{ path: "create/scenes", loader: pageLoader }] }],
    { initialEntries: ["/create/scenes"] },
  );
  await vi.waitFor(() => expect(router.state.initialized).toBe(true));
  // Another visit to the same page reloads only the page's own data.
  await router.navigate("/create/scenes", { replace: true });
  expect(pageLoader).toHaveBeenCalledTimes(2);
  expect(rootLoader).toHaveBeenCalledOnce();

  followExplorerIdentity(win, () => router.navigate("/create/scenes", { replace: true }));
  for (const listener of listeners) listener(next);
  expect(reloadingForAccount()).toBe(true);
  await vi.waitFor(() => expect(reloadingForAccount()).toBe(false));
  expect(pageLoader).toHaveBeenCalledTimes(3);
  expect(rootLoader).toHaveBeenCalledTimes(2);

  await router.navigate("/create/scenes", { replace: true });
  expect(rootLoader).toHaveBeenCalledTimes(2);
  router.dispose();
});

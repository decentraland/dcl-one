// Renders a Shop page the way a host mounts it: ui3's router with the /shop route table, the wallet seam
// (a signed-out, web2 or web3 session over a fake wallet), a fresh query client, and the document's Shop
// client pointed at an in-memory transport.

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { onTestFinished } from "vitest";

import type { AuthIdentity } from "../../data/auth/identity";
import { setShopClient } from "../api/shop";
import { shopRouteObjects } from "../routes";
import { ShopWalletProvider } from "../wallet/context";
import type { ShopWalletHost, WalletKind } from "../wallet/session";
import { fakeShop, fakeWallet, type FakeRoute, type FakeShop, type FakeWallet } from "./testing";

export function testIdentity(signer: string, ms = 3_600_000): AuthIdentity {
  return {
    signer,
    ephemeral: { address: "0x0000000000000000000000000000000000000001", privateKey: `0x${"11".repeat(32)}` },
    expiration: new Date(Date.now() + ms).toISOString(),
    authChain: [],
  };
}

export type Who = "signedOut" | "web2" | "web3";

export type Mounted = { shop: FakeShop; wallet: FakeWallet; host: ShopWalletHost; client: QueryClient };

export function mountShop(opts: { path: string; who: Who; routes?: FakeRoute[]; wallet?: FakeWallet; host?: Partial<ShopWalletHost> }): Mounted & ReturnType<typeof render> {
  const wallet = opts.wallet ?? fakeWallet();
  const shop = fakeShop(opts.routes ?? [], { signer: opts.who === "signedOut" ? null : wallet.address });
  setShopClient(shop.client);
  onTestFinished(() => setShopClient(null));
  const walletKind: WalletKind | null = opts.who === "web3" ? "web3" : opts.who === "web2" ? "managed" : null;
  const host: ShopWalletHost = {
    identity: opts.who === "signedOut" ? null : testIdentity(wallet.address),
    walletKind,
    provider: opts.who === "web3" ? () => wallet : null,
    ...opts.host,
  };
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } });
  onTestFinished(() => client.clear());
  const router = createMemoryRouter([{ path: "/shop", children: shopRouteObjects() }], { initialEntries: [opts.path] });
  const view = render(
    <QueryClientProvider client={client}>
      <ShopWalletProvider host={host}>
        <RouterProvider router={router} />
      </ShopWalletProvider>
    </QueryClientProvider>,
  );
  return { ...view, shop, wallet, host, client };
}

/** For stories: the same providers around a single element, with a fixed session. */
export function StoryShell({ who, wallet, children, host }: { who: Who; wallet?: FakeWallet; children: ReactNode; host?: Partial<ShopWalletHost> }) {
  const w = wallet ?? fakeWallet();
  const shellHost: ShopWalletHost = {
    identity: who === "signedOut" ? null : testIdentity(w.address, 365 * 86_400_000),
    walletKind: who === "web3" ? "web3" : who === "web2" ? "managed" : null,
    provider: who === "web3" ? () => w : null,
    ...host,
  };
  return <ShopWalletProvider host={shellHost}>{children}</ShopWalletProvider>;
}

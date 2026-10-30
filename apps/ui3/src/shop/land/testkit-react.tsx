// React wrappers for the LAND and NAME tests and stories: a router at `path` (with the Shop route the
// page expects) and the Shop's wallet context for a signed-out, web2 or web3 session.

import type { ReactNode } from "react";
import { MemoryRouter, Route, Routes } from "react-router";

import type { AuthIdentity } from "../../data/auth/identity";
import type { Eip1193Provider } from "../../data/auth/wallet";
import { createCartStore } from "../cart/store";
import { ShopWalletProvider } from "../wallet/context";
import type { ShopWalletHost } from "../wallet/session";

export type SessionKind = "signedOut" | "web2" | "web3";

export function testIdentity(signer: string, ms = 3_600_000): AuthIdentity {
  return {
    signer,
    ephemeral: { address: "0x0000000000000000000000000000000000000001", privateKey: `0x${"11".repeat(32)}` },
    expiration: new Date(Date.now() + ms).toISOString(),
    authChain: [],
  };
}

export type ShopHarnessProps = {
  children: ReactNode;
  session: SessionKind;
  account?: string;
  provider?: Eip1193Provider | null;
  path?: string;
  route?: string;
  host?: Partial<ShopWalletHost>;
};

export function ShopHarness({ children, session, account = "0x0000000000000000000000000000000000000b0b", provider = null, path = "/", route, host = {} }: ShopHarnessProps) {
  const walletHost: ShopWalletHost = {
    identity: session === "signedOut" ? null : testIdentity(account),
    walletKind: session === "web3" ? "web3" : session === "web2" ? "managed" : null,
    provider: provider ? () => provider : null,
    ...host,
  };
  return (
    <MemoryRouter initialEntries={[path]}>
      <ShopWalletProvider host={walletHost} cart={createCartStore({ storage: () => null })}>
        {route ? (
          <Routes>
            <Route path={route} element={children} />
            <Route path="*" element={<div data-testid="elsewhere" />} />
          </Routes>
        ) : (
          children
        )}
      </ShopWalletProvider>
    </MemoryRouter>
  );
}

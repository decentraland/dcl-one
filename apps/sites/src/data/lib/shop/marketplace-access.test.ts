// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";

const facts = vi.hoisted(() => ({
  identity: null as { signer: string; expiration: string } | null,
  kind: null as "web3" | "managed" | null,
  known: true,
}));
vi.mock("../auth/index", () => ({ useAuth: () => ({ identity: facts.identity }) }));
vi.mock("../auth/shop-wallet", () => ({ currentWalletKind: () => facts.kind, signInKnown: () => facts.known }));
const primed = vi.hoisted(() => ({ calls: 0 }));
vi.mock("../feature-flags.browser", () => ({ primeDappsFlags: async () => void primed.calls++ }));

import { useMarketplaceHere } from "./marketplace-access";

const ACCOUNT = "0x" + "ab".repeat(20);
const ON = { "dapps-shop-marketplace-features": true };
const LATER = new Date(Date.now() + 86_400_000).toISOString();

afterEach(() => {
  facts.identity = null;
  facts.kind = null;
  facts.known = true;
  primeRemoteFlags(null);
});

describe("the Marketplace on sites' own pages", () => {
  it("is on only for a web3 sign-in while the rollout is on", () => {
    for (const [flags, kind, expected] of [
      [ON, "web3", true],
      [ON, "managed", false],
      [{}, "web3", false],
    ] as const) {
      primeRemoteFlags(flags);
      facts.identity = { signer: ACCOUNT, expiration: LATER };
      facts.kind = kind;
      const { result, unmount } = renderHook(() => useMarketplaceHere());
      expect(result.current, `${kind} ${JSON.stringify(flags)}`).toBe(expected);
      unmount();
    }
  });

  it("is off for a web3 kind only guessed (a shared identity never signed in here), and for an expired one", () => {
    primeRemoteFlags(ON);
    facts.kind = "web3";
    facts.identity = { signer: ACCOUNT, expiration: LATER };
    facts.known = false;
    expect(renderHook(() => useMarketplaceHere()).result.current).toBe(false);
    facts.known = true;
    facts.identity = { signer: ACCOUNT, expiration: new Date(Date.now() - 1000).toISOString() };
    expect(renderHook(() => useMarketplaceHere()).result.current).toBe(false);
  });

  it("primes the flags from this server's reading, as the Shop's own pages do", () => {
    primed.calls = 0;
    renderHook(() => useMarketplaceHere());
    expect(primed.calls).toBe(1);
  });

  it("is off for a signed-out visitor", () => {
    primeRemoteFlags(ON);
    const { result } = renderHook(() => useMarketplaceHere());
    expect(result.current).toBe(false);
  });
});

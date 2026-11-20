// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";

const facts = vi.hoisted(() => ({
  identity: null as { signer: string; expiration: string } | null,
  kind: null as "web3" | "managed" | null,
  usable: true,
}));
vi.mock("../auth/index", () => ({ useAuth: () => ({ identity: facts.identity }) }));
vi.mock("../auth/shop-wallet", () => ({ currentWalletKind: () => facts.kind, web3Usable: () => facts.usable }));
const primed = vi.hoisted(() => ({ calls: 0 }));
vi.mock("../feature-flags.browser", () => ({ primeDappsFlags: async () => void primed.calls++ }));

import { useMarketplaceHere } from "./marketplace-access";

const ACCOUNT = "0x" + "ab".repeat(20);
const ON = { "dapps-shop-marketplace-features": true };
const LATER = new Date(Date.now() + 86_400_000).toISOString();

afterEach(() => {
  facts.identity = null;
  facts.kind = null;
  facts.usable = true;
  primed.calls = 0;
  primeRemoteFlags(null);
  vi.restoreAllMocks();
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
      const { result, unmount } = renderHook(() => useMarketplaceHere(ACCOUNT));
      expect(result.current, `${kind} ${JSON.stringify(flags)}`).toBe(expected);
      unmount();
    }
  });

  it("is off for a web3 kind only guessed (a shared identity never signed in here), and for an expired one", () => {
    primeRemoteFlags(ON);
    facts.kind = "web3";
    facts.identity = { signer: ACCOUNT, expiration: LATER };
    facts.usable = false;
    expect(renderHook(() => useMarketplaceHere(ACCOUNT)).result.current).toBe(false);
    facts.usable = true;
    facts.identity = { signer: ACCOUNT, expiration: new Date(Date.now() - 1000).toISOString() };
    expect(renderHook(() => useMarketplaceHere(ACCOUNT)).result.current).toBe(false);
  });

  it("primes the flags from this server's reading, as the Shop's own pages do", () => {
    primeRemoteFlags(ON);
    facts.identity = { signer: ACCOUNT, expiration: LATER };
    renderHook(() => useMarketplaceHere(ACCOUNT));
    expect(primed.calls).toBe(1);
  });

  it("skips both flag reads and focus refreshes for a signed-out visitor", () => {
    const readFlags = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("{}"));
    const { result } = renderHook(() => useMarketplaceHere(null));
    window.dispatchEvent(new Event("focus"));
    expect(result.current).toBe(false);
    expect(primed.calls).toBe(0);
    expect(readFlags).not.toHaveBeenCalled();
  });

  it("updates flag reads when the matching Sites session signs in and out", () => {
    primeRemoteFlags(ON);
    facts.kind = "web3";
    const { result, rerender } = renderHook<boolean, { account: string | null }>(
      ({ account }) => useMarketplaceHere(account),
      { initialProps: { account: null } },
    );

    facts.identity = { signer: ACCOUNT, expiration: LATER };
    rerender({ account: ACCOUNT });
    expect(result.current).toBe(true);
    expect(primed.calls).toBe(1);

    primeRemoteFlags(null);
    const readFlags = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("{}"));
    rerender({ account: null });
    window.dispatchEvent(new Event("focus"));
    expect(result.current).toBe(false);
    expect(readFlags).not.toHaveBeenCalled();
  });
});

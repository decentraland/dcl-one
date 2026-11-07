// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";

const facts = vi.hoisted(() => ({ identity: null as { signer: string } | null, kind: null as "web3" | "managed" | null }));
vi.mock("../auth/index", () => ({ useAuth: () => ({ identity: facts.identity }) }));
vi.mock("../auth/shop-wallet", () => ({ currentWalletKind: () => facts.kind }));

import { useMarketplaceHere } from "./marketplace-access";

const ACCOUNT = "0x" + "ab".repeat(20);
const ON = { "dapps-shop-marketplace-features": true };

afterEach(() => {
  facts.identity = null;
  facts.kind = null;
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
      facts.identity = { signer: ACCOUNT };
      facts.kind = kind;
      const { result, unmount } = renderHook(() => useMarketplaceHere());
      expect(result.current, `${kind} ${JSON.stringify(flags)}`).toBe(expected);
      unmount();
    }
  });

  it("is off for a signed-out visitor", () => {
    primeRemoteFlags(ON);
    const { result } = renderHook(() => useMarketplaceHere());
    expect(result.current).toBe(false);
  });
});

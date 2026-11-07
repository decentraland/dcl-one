import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { primeRemoteFlags, remoteFlagState, REMOTE_FLAG } from "../discovery/lib/remoteFlags";
import { evmAddress } from "../model/address";
import { SIGNED_OUT, type Session } from "../model/settlement";
import { ShopWalletProvider } from "../wallet/context";
import { fixtureHost, type SessionKind } from "../discovery/fixtures";
import { MarketplaceGate, marketplaceAccess } from "./access";
import { marketplaceOn } from "./testing-flags";

const ME = evmAddress("0x0000000000000000000000000000000000000b0b");
const web3: Session = { kind: "web3", wallet: ME };
const web2: Session = { kind: "web2", account: ME };

afterEach(() => primeRemoteFlags(null));

describe("who sees the Marketplace", () => {
  it("a web3 session, with the rollout on, on a node that offers resales: no one else", () => {
    expect(marketplaceAccess(true, { session: web3, resolving: false }, true)).toBe("on");
    expect(marketplaceAccess(true, { session: web2, resolving: false }, true)).toBe("off");
    expect(marketplaceAccess(true, { session: SIGNED_OUT, resolving: false }, true)).toBe("off");
    expect(marketplaceAccess(false, { session: web3, resolving: false }, true)).toBe("off");
    expect(marketplaceAccess(true, { session: web3, resolving: false }, false)).toBe("off");
  });

  it("waits while the flag has not answered or the session is being read back, never deciding early", () => {
    expect(marketplaceAccess(null, { session: web3, resolving: false }, true)).toBe("pending");
    expect(marketplaceAccess(true, { session: SIGNED_OUT, resolving: true }, true)).toBe("pending");
    // An answer that is already "off" needs no session at all.
    expect(marketplaceAccess(false, { session: SIGNED_OUT, resolving: true }, true)).toBe("off");
  });

  it("reads the flag as unanswered until the file answers, then as it says", () => {
    primeRemoteFlags(null);
    expect(remoteFlagState(REMOTE_FLAG.marketplaceFeatures)).toBeNull();
    primeRemoteFlags({});
    expect(remoteFlagState(REMOTE_FLAG.marketplaceFeatures)).toBe(false);
    primeRemoteFlags({ "dapps-shop-marketplace-features": true });
    expect(remoteFlagState(REMOTE_FLAG.marketplaceFeatures)).toBe(true);
  });
});

describe("a Marketplace page", () => {
  // An unprimed flag is read from the service: this one never answers, so the gate stays pending.
  beforeEach(() => vi.stubGlobal("fetch", vi.fn(() => new Promise<Response>(() => {}))));
  afterEach(() => vi.unstubAllGlobals());

  function page(session: SessionKind, flags: Record<string, boolean> | null) {
    primeRemoteFlags(flags);
    const router = createMemoryRouter(
      [
        { path: "/shop/lands", element: <MarketplaceGate><p>LAND PAGE</p></MarketplaceGate> },
        { path: "/shop/overview", element: <p>OVERVIEW</p> },
      ],
      { initialEntries: ["/shop/lands"] },
    );
    const view = render(
      <ShopWalletProvider host={fixtureHost(session)}>
        <RouterProvider router={router} />
      </ShopWalletProvider>,
    );
    return { router, unmount: view.unmount };
  }

  it("is drawn for a web3 session with the rollout on", () => {
    page("web3", { "dapps-shop-marketplace-features": true });
    expect(screen.getByText("LAND PAGE")).toBeInTheDocument();
  });

  it("follows a host's declared switch over the file's answer, either way", () => {
    const restore = marketplaceOn(false);
    const off = page("web3", { "dapps-shop-marketplace-features": true });
    expect(off.router.state.location.pathname).toBe("/shop/overview");
    off.router.dispose();
    off.unmount();
    restore();
    const undo = marketplaceOn(true);
    page("web3", {});
    expect(screen.getByText("LAND PAGE")).toBeInTheDocument();
    undo();
  });

  it("sends a web2 or signed-out visitor, or anyone while the rollout is off, to the overview", () => {
    for (const [session, flags] of [
      ["web2", { "dapps-shop-marketplace-features": true }],
      ["signedOut", { "dapps-shop-marketplace-features": true }],
      ["web3", {}],
    ] as const) {
      const { router, unmount } = page(session, flags);
      expect(router.state.location.pathname).toBe("/shop/overview");
      expect(screen.queryByText("LAND PAGE")).toBeNull();
      router.dispose();
      unmount();
    }
  });

  it("draws nothing while the flag has not answered, rather than sending a web3 visitor away", () => {
    const { router } = page("web3", null);
    expect(router.state.location.pathname).toBe("/shop/lands");
    expect(screen.queryByText("LAND PAGE")).toBeNull();
    expect(screen.queryByText("OVERVIEW")).toBeNull();
  });
});

// @vitest-environment jsdom
import { act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { shopFlags } from "@ui/shop/discovery/flags";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";
import type { ShopHost } from "@ui/shop/native-contract";

import { mountShop } from "./native.client";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type, $RefreshReg$: () => {} });
});
vi.mock("./native-routes", () => ({ nativeShopRoutes: () => [{ path: "*", element: <p>shop</p> }] }));

const host = {
  identity: null,
  path: "/shop",
  onSignIn: vi.fn(),
  onNavigate: vi.fn(),
  onExternalLink: vi.fn(),
} as unknown as ShopHost;

afterEach(() => {
  vi.unstubAllGlobals();
  primeRemoteFlags(null);
});

describe("the native shop", () => {
  it("renders only once decentraland.org's flags, read through this server, are primed", async () => {
    let answer: (r: Response) => void = () => {};
    const fetchMock = vi.fn(() => new Promise<Response>((resolve) => (answer = resolve)));
    vi.stubGlobal("fetch", fetchMock);
    const container = document.createElement("div");
    let mount: ReturnType<typeof mountShop> | undefined;
    // React gets every chance to render: nothing may show before the flags are in.
    await act(async () => {
      mount = mountShop(container, host);
    });
    expect(fetchMock).toHaveBeenCalledWith("/internal/dapps-flags?variants=1", expect.anything());
    expect(container.dataset.shopReady).toBeUndefined();

    await act(async () => answer(new Response(JSON.stringify({ "dapps-shop-creator-sales": true }))));
    expect(shopFlags().creatorSales).toBe(true);
    expect(container.dataset.shopReady).toBe("true");
    act(() => mount!.unmount());
  });

  it("still renders, with the flags off, when the endpoint cannot answer", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => {
      throw new TypeError("offline");
    }));
    const container = document.createElement("div");
    let mount: ReturnType<typeof mountShop> | undefined;
    await act(async () => {
      mount = mountShop(container, host);
    });
    expect(shopFlags().creatorSales).toBe(false);
    expect(container.dataset.shopReady).toBe("true");
    act(() => mount!.unmount());
  });
});

// @vitest-environment jsdom
import { act, createElement, type ReactNode } from "react";
import { hydrateRoot } from "react-dom/client";
import { renderToString } from "react-dom/server";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

import { useMyStoreAccess } from "@dcl/one-ui/apps/shop/capabilities";
import { REMOTE_FLAG, primeRemoteFlags, remoteFlag, remoteFlagVariant, useRemoteFlag } from "@dcl/one-ui/apps/shop/remote-flags";
import { shopFlags } from "@ui/shop/discovery/flags";

import { primeDappsFlags } from "@/data/lib/feature-flags.browser";
import { useMarketplaceHere } from "@/data/lib/shop/marketplace-access";
import { defaultShopSeo, type ShopPageData } from "@/data/lib/shop/pages";

import ShopLayout from "../routes/shop";
import { shopClientLoader } from "./page";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  });
});

vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => ({ identity: { signer: STORE_ACCOUNT, expiration: "2099-01-01T00:00:00.000Z" } }) }));

vi.mock("./host", () => ({
  default: function ShopFrame({ children }: { children: ReactNode }) {
    useMarketplaceHere(STORE_ACCOUNT);
    return children;
  },
}));

afterEach(() => {
  primeRemoteFlags(null);
  vi.unstubAllGlobals();
});

const STORE_ACCOUNT = "0x1111111111111111111111111111111111111111";
const OTHER_ACCOUNT = "0x2222222222222222222222222222222222222222";
const SALES_KEY = `dapps-${REMOTE_FLAG.creatorSales}`;
const STORE_KEY = `dapps-${REMOTE_FLAG.myStore}`;

function FlagProbe() {
  const deals = useRemoteFlag(REMOTE_FLAG.creatorSales);
  const store = useMyStoreAccess(STORE_ACCOUNT, false);
  return `Deals ${deals ? "available" : "unavailable"}, My Store ${store}`;
}

function shopPage(loaderData: ShopPageData) {
  const router = createMemoryRouter(
    [{ path: "/shop", Component: ShopLayout, children: [{ id: "shopPage", path: "overview", Component: FlagProbe, loader: () => loaderData }] }],
    { initialEntries: ["/shop/overview"], hydrationData: { loaderData: { shopPage: loaderData } } },
  );
  return createElement(RouterProvider, { router });
}

describe("when a Shop page hydrates with its server flags", () => {
  let data: ShopPageData;
  let container: HTMLDivElement;
  let response: Response;
  let onRecoverableError: Mock<() => void>;

  beforeEach(() => {
    data = {
      page: "overview",
      seo: defaultShopSeo("overview"),
      queries: [],
      featureFlags: { [SALES_KEY]: true, [STORE_KEY]: true },
      featureFlagVariants: { [STORE_KEY]: STORE_ACCOUNT },
    };
    container = document.createElement("div");
    container.innerHTML = renderToString(shopPage(data));
    response = Response.json({ flags: { [SALES_KEY]: false, [STORE_KEY]: true }, variants: { [STORE_KEY]: OTHER_ACCOUNT } });
    onRecoverableError = vi.fn();
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  });

  it("should keep the first paint and seed the store without asking for flags again", async () => {
    const fetchMock = vi.fn().mockResolvedValue(response);
    vi.stubGlobal("fetch", fetchMock);
    expect(container.textContent).toBe("Deals available, My Store on");
    expect(remoteFlag(REMOTE_FLAG.creatorSales)).toBe(false);

    const root = hydrateRoot(container, shopPage(data), { onRecoverableError });
    try {
      await act(async () => {});
      expect(fetchMock).not.toHaveBeenCalled();
      expect(container.textContent).toBe("Deals available, My Store on");
      expect(onRecoverableError).not.toHaveBeenCalled();
      expect(shopFlags().creatorSales).toBe(true);
      expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(STORE_ACCOUNT);
    } finally {
      await act(() => root.unmount());
    }
  });

  it("should keep both the rendered flags and the shared store after a late bootstrap response", async () => {
    let releaseResponse!: () => void;
    const delayed = new Promise<Response>((resolve) => {
      releaseResponse = () => resolve(response);
    });
    const fetchMock = vi.fn().mockReturnValueOnce(delayed).mockResolvedValue(response.clone());
    vi.stubGlobal("fetch", fetchMock);
    const pending = primeDappsFlags();

    const root = hydrateRoot(container, shopPage(data), { onRecoverableError });
    try {
      await act(async () => {});
      await act(async () => {
        releaseResponse();
        await pending;
      });
      expect(container.textContent).toBe("Deals available, My Store on");
      expect(shopFlags().creatorSales).toBe(true);
      expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(STORE_ACCOUNT);
      expect(fetchMock).toHaveBeenCalledOnce();
    } finally {
      releaseResponse();
      await act(() => root.unmount());
    }
  });
});

describe("when a client navigation reaches Shop", () => {
  it("should prime the flags before the page renders", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => Response.json({ [SALES_KEY]: true })));
    expect(shopFlags().creatorSales).toBe(false);
    expect(await shopClientLoader()).toBeNull();
    expect(shopFlags().creatorSales).toBe(true);
  });
});

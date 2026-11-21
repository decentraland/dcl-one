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

import { DEFAULT_SOCIAL_IMAGE } from "../lib/social-meta";
import ShopLayout from "../routes/shop";
import { shopClientLoader, shopMeta } from "./page";

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

describe("a Shop page's head", () => {
  const C = "0xabc0000000000000000000000000000000000001";
  const matches = [{ id: "root", loaderData: { publicPageBase: "https://dcl.test" } }];
  type Tags = ReturnType<ReturnType<typeof shopMeta>>;
  const content = (tags: Tags, key: string) =>
    tags.filter((t) => ("property" in t && t.property === key) || ("name" in t && t.name === key)).map((t) => (t as { content: string }).content);
  const data = (page: "itemDetail" | "tokenDetail" | "overview" | "cart", seo: object) => ({ page, seo: { description: "A hat.", noindex: false, ...seo }, queries: [] });

  it("an item with a thumbnail is a small product card under the Shop's name, canonical to the item", () => {
    const canonical = `https://dcl.test/shop/item/${C}/7`;
    const tags = shopMeta("itemDetail")({ loaderData: data("itemDetail", { title: "Pirate Hat", image: "https://img.test/7.png", canonical }), matches });
    expect(tags[0]).toEqual({ title: "Pirate Hat | Decentraland Shop" });
    expect(content(tags, "og:title")).toEqual(["Pirate Hat | Decentraland Shop"]);
    expect(content(tags, "twitter:title")).toEqual(["Pirate Hat | Decentraland Shop"]);
    expect(content(tags, "description")).toEqual(["A hat."]);
    expect(content(tags, "twitter:description")).toEqual(["A hat."]);
    expect(content(tags, "og:image")).toEqual(["https://img.test/7.png"]);
    expect(content(tags, "twitter:image")).toEqual(["https://img.test/7.png"]);
    expect(content(tags, "twitter:card")).toEqual(["summary"]);
    expect(content(tags, "twitter:site")).toEqual(["@decentraland"]);
    expect(content(tags, "twitter:creator")).toEqual(["@decentraland"]);
    expect(content(tags, "og:type")).toEqual(["product"]);
    expect(content(tags, "og:site_name")).toEqual(["Decentraland Shop"]);
    expect(content(tags, "og:locale")).toEqual(["en_US"]);
    expect(content(tags, "og:url")).toEqual([canonical]);
    expect(tags).toContainEqual({ tagName: "link", rel: "canonical", href: canonical });
    expect(content(tags, "robots")).toEqual([]);
    expect(tags.some((t) => "script:ld+json" in t)).toBe(false);
  });

  it("an item or token without an image shows the shared image as a large card", () => {
    for (const page of ["itemDetail", "tokenDetail"] as const) {
      const tags = shopMeta(page)({ loaderData: data(page, { title: "Pirate Hat" }), matches });
      expect(content(tags, "og:image")).toEqual([DEFAULT_SOCIAL_IMAGE]);
      expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
      expect(content(tags, "og:type")).toEqual(["product"]);
    }
  });

  it("a token with an image is small, and a LAND token's map is large", () => {
    const token = shopMeta("tokenDetail")({ loaderData: data("tokenDetail", { title: "Pirate Hat #5", image: "https://img.test/5.png" }), matches });
    expect(content(token, "twitter:card")).toEqual(["summary"]);
    const land = shopMeta("tokenDetail")({ loaderData: data("tokenDetail", { title: "Parcel", image: "https://img.test/map.png", largeImage: true, noindex: true }), matches });
    expect(content(land, "og:image")).toEqual(["https://img.test/map.png"]);
    expect(content(land, "twitter:card")).toEqual(["summary_large_image"]);
  });

  it("a private page stays out of the index with no canonical", () => {
    const tags = shopMeta("cart")({ loaderData: null, matches });
    expect(content(tags, "robots")).toEqual(["noindex"]);
    expect(content(tags, "og:url")).toEqual([]);
    expect(tags.some((t) => "rel" in t)).toBe(false);
    expect(content(tags, "og:type")).toEqual(["website"]);
  });

  it("a public page's canonical is the loader's, and the event grid is never indexed", () => {
    expect(shopMeta("items")({ loaderData: { page: "items", seo: { title: "Items", description: "x", noindex: false, canonical: "https://x.test/shop/items" }, queries: [] } })).toContainEqual({
      tagName: "link",
      rel: "canonical",
      href: "https://x.test/shop/items",
    });
    const event = shopMeta("event")({ loaderData: { page: "event", seo: { title: "Event", description: "x", noindex: false, canonical: "https://x.test/shop/event" }, queries: [] } });
    expect(content(event, "robots")).toEqual(["noindex"]);
    expect(event.some((t) => "rel" in t)).toBe(false);
  });

  it("the overview keeps the Shop's default title and describes the Shop as a site", () => {
    const tags = shopMeta("overview")({ loaderData: data("overview", { title: "Shop", canonical: "https://dcl.test/shop/overview" }), matches });
    expect(tags[0]).toEqual({ title: "Decentraland Shop | Wearables & Emotes for Your Avatar" });
    expect(content(tags, "og:type")).toEqual(["website"]);
    expect(content(tags, "twitter:card")).toEqual(["summary_large_image"]);
    expect(tags).toContainEqual({
      "script:ld+json": {
        "@context": "https://schema.org",
        "@type": "WebSite",
        name: "Decentraland Shop",
        url: "https://dcl.test/shop/overview",
        description: "A hat.",
        isPartOf: { "@type": "WebSite", name: "Decentraland", url: "https://dcl.test/" },
        publisher: { "@type": "Organization", name: "Decentraland", url: "https://dcl.test/" },
      },
    });
  });
});

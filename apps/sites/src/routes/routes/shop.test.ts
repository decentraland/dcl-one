import { beforeEach, describe, expect, it, vi } from "vitest";

import { track } from "@/core/lib/telemetry/track";
import { loadShopPage } from "@/data/lib/shop/pages.server";

import { loader as indexLoader } from "./shop._index";
import { loader as splatLoader } from "./shop.$";
import { loader as creditsLoader } from "./shop.credits";
import { headers as itemHeaders, loader as itemLoader, meta as itemMeta } from "./shop.item.$contractAddress.$itemId";

vi.mock("@/data/lib/shop/pages.server", () => ({ loadShopPage: vi.fn() }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

const C = "0xabc0000000000000000000000000000000000001";

beforeEach(() => {
  vi.mocked(track).mockReset();
  vi.mocked(loadShopPage).mockReset().mockImplementation(async (page) => ({
    status: page === "notFound" ? 404 : 200,
    data: { page, seo: { title: "Pirate Hat", description: "A hat.", noindex: page === "notFound" }, queries: [[["shop-item", C, "7"], { name: "Pirate Hat" }, 1]] },
  }));
});

const args = (url: string, params: Record<string, string> = {}) => ({ request: new Request(`https://sites.test${url}`), params, context: {} }) as never;

async function thrown(fn: () => unknown): Promise<Response> {
  try {
    const out = await fn();
    return out as Response;
  } catch (e) {
    return e as Response;
  }
}

describe("the /shop routes", () => {
  it("/shop redirects to the overview, and v2's ?tab= to its page", async () => {
    const index = (url: string) => indexLoader(args(url)) as Response;
    expect([index("/shop").status, index("/shop").headers.get("Location")]).toEqual([308, "/shop/overview"]);
    expect(index("/shop?tab=all-assets&category=emote&search=wave").headers.get("Location")).toBe("/shop/items?category=emote&q=wave");
    expect(index("/shop?tab=my-assets").headers.get("Location")).toBe("/shop/my-items");
  });

  it("the upstream aliases redirect with the query; anything else is the Shop's 404", async () => {
    const alias = await thrown(() => splatLoader(args("/shop/assets/creator/0x1?x=1", { "*": "assets/creator/0x1" })));
    expect([alias.status, alias.headers.get("Location")]).toEqual([308, "/shop/items/creator/0x1?x=1"]);
    const parcel = await thrown(() => splatLoader(args("/shop/parcels/0/0/detail")));
    expect(parcel.headers.get("Location")).toMatch(/^\/shop\/token\/0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d\/\d+$/);
    const missing = (await splatLoader(args("/shop/nope"))) as unknown as { init: ResponseInit | null; data: { page: string } };
    expect(missing.init?.status).toBe(404);
    expect(missing.data.page).toBe("notFound");
  });

  it("the iOS app's web view never reaches the credits page", async () => {
    const res = await thrown(() => creditsLoader(args("/shop/credits?view=mobile-iap")));
    expect(res.headers.get("Location")).toBe("/shop/overview?view=mobile-iap");
  });

  it("a page's loader answers its SSR data with the session cookie; documents are never publicly cached", async () => {
    const res = (await itemLoader(args(`/shop/item/${C}/7`, { contractAddress: C, itemId: "7" }))) as unknown as { init: ResponseInit | null; data: { seo: { title: string } } };
    expect(res.data.seo.title).toBe("Pirate Hat");
    expect(new Headers(res.init?.headers).get("Set-Cookie")).toContain("sid=");
    expect(vi.mocked(loadShopPage).mock.calls[0]![0]).toBe("itemDetail");
    const headers = itemHeaders({ loaderHeaders: new Headers(), parentHeaders: new Headers({ "X-Parent": "kept" }) });
    expect([headers.get("Cache-Control"), headers.get("X-Parent")]).toEqual(["private, no-store", "kept"]);
    expect(itemMeta({ loaderData: res.data })).toContainEqual({ title: "Pirate Hat | Decentraland Shop" });
    expect(itemMeta({ loaderData: null })).toContainEqual({ title: "Item | Decentraland Shop" });
  });

  it("a friend-first-wear notification's landing is still measured on the Shop item page", async () => {
    await itemLoader(args(`/shop/item/${C}/7?src=ffw&nid=n-1`, { contractAddress: C, itemId: "7" }));
    expect(track).toHaveBeenCalledWith(
      "ffw_shop_landing",
      { item_id: `${C}-7`, nid: "n-1", on_sale: true },
      expect.objectContaining({ sid: expect.any(String), story: "notifications/first-wear" }),
    );
  });
});

describe("the Creator Hub's publish flow through /shop/names", () => {
  it("returns to publishing with its project, World and origin, and the registered NAME", async () => {
    const { returnToPublishHref } = await import("./shop.names");
    const q = new URLSearchParams("from=deploy-world&project=p1&world=w1&origin=create");
    expect(returnToPublishHref(q)).toBe("/create/deploy-world?project=p1&world=w1&from=create");
    expect(returnToPublishHref(q, "ana.dcl.eth")).toBe("/create/deploy-world?name=ana.dcl.eth&claimed=1&project=p1&world=w1&from=create");
  });
});

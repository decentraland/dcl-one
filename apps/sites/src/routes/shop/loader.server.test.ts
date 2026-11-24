import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { defaultShopSeo } from "@/data/lib/shop/pages";
import { loadShopPage } from "@/data/lib/shop/pages.server";

import { shopLoader } from "./loader.server";

vi.mock("@/data/lib/shop/pages.server", () => ({ loadShopPage: vi.fn() }));

const BROWSER_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const PAGE = "https://sites.example/shop/item/0xabc/1";
const PARAMS = { contractAddress: "0xABC", itemId: "1" };

describe("shopLoader", () => {
  const fetchMock = vi.fn<typeof fetch>();

  beforeEach(() => {
    vi.stubEnv("SEGMENT_WRITE_KEY", "sites-key");
    vi.stubEnv("SHOP_SEGMENT_WRITE_KEY", "shop-key");
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockResolvedValue(new Response(null, { status: 200 }));
    vi.mocked(loadShopPage).mockResolvedValue({
      data: { page: "itemDetail", queries: [], seo: defaultShopSeo("itemDetail", PAGE) },
      status: 200,
    });
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    vi.resetAllMocks();
  });

  describe("when a visitor opens a first-wear notification", () => {
    it("should report the legacy landing to the Sites source", async () => {
      await load("?src=ffw&nid=notification", BROWSER_UA);
      expect(fetchMock).toHaveBeenCalledTimes(1);
      expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toMatchObject({
        event: "ffw_shop_landing",
        writeKey: "sites-key",
        properties: { item_id: "0xabc-1", nid: "notification", on_sale: false },
      });
    });
  });

  describe("when a bot opens a first-wear notification", () => {
    it("should load the item without reporting a landing", async () => {
      await load("?src=ffw&nid=notification", "curl/8.6.0");
      expect(loadShopPage).toHaveBeenCalledOnce();
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  describe("when the item visit has no first-wear source", () => {
    it("should load the item without reporting a landing", async () => {
      await load("", BROWSER_UA);
      expect(loadShopPage).toHaveBeenCalledOnce();
      expect(fetchMock).not.toHaveBeenCalled();
    });
  });

  function load(search: string, userAgent: string) {
    const request = new Request(PAGE + search, { headers: { "User-Agent": userAgent } });
    return shopLoader("itemDetail", request, PARAMS, new RouterContextProvider());
  }
});

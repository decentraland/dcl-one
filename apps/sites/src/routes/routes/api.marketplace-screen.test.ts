import { beforeEach, expect, it, vi } from "vitest";

import { loadShopScreen } from "@/data/lib/screens/shop.server";
import { loader as alias } from "./api.marketplace-screen";
import { loader as canonical } from "./api.screens.v1.shop";

vi.mock("@/data/lib/screens/shop.server", () => ({ loadShopScreen: vi.fn() }));

beforeEach(() => {
  vi.mocked(loadShopScreen).mockReset().mockResolvedValue({
    data: {
      filters: { tab: "overview", page: 0, category: "", rarity: "", search: "", sortBy: "recently_listed" },
      cards: [], topCards: [], trendingCards: [], total: 0, fallback: false,
      sections: { catalog: "ready", top: "ready", trending: "ready" },
    },
    serverTiming: "shop_total;dur=1.0",
  });
});

it("/api/marketplace-screen answers exactly what /api/screens/v1/shop answers", async () => {
  const a = await alias({ request: new Request("https://sites.test/api/marketplace-screen?tab=all-assets") });
  const c = await canonical({ request: new Request("https://sites.test/api/screens/v1/shop?tab=all-assets") });
  expect(await a.json()).toEqual(await c.json());
  expect(a.headers.get("Cache-Control")).toBe(c.headers.get("Cache-Control"));
  expect(vi.mocked(loadShopScreen).mock.calls.map(([params]) => params.toString())).toEqual(["tab=all-assets", "tab=all-assets"]);
});

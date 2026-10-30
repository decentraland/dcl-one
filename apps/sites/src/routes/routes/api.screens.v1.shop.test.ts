import { beforeEach, expect, it, vi } from "vitest";

import { loadShopScreen } from "@/data/lib/screens/shop.server";
import { loader } from "./api.screens.v1.shop";

vi.mock("@/data/lib/screens/shop.server", () => ({ loadShopScreen: vi.fn() }));

beforeEach(() => {
  vi.mocked(loadShopScreen).mockReset().mockResolvedValue({
    data: {
      filters: { tab: "overview", page: 0, category: "", rarity: "", search: "", sortBy: "recently_listed" },
      cards: [], topCards: [], trendingCards: [], total: 0, fallback: false,
      sections: { catalog: "ready", top: "ready", trending: "ready" },
    },
    serverTiming: "shop_total;dur=12.3",
  });
});

it("serves the versioned public JSON contract without a session id or cookie", async () => {
  const request = new Request("https://sites.test/api/screens/v1/shop");
  const response = await loader({ request });
  const body = await response.json();
  expect(body).toMatchObject({ version: 1, cards: [], sections: { catalog: "ready" } });
  expect(body).not.toHaveProperty("sid");
  expect(response.headers.has("Set-Cookie")).toBe(false);
  expect(response.headers.get("Server-Timing")).toBe("shop_total;dur=12.3");
  expect(response.headers.get("Cache-Control")).toBe("no-store");
});

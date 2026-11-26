// The overview route's client loader: a link to the overview sends its rails beside the flags
// read, with the Shop client in place, and the navigation settles with the flags.

import { describe, expect, it, vi } from "vitest";

vi.mock("@ui/shop/discovery/pages/overviewRails", () => ({ prefetchOverviewRails: vi.fn() }));
vi.mock("@ui/shop/pages/Overview", () => ({ default: () => null }));
vi.mock("@/data/lib/auth/shop-client", () => ({ installSitesShopClient: vi.fn() }));
vi.mock("@/data/lib/feature-flags.browser", () => ({ primeDappsFlags: vi.fn() }));
vi.mock("../shop/loader.server", () => ({ shopLoader: vi.fn() }));
vi.mock("../shop/page", () => ({ ShopPage: () => null, shopHeaders: () => new Headers(), shopMeta: () => () => [] }));

import { prefetchOverviewRails } from "@ui/shop/discovery/pages/overviewRails";
import { installSitesShopClient } from "@/data/lib/auth/shop-client";
import { primeDappsFlags } from "@/data/lib/feature-flags.browser";

import { clientLoader } from "./shop.overview";

describe("the overview's client loader", () => {
  it("sends the rails with the flags read, after the Shop client is in place, and settles with the flags", async () => {
    let answer!: () => void;
    const flags = new Promise<void>((resolve) => (answer = resolve));
    vi.mocked(primeDappsFlags).mockReturnValue(flags as ReturnType<typeof primeDappsFlags>);
    let settled = false;
    const loaded = clientLoader().then((data) => {
      settled = true;
      return data;
    });
    expect(prefetchOverviewRails).toHaveBeenCalledWith(flags);
    expect(vi.mocked(installSitesShopClient).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(prefetchOverviewRails).mock.invocationCallOrder[0]!,
    );
    await Promise.resolve();
    expect(settled).toBe(false);
    answer();
    expect(await loaded).toBeNull();
  });
});

import { describe, expect, it } from "vitest";

import { fakeShop } from "../../testing";
import { fetchAccountNfts } from "./api";

const OWNER = "0x0000000000000000000000000000000000000b0b";

describe("an account's NFTs by section", () => {
  it("asks for a wearable group by its flag, and for a single category by its name", async () => {
    const shop = fakeShop([{ method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ body: { data: [], total: 0 } }) }]);
    const asked = (section: Parameters<typeof fetchAccountNfts>[1]["section"]) =>
      fetchAccountNfts(OWNER, { section, first: 24, skip: 0, sortBy: "newest", onlyOnSale: false }, shop.client).then(() => shop.requests.at(-1)!.url.searchParams);
    const head = await asked("wearables_head");
    expect(head.get("isWearableHead")).toBe("true");
    expect(head.getAll("wearableCategory")).toEqual([]);
    const accessories = await asked("wearables_accessories");
    expect(accessories.get("isWearableAccessory")).toBe("true");
    expect(accessories.getAll("wearableCategory")).toEqual([]);
    const upper = await asked("wearables_upper_body");
    expect(upper.getAll("wearableCategory")).toEqual(["upper_body"]);
  });
});

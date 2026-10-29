
import { render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PRIMARY_WEARABLE, ShopHarness, type CatalogAnswers } from "../fixtures";
import { invalidateShopQueries } from "../query";
import OverviewOutfitsRow from "./OverviewOutfitsRow";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

const look = (id: string, name: string, items: { contractAddress: string; itemId: string }[]) => ({
  id,
  name,
  thumbnailHash: "",
  items,
  bodyShape: "unisex",
  gradientFrom: "#112233",
  gradientTo: "#445566",
  published: true,
});

function row(overrides?: CatalogAnswers) {
  return render(
    <ShopHarness path="/shop/overview" entry="/shop/overview" overrides={overrides}>
      <OverviewOutfitsRow />
    </ShopHarness>,
  );
}

describe("the overview's Buy the Look row", () => {
  it("shows each fully buyable look with its credit total, and leaves out a look that no longer resolves", async () => {
    const hat = { contractAddress: PRIMARY_WEARABLE.contractAddress, itemId: PRIMARY_WEARABLE.itemId! };
    row({
      "/v1/outfits": {
        outfits: [look("o1", "Neon Bloom", [hat]), look("o2", "Gone Look", [{ contractAddress: "0x0000000000000000000000000000000000000001", itemId: "9" }])],
      },
    });
    const section = await screen.findByTestId("outfits-row");
    expect(section).toHaveTextContent("Buy the Look");
    await waitFor(() => expect(within(section).getAllByTestId("outfit-card")).toHaveLength(1));
    const cards = within(section).getAllByTestId("outfit-card");
    expect(cards[0]).toHaveTextContent("Neon Bloom");
    expect(cards[0]).toHaveTextContent("1 item");
    expect(cards[0]).toHaveTextContent("12");
    expect(cards[0]!.getAttribute("href")).toBe("/shop/items/outfits/o1");
  });
});

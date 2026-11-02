
import { render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PRIMARY_WEARABLE, ShopHarness, type CatalogAnswers } from "../fixtures";
import { invalidateShopQueries } from "../query";
import OverviewOutfitsRow, { orderOutfitsByRecency } from "./OverviewOutfitsRow";

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
    expect(within(cards[0]!).getByText("12 Credits")).toBeTruthy();
    expect(cards[0]!.getAttribute("href")).toBe("/shop/items/outfits/o1");
  });
});

describe("orderOutfitsByRecency", () => {
  const DAY = 24 * 60 * 60 * 1000;
  const NOW = Date.UTC(2026, 9, 30);

  it("shuffles, but puts each age bucket (7, 21, 60, 365 days, older) ahead of the next", () => {
    const outfits = [400, 2, 30, 100, 10, 5, 3000, 20].map((days, i) => ({ id: i, createdAt: NOW - days * DAY }));
    const bucket = (createdAt: number) => {
      const i = [7, 21, 60, 365].findIndex((d) => NOW - createdAt < d * DAY);
      return i === -1 ? 4 : i;
    };
    for (let run = 0; run < 20; run++) {
      const order = orderOutfitsByRecency(outfits, NOW).map((o) => bucket(o.createdAt));
      expect(order).toHaveLength(outfits.length);
      expect(order).toEqual([...order].sort((a, b) => a - b));
    }
  });

  it("leaves the list it is given as it was", () => {
    const outfits = [{ createdAt: NOW - 30 * DAY }, { createdAt: NOW - DAY }];
    const copy = [...outfits];
    orderOutfitsByRecency(outfits, NOW);
    expect(outfits).toEqual(copy);
  });
});

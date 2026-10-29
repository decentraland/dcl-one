import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BOOK } from "../../payment/fixtures";
import { COLLECTOR_RANKS, CREATOR_RANKS, ITEM_RANKS, MANA, VOLUME } from "../fixtures";
import type { FakeRoute } from "../testing";
import { mountShop } from "../testHarness";
import { parseRankingsHash } from "./rankings/RankingsTable";

const catalog: FakeRoute = {
  method: "GET",
  path: /^\/v3\/catalog\/items$/,
  reply: ({ url }) => ({
    body: {
      data: url.searchParams.getAll("id").map((id) => {
        const itemId = id.split("-")[1]!;
        return { id, itemId, name: `Item ${itemId}`, thumbnail: "", url: "", urn: "", category: "wearable", contractAddress: BOOK.collection, rarity: "mythic", available: 5, isOnSale: true, creator: BOOK.creator, data: { wearable: { category: "eyewear" } }, network: "MATIC", chainId: 137, price: MANA(2), priceCredits: 12 };
      }),
      total: 2,
    },
  }),
};

describe("Rankings (upstream marketplace home RankingsTable)", () => {
  it("renders upstream's wearables columns, switches tabs and timeframes (no volume card: dropped upstream in 0fdb65fe)", async () => {
    mountShop({
      path: "/shop/rankings",
      who: "signedOut",
      routes: [
        { method: "GET", path: /^\/v1\/volume\/week$/, reply: () => ({ body: { data: VOLUME } }) },
        { method: "GET", path: /^\/v1\/rankings\/wearables\/week$/, reply: () => ({ body: { data: ITEM_RANKS } }) },
        { method: "GET", path: /^\/v1\/rankings\/creators\/week$/, reply: () => ({ body: { data: CREATOR_RANKS } }) },
        { method: "GET", path: /^\/v1\/rankings\/collectors\/week$/, reply: () => ({ body: { data: COLLECTOR_RANKS } }) },
        { method: "GET", path: /^\/v1\/rankings\/creators\/all$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "all needs the index", source: "nft-indexer" } }) },
        catalog,
      ],
    });
    expect(await screen.findByRole("heading", { level: 1, name: "Rankings" })).toBeInTheDocument();
    expect(screen.getByText("Includes only Wearables and Emotes")).toBeInTheDocument();
    expect(await screen.findByText("Item 7")).toBeInTheDocument();
    for (const h of ["Wearable", "Category", "Rarity", "Wearables sold"]) expect(screen.getByRole("columnheader", { name: h })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: /^Total Volume/ })).toBeInTheDocument();
    const row = screen.getByText("Item 7").closest("tr")!;
    expect(within(row).getByText("Eyewear")).toBeInTheDocument();
    expect(within(row).getByText("Mythic")).toBeInTheDocument();
    expect(row).toHaveTextContent("420");
    expect(screen.queryByTestId("shop-volume")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Creators" }));
    expect(await screen.findByRole("columnheader", { name: /^Total Volume of Sales/ })).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: /^Unique collectors/ })).toBeInTheDocument();
    expect(await screen.findByText("77")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Collectors" }));
    expect(await screen.findByRole("columnheader", { name: /^Total Spent/ })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Creators" }));
    fireEvent.click(within(screen.getAllByRole("group", { name: "Timeframe" })[0]!).getByRole("button", { name: "All" }));
    expect(await screen.findByTestId("shop-outage")).toHaveTextContent("the NFT index is still catching up");
  });

  it("shows upstream's empty state when a timeframe has no sales", async () => {
    mountShop({
      path: "/shop/rankings",
      who: "signedOut",
      routes: [
        { method: "GET", path: /^\/v1\/volume\//, reply: () => ({ body: { data: VOLUME } }) },
        { method: "GET", path: /^\/v1\/rankings\//, reply: () => ({ body: { data: [] } }) },
        catalog,
      ],
    });
    expect(await screen.findByText("No results found for these filters")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear filters" })).toBeInTheDocument();
  });

  it("reads the #rankings- hash", () => {
    expect(parseRankingsHash("#rankings-creators-month-most_sales")).toEqual({ entity: "creators", timeframe: "month", sortBy: "most_sales" });
    expect(parseRankingsHash("#other")).toBeNull();
  });
});

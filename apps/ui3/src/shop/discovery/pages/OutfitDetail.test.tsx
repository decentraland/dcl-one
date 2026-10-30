
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createShopClient } from "../../api/shop";
import { shopCart } from "../../cart/store";
import { catalogClient, catalogTransport, COLLECTION, ShopHarness, type CatalogAnswers, type CatalogScenario } from "../fixtures";
import OutfitDetail from "./OutfitDetail";

const OUTFIT = {
  id: "0f1e2d3c-4b5a-4968-8778-695a4b3c2d1e",
  name: "Nacho Night",
  thumbnailHash: "",
  items: [
    { contractAddress: COLLECTION, itemId: "7" },
    { contractAddress: COLLECTION, itemId: "8" },
  ],
  bodyShape: "unisex",
  gradientFrom: "#3a0653",
  gradientTo: "#175e52",
  published: true,
};

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  shopCart.clear();
});
afterEach(() => vi.unstubAllGlobals());

function outfit(overrides: CatalogAnswers = { [`/v1/outfits/${OUTFIT.id}`]: { outfit: OUTFIT } }, scenario: CatalogScenario = "ok", legacy = false) {
  const client = legacy
    ? createShopClient({ base: "http://shop.test", mode: "legacy", transport: catalogTransport(scenario, overrides), signer: null })
    : catalogClient(scenario, overrides);
  render(
    <ShopHarness path="/shop/items/outfits/:id" entry={`/shop/items/outfits/${OUTFIT.id}`} client={client} session="web2">
      <OutfitDetail />
    </ShopHarness>,
  );
}

describe("an outfit", () => {
  it("loading shows a busy page", () => {
    outfit(undefined, "loading");
    expect(screen.getByTestId("outfit-loading").getAttribute("aria-busy")).toBe("true");
  });

  it("lists its items as rows and adds the look as one line per item", async () => {
    outfit();
    expect(await screen.findByRole("heading", { level: 1, name: "Nacho Night" })).toBeTruthy();
    expect(screen.getByTestId("outfit-meta")).toHaveTextContent("2 items in this outfit");
    const list = screen.getByTestId("outfit-items");
    await waitFor(() => expect(within(list).getAllByRole("link", { name: /.+/ }).length).toBeGreaterThanOrEqual(2));
    expect(within(list).getAllByTestId("outfit-detail-item")).toHaveLength(2);
    await waitFor(() => expect(screen.getByTestId("add-look")).toHaveTextContent("Add 2 items to cart"));
    fireEvent.click(screen.getByTestId("add-look"));
    await waitFor(() => expect(shopCart.getSnapshot().lines).toHaveLength(2));
    const lines = shopCart.getSnapshot().lines;
    expect(lines.every((l) => l.provenance.outfitId === OUTFIT.id && l.method === null)).toBe(true);
    await waitFor(() => expect(screen.getByTestId("add-look")).toHaveTextContent("Added to cart"));
  });

  it("an unknown outfit is not found", async () => {
    outfit({});
    expect(await screen.findByTestId("outfit-notfound")).toHaveTextContent("This outfit isn't available");
  });

  it("a failed read says so with a retry", async () => {
    outfit(undefined, "error");
    expect(await screen.findByTestId("outfit-error")).toHaveTextContent("Something went wrong. Please try again.");
  });

  it("a node without the outfits service (legacy mode: no alias) says so instead of failing", async () => {
    outfit(undefined, "ok", true);
    expect(await screen.findByTestId("outfit-unavailable")).toHaveTextContent("Outfits are not served by this node yet.");
  });
});

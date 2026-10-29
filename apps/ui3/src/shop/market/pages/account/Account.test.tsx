import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BOOK } from "../../../payment/fixtures";
import { OWNED_NAMES, OWNED_WEARABLES } from "../../fixtures";
import type { FakeRoute } from "../../testing";
import { mountShop } from "../../testHarness";
import { parseSection, sortOptions } from "./api";

const nfts = (seen: URLSearchParams[]): FakeRoute => ({
  method: "GET",
  path: /^\/v1\/nfts$/,
  reply: ({ url }) => {
    seen.push(url.searchParams);
    const rows = url.searchParams.get("category") === "ens" ? OWNED_NAMES : OWNED_WEARABLES;
    return { body: { data: rows, total: rows.length === 3 ? 1472 : rows.length } };
  },
});
const catalog: FakeRoute = { method: "GET", path: /^\/v3\/catalog\/items$/, reply: () => ({ body: { data: [], total: 0 } }) };

describe("Account (upstream Marketplace AccountPage)", () => {
  it("opens on Collectibles > All Assets with not-on-sale included, and prices resales only in MANA", async () => {
    const seen: URLSearchParams[] = [];
    mountShop({ path: `/shop/accounts/${BOOK.creator}`, who: "signedOut", routes: [nfts(seen), catalog] });
    expect(await screen.findByTestId("shop-account-banner")).toBeInTheDocument();
    const tiles = await screen.findAllByTestId("shop-asset-tile");
    expect(tiles).toHaveLength(3);
    expect(screen.getByTestId("shop-account-count")).toHaveTextContent("1,472 items");
    expect(screen.getByRole("button", { name: /Includes not on sale/i })).toBeInTheDocument();
    const q = seen[0]!;
    expect(q.get("owner")).toBe(BOOK.creator.toLowerCase());
    expect(q.get("category")).toBeNull();
    expect(q.get("isOnSale")).toBeNull();
    expect(within(tiles[0]!).getByTestId("shop-asset-price")).toHaveTextContent("12.5");
    expect(document.body.textContent).not.toMatch(/credits/i);
  });

  it("the sidebar switches sections and to Originals", async () => {
    const seen: URLSearchParams[] = [];
    mountShop({ path: `/shop/accounts/${BOOK.creator}`, who: "signedOut", routes: [nfts(seen), catalog] });
    await screen.findAllByTestId("shop-asset-tile");
    fireEvent.click(screen.getByRole("button", { name: "Names" }));
    await waitFor(() => expect(seen.at(-1)!.get("category")).toBe("ens"));
    fireEvent.click(screen.getByRole("button", { name: /Originals/ }));
    expect(await screen.findByTestId("shop-empty")).toHaveTextContent("No items found.");
    expect(screen.getByRole("heading", { name: "CATEGORIES" })).toBeInTheDocument();
  });

  it("parses upstream section names and sort options", () => {
    expect(parseSection("names", "nft")).toBe("ens");
    expect(parseSection("land", "item")).toBe("wearables");
    expect(parseSection(null, "nft")).toBe("all");
    expect(sortOptions("nft", false).map((o) => o.value)).toEqual(["newest", "name"]);
    expect(sortOptions("nft", true)[0]!.value).toBe("recently_listed");
  });
});

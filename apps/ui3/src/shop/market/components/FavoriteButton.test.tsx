import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, expect, it } from "vitest";
import { setShopClient } from "../../api/shop";
import { BOOK } from "../../payment/fixtures";
import { StoryShell } from "../testHarness";
import { fakeShop } from "../testing";
import { shopFavorites } from "../store/favorites";
import FavoriteButton from "./FavoriteButton";

beforeEach(() => { localStorage.clear(); shopFavorites.reloadFor(null); });
afterEach(() => setShopClient(null));

it("saves the actual collection item and rolls a failed signed update back with an error", async () => {
  const key = `${BOOK.collection}-7`;
  let fail = true;
  const shop = fakeShop([
    { method: "GET", path: /^\/v1\/lists\/[^/]+\/picks$/, reply: () => ({ body: { ok: true, data: { results: [], total: 0 } } }) },
    { method: "GET", path: /^\/v3\/catalog\/items$/, reply: () => ({ body: { data: [{ id: key, contractAddress: BOOK.collection, itemId: "7", name: "Jacket", thumbnail: "", rarity: "rare", category: "wearable", chainId: 137, creator: BOOK.creator, isOnSale: false, available: 2 }], total: 1 } }) },
    { method: "POST", path: /^\/v1\/picks\//, reply: () => fail ? ({ status: 503, body: { ok: false } }) : ({ body: { ok: true, data: { pickedByUser: true } } }) },
  ]);
  setShopClient(shop.client);
  render(<MemoryRouter><StoryShell who="web3"><FavoriteButton contractAddress={BOOK.collection} itemId="7" name="Jacket" /></StoryShell></MemoryRouter>);
  await waitFor(() => expect(screen.getByRole("button", { name: "Save Jacket" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Save Jacket" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't update favorites");
  expect(screen.getByRole("button", { name: "Save Jacket" })).toHaveAttribute("aria-pressed", "false");
  expect(shopFavorites.getState().items).toEqual({});
  fail = false;
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Remove Jacket from favorites" })).toBeEnabled());
  expect(shopFavorites.getState().items[key]).toMatchObject({ key, isOnSale: false, priceCredits: null });
  expect(screen.queryByRole("alert")).toBeNull();
  expect(shop.requests.filter(r => r.method === "POST").every(r => r.signed)).toBe(true);
});

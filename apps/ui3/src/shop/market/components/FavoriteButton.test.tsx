import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setShopClient } from "../../api/shop";
import { invalidateShopQueries } from "../../discovery/query";
import { BOOK } from "../../payment/fixtures";
import { StoryShell } from "../testHarness";
import { fakeShop } from "../testing";
import { marketplaceOn } from "../testing-flags";
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

describe("who saved an item", () => {
  const key = `${BOOK.collection}-7`;
  const addr = (i: number) => `0x${i.toString(16).padStart(40, "0")}`;
  const NACHO = addr(1);

  // Each account links to its page, which is the Marketplace's: these tests turn it on; the last one leaves it off.
  let marketplaceOff: (() => void) | null = null;
  beforeEach(() => {
    marketplaceOff = marketplaceOn();
    invalidateShopQueries();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        const url = String(input instanceof Request ? input.url : input);
        const body = url.includes("/lambdas/profiles") ? [{ avatars: [{ ethAddress: NACHO, userId: NACHO, name: "nacho", hasClaimedName: true, avatar: {} }] }] : [];
        return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
      }),
    );
  });
  afterEach(() => {
    marketplaceOff?.();
    vi.unstubAllGlobals();
  });

  function heart(opts: { count: number; total?: number; failFirst?: boolean }) {
    const total = opts.total ?? opts.count;
    let fail = !!opts.failFirst;
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/lists\/[^/]+\/picks$/, reply: () => ({ body: { ok: true, data: { results: [], total: 0 } } }) },
      { method: "GET", path: /^\/v1\/picks\/stats$/, reply: () => ({ body: { ok: true, data: [{ itemId: key, count: opts.count, pickedByUser: false }] } }) },
      {
        method: "GET",
        path: /^\/v1\/picks\/(?!stats$)[^/]+$/,
        reply: ({ url }) => {
          if (fail) {
            fail = false;
            return { status: 500, body: { ok: false, message: "boom" } };
          }
          const offset = Number(url.searchParams.get("offset") ?? 0);
          const limit = Number(url.searchParams.get("limit") ?? 50);
          const results = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, i) => ({ userAddress: addr(offset + i + 1) }));
          return { body: { ok: true, data: { results, total } } };
        },
      },
    ]);
    setShopClient(shop.client);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter><StoryShell who="web3"><FavoriteButton contractAddress={BOOK.collection} itemId="7" name="Jacket" /></StoryShell></MemoryRouter>
      </QueryClientProvider>,
    );
    const reads = () => shop.requests.filter((r) => /^\/v1\/picks\/(?!stats$)[^/]+$/.test(r.url.pathname.replace(/^\/shop-api/, ""))).map((r) => [r.url.searchParams.get("limit"), r.url.searchParams.get("offset")]);
    return { shop, reads };
  }

  it("opens from the count, lists each account by name or address, and reads the next page on request", async () => {
    const { reads } = heart({ count: 51 });
    const count = await screen.findByRole("button", { name: "Saved by 51 people. See who saved Jacket" });
    expect(count).toHaveTextContent("51");
    expect(reads()).toEqual([]);
    fireEvent.click(count);
    const dialog = await screen.findByRole("dialog", { name: "Saved by" });
    await waitFor(() => expect(within(dialog).getAllByRole("link")).toHaveLength(50));
    expect(reads()).toEqual([["50", "0"]]);
    await waitFor(() => expect(within(dialog).getAllByRole("link")[0]).toHaveTextContent("nacho"));
    expect(within(dialog).getAllByRole("link")[0]).toHaveAttribute("href", `/shop/accounts/${NACHO}`);
    expect(within(dialog).getAllByRole("link")[1]).toHaveAttribute("title", addr(2));
    // A named account shows its short address under the name; an unnamed one shows it once.
    expect(within(dialog).getAllByRole("link")[0]).toHaveTextContent("nacho0x0000…0001");
    expect(within(dialog).getAllByRole("link")[1]!.textContent).toBe("0x0000…0002");
    fireEvent.click(within(dialog).getByTestId("load-more"));
    await waitFor(() => expect(within(dialog).getAllByRole("link")).toHaveLength(51));
    expect(reads()).toEqual([["50", "0"], ["50", "50"]]);
    expect(within(dialog).queryByTestId("load-more")).toBeNull();
  });

  it("keeps a zero count inside the heart: it saves, as before, and opens nothing", async () => {
    heart({ count: 0 });
    expect(await screen.findByTestId("fav-count")).toHaveTextContent("0");
    expect(screen.getByTestId("fav-count").closest("button")).toBe(screen.getByRole("button", { name: "Save Jacket" }));
    expect(screen.queryByRole("button", { name: /See who saved/ })).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("reads the list afresh each time it opens", async () => {
    const { reads } = heart({ count: 2 });
    const count = await screen.findByRole("button", { name: /See who saved Jacket/ });
    fireEvent.click(count);
    await waitFor(() => expect(within(screen.getByRole("dialog")).getAllByRole("link")).toHaveLength(2));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: /close/i }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    fireEvent.click(count);
    await waitFor(() => expect(reads()).toHaveLength(2));
  });

  it("says when the list could not be read and reads it again on retry", async () => {
    heart({ count: 2, failFirst: true });
    fireEvent.click(await screen.findByRole("button", { name: /See who saved Jacket/ }));
    const dialog = await screen.findByRole("dialog", { name: "Saved by" });
    fireEvent.click(await within(dialog).findByRole("button", { name: /try again|retry/i }));
    await waitFor(() => expect(within(dialog).getAllByRole("link")).toHaveLength(2));
  });

  it("lists the accounts as plain names, not links, without the Marketplace", async () => {
    marketplaceOff?.();
    marketplaceOff = marketplaceOn(false);
    heart({ count: 2 });
    fireEvent.click(await screen.findByRole("button", { name: /See who saved Jacket/ }));
    const dialog = await screen.findByRole("dialog", { name: "Saved by" });
    await waitFor(() => expect(within(dialog).getByTestId("saved-by-list").querySelectorAll("li")).toHaveLength(2));
    expect(within(dialog).queryAllByRole("link")).toHaveLength(0);
    await waitFor(() => expect(within(dialog).getByTitle(NACHO)).toHaveTextContent("nacho"));
  });
});


import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ShopClient } from "../../api/shop";
import { catalogClient, COLLECTION, CREATOR, ShopHarness, type CatalogScenario, type SessionKind } from "../fixtures";
import { invalidateShopQueries } from "../query";
import Items from "./Items";

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

function Where() {
  const { pathname, search } = useLocation();
  return <output data-testid="where">{pathname + search}</output>;
}

function items(entry = "/shop/items", opts: { scenario?: CatalogScenario; session?: SessionKind; client?: ShopClient } = {}) {
  return render(
    <ShopHarness path="/shop/*" entry={entry} scenario={opts.scenario} session={opts.session} client={opts.client}>
      <>
        <Items />
        <Where />
      </>
    </ShopHarness>,
  );
}

function lastUrl(client: ShopClient & { calls: { url: string }[] }): URL {
  const catalogue = client.calls.map((c) => new URL(c.url)).filter((u) => u.pathname.includes("/catalog/"));
  return catalogue.at(-1)!;
}

describe("the collectibles grid", () => {
  it("loading: skeleton cards and a count shimmer, never zero", () => {
    items("/shop/items", { scenario: "loading" });
    expect(screen.getAllByTestId("skeleton-card").length).toBe(15);
    expect(screen.getByTestId("browse-count-skeleton")).toBeTruthy();
  });

  it("On Sale reads the item-unified feed, one card per item, priced under the rule", async () => {
    const client = catalogClient();
    items("/shop/items", { client });
    const grid = await screen.findByTestId("grid");
    const cards = await within(grid).findAllByTestId("card");
    expect(cards).toHaveLength(5);
    expect(lastUrl(client).pathname).toBe("/shop-api/v3/catalog/unified");
    expect(lastUrl(client).searchParams.get("groupBy")).toBe("item");
    const byName = (name: string) => cards.filter((c) => within(c).queryByText(name));
    expect(within(byName("Unpriced Cape")[0]!).getByTestId("card-price-unavailable")).toHaveTextContent("Credits price unavailable");
    const resale = byName("Nacho Hat").find((c) => within(c).queryByTestId("card-resale"))!;
    expect(resale.textContent).not.toMatch(/Credit/);
    expect(within(byName("Genesis Boots")[0]!).getByTestId("card-price")).toHaveTextContent("12 MANA");
    expect(screen.getByTestId("browse-count")).toHaveTextContent("5 Items");
  });

  it("a mirror still on its first pass says the list may be incomplete, not sold out", async () => {
    const client = catalogClient("ok", { "/v1/trades-mirror": { ok: true, data: { enabled: true, completedAt: null, inProgress: true } } });
    items("/shop/items", { client });
    expect(await screen.findByTestId("index-notice")).toHaveTextContent("still being indexed (tradesMirror)");
  });

  it("a card links to the item or token page, never confusing the two", async () => {
    items();
    const cards = await screen.findAllByTestId("card");
    const hrefs = cards.map((c) => within(c).getByTestId("card-link").getAttribute("href"));
    expect(hrefs).toContain("/shop/item/0x000000000000000000000000000000000000c011/7");
    expect(hrefs).toContain("/shop/token/0x000000000000000000000000000000000000c011/42");
  });

  it("a search defaults to every item (the catalogue feed) sorted by relevance", async () => {
    const client = catalogClient();
    items("/shop/items?q=hat", { client });
    await screen.findAllByTestId("card");
    const url = lastUrl(client);
    expect(url.pathname).toBe("/shop-api/v3/catalog/items");
    expect(url.searchParams.get("search")).toBe("hat");
    expect(url.searchParams.get("sortBy")).toBe("relevance");
    expect(screen.getByTestId("browse-count")).toHaveTextContent("for “hat”");
  });

  it("filters and sort live in the URL, and a chip removes its own filter", async () => {
    const client = catalogClient();
    items("/shop/items?rarities=epic&smart=true", { client });
    await screen.findAllByTestId("card");
    expect(lastUrl(client).searchParams.get("rarity")).toBe("epic");
    expect(lastUrl(client).searchParams.get("isSmart")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Sort by" }));
    fireEvent.click(within(screen.getByTestId("sort-select")).getByRole("option", { name: /cheapest/i }));
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("sort=price-asc"));
    await waitFor(() => expect(lastUrl(client).searchParams.get("sortBy")).toBe("cheapest"));
    fireEvent.click(screen.getByRole("button", { name: "Remove Smart filter" }));
    await waitFor(() => expect(screen.getByTestId("where").textContent).not.toContain("smart"));
  });

  it("Not for Sale reads the catalogue as view cards with no purchase", async () => {
    const client = catalogClient();
    items("/shop/items?status=not_for_sale&priceMin=5", { client });
    const cards = await screen.findAllByTestId("card");
    expect(lastUrl(client).searchParams.get("isOnSale")).toBe("false");
    // Nothing not for sale has a price: no range control, none sent, no chip.
    expect(screen.queryAllByRole("slider")).toHaveLength(0);
    expect(lastUrl(client).searchParams.has("minPriceCredits")).toBe(false);
    expect(screen.queryByRole("button", { name: "Remove Price filter" })).toBeNull();
    for (const card of cards) {
      expect(card.dataset.mode).toBe("view");
      expect(within(card).queryByTestId("card-cart")).toBeNull();
    }
  });

  it("combines body shape and rarity on both feeds and restores them from the URL", async () => {
    const client = catalogClient();
    items("/shop/items?rarities=rare&bodyShape=female", { client });
    await screen.findAllByTestId("card");
    expect(screen.getByRole("combobox", { name: "Body shape" })).toHaveValue("female");
    expect(lastUrl(client).searchParams.get("rarity")).toBe("rare");
    expect(lastUrl(client).searchParams.get("wearableGender")).toBe("female");
    fireEvent.click(screen.getByRole("radio", { name: /^All$/ }));
    await waitFor(() => expect(lastUrl(client).pathname).toBe("/shop-api/v3/catalog/items"));
    expect(lastUrl(client).searchParams.get("rarity")).toBe("rare");
    expect(lastUrl(client).searchParams.get("wearableGender")).toBe("female");
    fireEvent.change(screen.getByRole("combobox", { name: "Body shape" }), { target: { value: "unisex" } });
    await waitFor(() => expect(lastUrl(client).searchParams.get("wearableGender")).toBe("unisex"));
    expect(screen.getByTestId("where")).toHaveTextContent("bodyShape=unisex");
  });

  it("offers the emote attributes only while browsing emotes", async () => {
    items("/shop/items?category=wearable");
    await screen.findAllByTestId("card");
    expect(screen.queryByRole("combobox", { name: "Play mode" })).toBeNull();
    expect(screen.queryByRole("switch", { name: "With sound" })).toBeNull();
    expect(screen.queryByRole("switch", { name: "With props" })).toBeNull();
  });

  it("sends the emote attributes on both feeds, restores them from the URL, and a chip removes one", async () => {
    const client = catalogClient();
    items("/shop/items?category=emote&emotePlayMode=loop&emoteSound=true", { client });
    await waitFor(() => expect(lastUrl(client).searchParams.get("emotePlayMode")).toBe("loop"));
    expect(lastUrl(client).pathname).toBe("/shop-api/v3/catalog/unified");
    expect(lastUrl(client).searchParams.get("emoteHasSound")).toBe("true");
    expect(screen.getByRole("combobox", { name: "Play mode" })).toHaveValue("loop");
    fireEvent.click(screen.getByRole("switch", { name: "With props" }));
    await waitFor(() => expect(lastUrl(client).searchParams.get("emoteHasGeometry")).toBe("true"));
    fireEvent.click(screen.getByRole("radio", { name: /^All$/ }));
    await waitFor(() => expect(lastUrl(client).pathname).toBe("/shop-api/v3/catalog/items"));
    expect(lastUrl(client).searchParams.get("emotePlayMode")).toBe("loop");
    expect(lastUrl(client).searchParams.get("emoteHasSound")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Remove With sound filter" }));
    await waitFor(() => expect(lastUrl(client).searchParams.has("emoteHasSound")).toBe(false));
    expect(screen.getByTestId("where").textContent).not.toContain("emoteSound");
  });

  it("filters primary offers before sorting, and hides sale type outside On Sale", async () => {
    const client = catalogClient();
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    fireEvent.change(screen.getByRole("combobox", { name: "Sale type" }), { target: { value: "primary" } });
    fireEvent.click(screen.getByRole("button", { name: "Sort by" }));
    fireEvent.click(within(screen.getByTestId("sort-select")).getByRole("option", { name: /cheapest/i }));
    await waitFor(() => expect(lastUrl(client).searchParams.get("sortBy")).toBe("cheapest"));
    expect(lastUrl(client).searchParams.get("listingType")).toBe("primary");
    fireEvent.click(screen.getByRole("radio", { name: /^Not for Sale$/ }));
    await waitFor(() => expect(lastUrl(client).pathname).toBe("/shop-api/v3/catalog/items"));
    expect(lastUrl(client).searchParams.has("listingType")).toBe(false);
    expect(screen.queryByRole("combobox", { name: "Sale type" })).toBeNull();
  });

  it("empty with a query says what was searched", async () => {
    items("/shop/items?q=nothing", { scenario: "empty" });
    const empty = await screen.findByTestId("browse-empty");
    expect(empty).toHaveTextContent("We couldn't find any results for “nothing”");
    expect(within(empty).getByText("nothing").tagName).toBe("B");
  });

  it("a failed read says so and offers a retry", async () => {
    items("/shop/items", { scenario: "error" });
    expect(await screen.findByTestId("browse-error")).toHaveTextContent("Couldn't load items");
    expect(screen.getByTestId("browse-count")).toHaveTextContent("Item count unavailable");
    expect(screen.queryByTestId("browse-empty")).toBeNull();
  });

  it("an index outage is explained instead of an empty grid", async () => {
    items("/shop/items", { scenario: "outage" });
    expect(await screen.findByTestId("index-notice")).toHaveTextContent("no complete pass yet");
    expect(screen.getByTestId("browse-count")).toHaveTextContent("Item count unavailable");
    expect(screen.queryByTestId("browse-empty")).toBeNull();
    expect(screen.queryByTestId("grid")).toBeNull();
  });

  it("the NAMEs category goes to the NAMEs page", async () => {
    items("/shop/items?category=names");
    await waitFor(() => expect(screen.getByTestId("where")).toHaveTextContent("/shop/names"));
  });

  it("add to cart keeps the listing, not a price", async () => {
    const { shopCart } = await import("../../cart/store");
    shopCart.clear();
    items("/shop/items", { session: "web2" });
    const cards = await screen.findAllByTestId("card");
    const hat = cards.find((c) => within(c).queryByText("Nacho Hat") && !within(c).queryByTestId("card-resale"))!;
    fireEvent.click(within(hat).getByTestId("card-cart"));
    await waitFor(() => expect(shopCart.getSnapshot().lines).toHaveLength(1));
    expect(shopCart.getSnapshot().lines[0]).toMatchObject({ listing: { tradeId: "trade-primary-7", acquisition: "trade" }, method: null, seen: null });
  });
});

describe("the creators and collection pickers", () => {
  const OTHER = "0x00000000000000000000000000000000000000b2";
  const feedUrl = (client: ShopClient & { calls: { url: string }[] }): URL =>
    client.calls.map((c) => new URL(c.url)).filter((u) => /\/v3\/catalog\/(unified|items)$/.test(u.pathname)).at(-1)!;
  const named = (address: string) => [{ address, name: address === CREATOR ? "Nacho" : "Nachito" }];
  const creatorSearch = (url: URL) => ({ data: url.searchParams.get("search") === "nacho" ? [...named(CREATOR), ...named(OTHER)] : [] });

  it("picks creators by name; both feeds ask for any of them, and a chip removes one", async () => {
    const client = catalogClient("ok", { "/v3/catalog/creators/search": creatorSearch });
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    const box = screen.getByRole("combobox", { name: "Creators" });
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "nacho" } });
    fireEvent.click(await screen.findByRole("option", { name: "Nacho" }));
    await waitFor(() => expect(feedUrl(client).searchParams.get("creator")).toBe(CREATOR));
    expect(screen.getByTestId("where")).toHaveTextContent(`creators=${CREATOR}`);
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "nacho" } });
    // A creator already picked is not offered again.
    const second = await screen.findByRole("option", { name: "Nachito" });
    expect(screen.queryByRole("option", { name: "Nacho" })).toBeNull();
    fireEvent.click(second);
    await waitFor(() => expect(feedUrl(client).searchParams.get("creator")).toBe(`${CREATOR},${OTHER}`));
    expect(feedUrl(client).pathname).toBe("/shop-api/v3/catalog/unified");
    fireEvent.click(screen.getByRole("radio", { name: /^All$/ }));
    await waitFor(() => expect(feedUrl(client).pathname).toBe("/shop-api/v3/catalog/items"));
    expect(feedUrl(client).searchParams.getAll("creator")).toEqual([CREATOR, OTHER]);
    fireEvent.click(screen.getByRole("button", { name: `Remove ${OTHER.slice(0, 6)}…${OTHER.slice(-4)} filter` }));
    await waitFor(() => expect(feedUrl(client).searchParams.getAll("creator")).toEqual([CREATOR]));
  });

  it("offers the top creators on focus, before anything is typed", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) =>
        String(input).includes("/lambdas/profiles")
          ? new Response(JSON.stringify([{ avatars: [{ ethAddress: CREATOR, userId: CREATOR, name: "Nacho", hasClaimedName: true, avatar: {} }] }]), { status: 200, headers: { "content-type": "application/json" } })
          : new Response("{}", { status: 200 }),
      ),
    );
    const client = catalogClient("ok", { "/v3/catalog/creators": { data: [{ id: CREATOR, sales: 9, totalSales: 9, collections: 1, items: 3 }] } });
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    fireEvent.focus(screen.getByRole("combobox", { name: "Creators" }));
    expect(await screen.findByText("Top creators")).toBeTruthy();
    fireEvent.click(await screen.findByRole("option", { name: "Nacho" }));
    await waitFor(() => expect(feedUrl(client).searchParams.get("creator")).toBe(CREATOR));
    expect(await screen.findByRole("button", { name: "Remove Nacho filter" })).toBeTruthy();
  });

  it("picks one collection by name, searching every collection while resales are in the grid", async () => {
    const client = catalogClient();
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    const box = screen.getByRole("combobox", { name: "Collection" });
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "nacho" } });
    const searched = () => client.calls.map((c) => new URL(c.url)).find((u) => u.pathname.endsWith("/v1/collections") && u.searchParams.get("search") === "nacho");
    await waitFor(() => expect(searched()).toBeTruthy());
    expect(searched()!.searchParams.has("isOnSale")).toBe(false);
    fireEvent.click(await screen.findByRole("option", { name: "Nacho Collection" }));
    await waitFor(() => expect(feedUrl(client).searchParams.get("contractAddress")).toBe(COLLECTION));
    expect(screen.getByTestId("where")).toHaveTextContent(`collection=${COLLECTION}`);
    expect(await screen.findByRole("button", { name: "Remove Nacho Collection filter" })).toBeTruthy();
    // The sidebar pill removes it too (the unfiltered grid is already cached, so the URL is what changes).
    fireEvent.click(within(screen.getByTestId("collection-filter")).getByRole("button", { name: "Remove Nacho Collection" }));
    await waitFor(() => expect(screen.getByTestId("where").textContent).not.toContain("collection="));
    expect(screen.queryByRole("button", { name: "Remove Nacho Collection filter" })).toBeNull();
  });

  it("searches only collections on primary sale while the grid shows creator listings only", async () => {
    const client = catalogClient();
    items("/shop/items?listingType=primary", { client });
    await screen.findAllByTestId("card");
    const box = screen.getByRole("combobox", { name: "Collection" });
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "nacho" } });
    const searched = () => client.calls.map((c) => new URL(c.url)).find((u) => u.pathname.endsWith("/v1/collections") && u.searchParams.get("search") === "nacho");
    await waitFor(() => expect(searched()).toBeTruthy());
    expect(searched()!.searchParams.get("isOnSale")).toBe("true");
  });

  it("Enter while the search is still waiting picks nothing from the previous list", async () => {
    const client = catalogClient("ok", { "/v3/catalog/creators/search": creatorSearch });
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    const box = screen.getByRole("combobox", { name: "Creators" });
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "nacho" } });
    await screen.findByRole("option", { name: "Nacho" });
    fireEvent.change(box, { target: { value: "nachoz" } });
    expect(within(screen.getByTestId("creator-filter")).queryAllByRole("option")).toHaveLength(0);
    fireEvent.keyDown(box, { key: "Enter" });
    expect(screen.getByTestId("where").textContent).not.toContain("creators=");
  });

  it("a collection and creators in the URL filter the first read; junk in them is ignored", async () => {
    const client = catalogClient();
    items(`/shop/items?collection=${COLLECTION}&creators=${CREATOR},nope`, { client });
    await screen.findAllByTestId("card");
    expect(feedUrl(client).searchParams.get("contractAddress")).toBe(COLLECTION);
    expect(feedUrl(client).searchParams.get("creator")).toBe(CREATOR);
    expect(await screen.findByRole("button", { name: "Remove Nacho Collection filter" })).toBeTruthy();
  });

  it("says when a search finds nothing", async () => {
    const client = catalogClient("ok", { "/v3/catalog/creators/search": creatorSearch });
    items("/shop/items", { client });
    await screen.findAllByTestId("card");
    const box = screen.getByRole("combobox", { name: "Creators" });
    fireEvent.focus(box);
    fireEvent.change(box, { target: { value: "zzz" } });
    expect(await screen.findByText("No results")).toBeTruthy();
  });
});

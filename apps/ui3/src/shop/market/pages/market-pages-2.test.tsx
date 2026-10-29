import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { BOOK } from "../../payment/fixtures";
import { LISTS, list, MANA } from "../fixtures";
import { DEFAULT_LIST_ID } from "../lib/favorites";
import { createFavoritesStore } from "../store/favorites";
import { fakeWallet, type FakeRoute } from "../testing";
import { mountShop } from "../testHarness";

const catalogRow = (itemId: string, over: Record<string, unknown> = {}) => ({
  id: `${BOOK.collection}-${itemId}`,
  itemId,
  name: `Item ${itemId}`,
  thumbnail: "",
  url: "",
  urn: "",
  category: "wearable",
  contractAddress: BOOK.collection,
  rarity: "rare",
  available: 5,
  isOnSale: true,
  creator: BOOK.creator,
  data: {},
  network: "MATIC",
  chainId: 137,
  price: MANA(2),
  priceCredits: 12,
  ...over,
});
const catalog: FakeRoute = {
  method: "GET",
  path: /^\/v3\/catalog\/items$/,
  reply: ({ url }) => ({ body: { data: url.searchParams.getAll("id").map((id) => catalogRow(id.split("-")[1]!, id.endsWith("-8") ? { priceCredits: null } : {})), total: 2 } }),
};
const picks = (ids: string[]): FakeRoute => ({ method: "GET", path: /^\/v1\/lists\/[^/]+\/picks$/, reply: () => ({ body: { ok: true, data: { results: ids.map((itemId) => ({ itemId, createdAt: 1 })), total: ids.length } } }) });

beforeEach(() => localStorage.clear());

describe("My favorites", () => {
  it("signed in: the default list's items, priced in credits, and an explicit outage for a missing quote (never MANA)", async () => {
    const { shopFavorites } = await import("../store/favorites");
    shopFavorites.reloadFor(null);
    mountShop({ path: "/shop/my-favorites", who: "web2", routes: [picks([`${BOOK.collection}-7`, `${BOOK.collection}-8`]), catalog] });
    const cards = await screen.findAllByTestId("shop-item-card");
    expect(cards).toHaveLength(2);
    expect(within(cards[0]!).getByTestId("shop-item-price")).toHaveTextContent("12");
    expect(within(cards[1]!).getByTestId("shop-credits-unavailable")).toHaveTextContent("Credits price unavailable");
    expect(document.body.textContent).not.toMatch(/MANA/);
  });

  it("lists tab: only the signer's own lists, with create", async () => {
    const m = mountShop({ path: "/shop/my-favorites?tab=lists", who: "web3", routes: [picks([]), catalog, { method: "GET", path: /^\/v1\/lists$/, reply: () => ({ body: { ok: true, data: { results: LISTS, total: 3 } } }) }] });
    const rows = await screen.findAllByTestId("shop-list-row");
    expect(rows.map((r) => within(r).getByRole("link").textContent)).toEqual(["Winter fits", "Party pack"]);
    expect(within(rows[0]!).getByText("Private")).toBeInTheDocument();
    expect(within(rows[1]!).getByText("Shared")).toBeInTheDocument();
    expect(m.shop.requests.find((r) => r.url.pathname === "/shop-api/v1/lists")?.signed).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Create a list" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create list" }));
    expect(await screen.findByText("Give the list a name.")).toBeInTheDocument();
  });

  it("saves an item into several lists with one bulk pick of the difference", async () => {
    const { shopFavorites } = await import("../store/favorites");
    shopFavorites.reloadFor(null);
    const key = `${BOOK.collection}-7`;
    const m = mountShop({
      path: "/shop/my-favorites",
      who: "web3",
      routes: [
        picks([key]),
        catalog,
        { method: "GET", path: /^\/v1\/lists$/, reply: () => ({ body: { ok: true, data: { results: [{ ...LISTS[0]!, isItemInList: true }, { ...LISTS[1]!, isItemInList: false }, { ...LISTS[2]!, isItemInList: false }], total: 3 } } }) },
        { method: "POST", path: /^\/v1\/picks\//, reply: () => ({ body: { ok: true, data: { pickedByUser: true } } }) },
      ],
    });
    fireEvent.click(await screen.findByRole("button", { name: "Add to a list" }));
    const picker = await screen.findByTestId("shop-save-to-lists");
    fireEvent.click(await within(picker).findByRole("checkbox", { name: /Winter fits/ }));
    fireEvent.click(within(picker).getByRole("checkbox", { name: /Favorites/ }));
    fireEvent.click(within(picker).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(m.shop.requests.find((r) => r.method === "POST")?.body).toEqual({ pickedFor: ["list-private"], unpickedFrom: [DEFAULT_LIST_ID] }));
    expect(m.shop.requests.find((r) => r.url.pathname === "/shop-api/v1/lists")?.url.searchParams.get("itemId")).toBe(key);
  });

  it("signed out: favorites from this browser, and a toggle stays local", () => {
    const store = createFavoritesStore();
    const item = { id: "x", key: `${BOOK.collection}-7`, name: "Jacket", thumbnail: "", contractAddress: BOOK.collection, itemId: "7", rarity: "rare", category: "wearable", chainId: 137, creator: BOOK.creator, isOnSale: true, available: 1, priceCredits: 5 };
    store.reloadFor(null);
    store.toggle(item);
    expect(JSON.parse(localStorage.getItem("shop-favorites")!)).toHaveProperty(`${BOOK.collection}-7`);
    store.toggle(item);
    expect(store.getState().items).toEqual({});
  });
});

describe("One list: a private list is never rendered to another account", () => {
  const privateList = list({ id: "list-private", name: "Winter fits", description: "Only for me", isPrivate: true, userAddress: BOOK.buyer });

  it("a stranger gets 'private or does not exist' and its name and picks are never shown or requested", async () => {
    const m = mountShop({
      path: "/shop/my-favorites/list-private",
      who: "web3",
      wallet: fakeWallet(),
      routes: [{ method: "GET", path: /^\/v1\/lists\/list-private$/, reply: () => ({ body: { ok: true, data: privateList } }) }, picks([`${BOOK.collection}-7`]), catalog],
    });
    expect(await screen.findByTestId("shop-list-unavailable")).toHaveTextContent("This list is private or doesn't exist");
    expect(screen.queryByText("Winter fits")).toBeNull();
    expect(screen.queryByText("Only for me")).toBeNull();
    expect(m.shop.requests.some((r) => r.url.pathname.endsWith("/picks"))).toBe(false);
  });

  it("signed out, upstream's sign-in page is drawn in place (ListPage.tsx:105 redirects to it)", async () => {
    mountShop({ path: "/shop/my-favorites/list-private", who: "signedOut", routes: [{ method: "GET", path: /^\/v1\/lists\/list-private$/, reply: () => ({ status: 401, body: { ok: false, message: "unauthorized" } }) }] });
    expect(await screen.findByTestId("shop-list-signin")).toHaveTextContent("Get Started");
    expect(screen.queryByTestId("shop-list-unavailable")).toBeNull();
  });

  it("the owner sees it, edits it, and can remove items", async () => {
    const wallet = fakeWallet();
    const own = { ...privateList, userAddress: wallet.address };
    const m = mountShop({
      path: "/shop/my-favorites/list-private",
      who: "web3",
      wallet,
      routes: [
        { method: "GET", path: /^\/v1\/lists\/list-private$/, reply: () => ({ body: { ok: true, data: own } }) },
        picks([`${BOOK.collection}-7`]),
        catalog,
        { method: "POST", path: /^\/v1\/picks\//, reply: () => ({ body: { ok: true, data: { pickedByUser: false } } }) },
        { method: "PUT", path: /^\/v1\/lists\/list-private$/, reply: ({ body }) => ({ body: { ok: true, data: { ...own, name: (body as { name: string }).name } } }) },
      ],
    });
    expect(await screen.findByRole("heading", { name: "Winter fits" })).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Remove from list" }));
    await waitFor(() => expect(m.shop.requests.some((r) => r.method === "POST" && (r.body as { unpickedFrom?: string[] })?.unpickedFrom?.[0] === "list-private")).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Winter 2026" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(m.shop.requests.find((r) => r.method === "PUT")?.body).toEqual({ name: "Winter 2026", private: true, description: "Only for me" }));
  });

  it("a shared list is readable by anyone, without edit controls", async () => {
    mountShop({
      path: "/shop/my-favorites/list-shared",
      who: "signedOut",
      routes: [{ method: "GET", path: /^\/v1\/lists\/list-shared$/, reply: () => ({ body: { ok: true, data: LISTS[2] } }) }, picks([`${BOOK.collection}-7`]), catalog],
    });
    expect(await screen.findByRole("heading", { name: "Party pack" })).toBeInTheDocument();
    expect(await screen.findAllByTestId("shop-item-card")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();
    expect(screen.getByRole("button", { name: "Copy link" })).toBeInTheDocument();
  });

  it("the default list is never shown as someone else's", async () => {
    const m = mountShop({ path: `/shop/my-favorites/${DEFAULT_LIST_ID}`, who: "signedOut", routes: [{ method: "GET", path: /^\/v1\/lists\/[^/]+$/, reply: () => ({ body: { ok: true, data: LISTS[0] } }) }] });
    expect(await screen.findByTestId("shop-list-signin")).toBeInTheDocument();
    expect(m.shop.requests.some((r) => r.url.pathname.endsWith("/picks"))).toBe(false);
  });
});

describe("loading states", () => {
  it("each page says what it is loading while its reads are in flight", async () => {
    const { PENDING } = await import("../testing");
    const pending = (path: RegExp, method = "GET"): FakeRoute => ({ method, path, reply: PENDING });
    let view = mountShop({ path: "/shop/my-items", who: "web3", routes: [pending(/^\/v1\/nfts$/), pending(/^\/v3\/catalog\/importable$/)] });
    expect(await screen.findByText("Loading your wearables…")).toBeInTheDocument();
    view.unmount();
    view = mountShop({ path: "/shop/rankings", who: "signedOut", routes: [pending(/^\/v1\/volume\//), pending(/^\/v1\/rankings\//)] });
    expect(await screen.findByText("Loading rankings…")).toBeInTheDocument();
    view.unmount();
    view = mountShop({ path: "/shop/my-favorites/list-shared", who: "signedOut", routes: [pending(/^\/v1\/lists\/list-shared$/)] });
    expect(await screen.findByText("Loading the list…")).toBeInTheDocument();
    view.unmount();
    mountShop({ path: `/shop/token/${BOOK.collection}/1/bids`, who: "signedOut", routes: [pending(/^\/v1\/nfts$/)] });
    expect(await screen.findByText("Loading the item…")).toBeInTheDocument();
  });
});

import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi, onTestFinished } from "vitest";

import salesEmptyArt from "../../components/empty-art/sales-empty.svg?url";
import { CHAIN } from "@shop/model/chain";
import { BOOK } from "@shop/payment/fixtures";
import { resetArchivedBidsForTests } from "../store/archivedBids";
import { FIXTURE_NOW, LEGACY_BID_ROW, MADE_BIDS, MANA, nftRow, OWNED_NAMES, OWNED_WEARABLES, RECEIVED_BIDS } from "../fixtures";
import { COLLECTION_V2_ABI, ERC20_ABI, ERC721_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, manaToken } from "@shop/market/lib/contracts";
import { fakeChain, fakeWallet, type FakeRoute } from "../testing";
import { mountShop } from "../testHarness";
import { primeRemoteFlags } from "../../discovery/lib/remoteFlags";

const MARKET = latestOffChainMarketplace(CHAIN.POLYGON).address;
const MANA_POLYGON = manaToken(CHAIN.POLYGON).address;
let restoreChain: (() => void) | null = null;
const originalFetch = globalThis.fetch;
let relayed: Hex[] = [];

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"], now: FIXTURE_NOW, shouldAdvanceTime: true });
  localStorage.clear();
  resetArchivedBidsForTests();
  relayed = [];
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    if (String(url).endsWith("/transactions")) {
      relayed.push((JSON.parse(String(init?.body)) as { transactionData: { params: [string, Hex] } }).transactionData.params[1]);
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"e".repeat(64)}` }));
    }
    throw new Error(`unexpected fetch ${String(url)}`);
  }) as typeof fetch;
});

afterEach(() => {
  vi.useRealTimers();
  restoreChain?.();
  restoreChain = null;
  globalThis.fetch = originalFetch;
});

function chain(overrides: { approved?: boolean; balance?: bigint; allowance?: bigint } = {}) {
  restoreChain = fakeChain({
    contracts: {
      [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
      [BOOK.collection]: { abi: [...ERC721_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "isApprovedForAll" || fn === "globalMinters" ? (overrides.approved ?? true) : 0n) },
      [MANA_POLYGON]: {
        abi: [...ERC20_ABI, ...COLLECTION_V2_ABI.filter((f) => f.name === "getNonce")],
        answer: (fn) => (fn === "balanceOf" ? (overrides.balance ?? 10n ** 20n) : fn === "allowance" ? (overrides.allowance ?? 0n) : 0n),
      },
    },
  });
}

const nfts = (rows = OWNED_WEARABLES): FakeRoute => ({
  method: "GET",
  path: /^\/v1\/nfts$/,
  reply: ({ url }) => {
    const tokenId = url.searchParams.get("tokenId");
    const cat = url.searchParams.get("category");
    const all = [...rows, ...OWNED_NAMES];
    const data = tokenId ? all.filter((r) => r.nft.tokenId === tokenId) : all.filter((r) => !cat || r.nft.category === cat);
    return { body: { data, total: data.length } };
  },
});
const importable = (creations = 0): FakeRoute => ({
  method: "GET",
  path: /^\/v3\/catalog\/importable$/,
  reply: () => ({ body: { data: Array.from({ length: creations }, (_, i) => ({ oldTradeId: `o${i}`, listingType: "primary", contractAddress: BOOK.collection, itemId: String(i), tokenId: null, name: "x", thumbnail: "", rarity: "rare", category: "wearable", wearableCategory: null, manaWei: MANA(1), available: 1, network: "MATIC", chainId: 137 })) } }),
});
const quotes: FakeRoute = { method: "POST", path: /^\/v1\/credit-quotes$/, reply: () => ({ body: { items: [], amounts: ["10"] } }) };

describe("My items", () => {
  it("a link to LAND waits while the Marketplace has not answered, rather than mounting the wearables", async () => {
    primeRemoteFlags(null);
    const real = globalThis.fetch;
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL, init?: RequestInit) => (String(input).endsWith("/dapps.json") ? new Promise<Response>(() => {}) : real(input, init))));
    onTestFinished(() => {
      vi.unstubAllGlobals();
    });
    const m = mountShop({ path: "/shop/my-items?section=land", who: "web3", routes: [nfts(), importable(0)], marketplace: null });
    expect(await screen.findByText("Loading your items…")).toBeInTheDocument();
    expect(m.shop.requests.some((r) => r.url.pathname.endsWith("/v1/nfts"))).toBe(false);
  });

  it("signed out: the sign-in gate, no request for anyone's items", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "signedOut", routes: [nfts()] });
    expect(await screen.findByTestId("shop-signin-gate")).toBeInTheDocument();
    expect(m.shop.requests).toHaveLength(0);
  });

  it("lists owned wearables with their MANA listing price, never a credit price, and the manage actions", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "web3", routes: [nfts(), importable(0)] });
    const tiles = await screen.findAllByTestId("shop-asset-tile");
    expect(tiles).toHaveLength(3);
    const listed = within(tiles[0]!);
    expect(listed.getByTestId("shop-asset-price")).toHaveTextContent("12.5");
    expect(listed.getByTestId("shop-asset-price")).toHaveTextContent("MANA");
    expect(document.body.textContent).not.toMatch(/credit/i);
    expect(listed.getByRole("link", { name: "Update price" })).toHaveAttribute("href", `/shop/token/${BOOK.collection}/105312/sell`);
    expect(listed.getByRole("link", { name: "Remove from sale" })).toHaveAttribute("href", `/shop/token/${BOOK.collection}/105312/cancel`);
    fireEvent.click(screen.getByRole("button", { name: "On sale" }));
    await waitFor(() => expect(m.shop.requests.some((r) => r.url.searchParams.get("isOnSale") === "true")).toBe(true));
  });

  it("without the Marketplace (a web2 session, or the rollout off), owned items carry no manage actions and no LAND or Bids", async () => {
    const web2 = mountShop({ path: "/shop/my-items?section=land", who: "web2", routes: [nfts(), importable(0)] });
    const tiles = await screen.findAllByTestId("shop-asset-tile");
    expect(within(tiles[0]!).queryByRole("link", { name: "Update price" })).toBeNull();
    expect(screen.queryByRole("button", { name: "LAND" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Bids" })).toBeNull();
    web2.unmount();
    mountShop({ path: "/shop/my-items", who: "web3", marketplace: false, routes: [nfts(), importable(0)] });
    expect(within((await screen.findAllByTestId("shop-asset-tile"))[0]!).queryByRole("link", { name: "Transfer" })).toBeNull();
  });

  it("the old Marketplace section names land on their Shop section; empty, error and outage each explain themselves", async () => {
    const m = mountShop({ path: "/shop/my-items?section=ens", who: "web3", routes: [nfts([]), importable(0)] });
    // The section nav is the Collectibles category nav, as upstream's: NAMEs is pressed.
    expect(await screen.findByRole("button", { name: "NAMEs" })).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findAllByTestId("shop-asset-tile")).toHaveLength(1);
    m.unmount();
    mountShop({ path: "/shop/my-items?section=emotes", who: "web3", routes: [nfts([]), importable(0)] });
    const empty = await screen.findByTestId("owned-empty");
    expect(empty).toHaveTextContent("Nothing here... yet!");
    expect(within(empty).getByRole("link", { name: "Explore Shop" })).toHaveAttribute("href", "/shop/items");
    expect(screen.getByTestId("browse-count")).toHaveTextContent("0 Items");
  });

  it("Wearables open to their sub-categories, as upstream's My Assets: one narrows the owned list and shows as a chip", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "web3", routes: [nfts(), importable(0)] });
    await screen.findAllByTestId("shop-asset-tile");
    expect(within(screen.getByTestId("my-items-sidebar")).getByRole("heading", { name: "Items" })).toBeInTheDocument();
    const last = () => m.shop.requests.filter((r) => r.url.pathname.endsWith("/v1/nfts")).at(-1)!.url.searchParams;
    // A group is its flag, as the catalog's: never a list of categories the server would narrow to the first.
    fireEvent.click(await screen.findByRole("button", { name: "Head" }));
    await waitFor(() => expect(last().get("isWearableHead")).toBe("true"));
    expect(last().getAll("wearableCategory")).toEqual([]);
    expect(within(screen.getByTestId("filter-chips")).getByRole("button", { name: /Head/ })).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Accessories" }));
    await waitFor(() => expect(last().get("isWearableAccessory")).toBe("true"));
    expect(last().get("isWearableHead")).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: "Upper Body" }));
    await waitFor(() => expect(last().getAll("wearableCategory")).toEqual(["upper_body"]));
    // The chip clears it, and so does Clear all.
    fireEvent.click(within(screen.getByTestId("filter-chips")).getByRole("button", { name: /Upper Body/ }));
    await waitFor(() => expect(last().getAll("wearableCategory")).toEqual([]));
    fireEvent.click(await screen.findByRole("button", { name: "Head" }));
    await waitFor(() => expect(last().get("isWearableHead")).toBe("true"));
    fireEvent.click(within(screen.getByTestId("filter-chips").parentElement!).getByRole("button", { name: /Clear all/i }));
    await waitFor(() => expect(last().get("isWearableHead")).toBeNull());
  });

  it("a sub-category stays with its section: a section change from outside the sidebar drops it, and Back does not bring it back", async () => {
    const m = mountShop({ path: "/shop/my-items?section=emotes", who: "web3", routes: [nfts(), importable(0)] });
    const last = () => m.shop.requests.filter((r) => r.url.pathname.endsWith("/v1/nfts")).at(-1)!.url.searchParams;
    fireEvent.click(await screen.findByRole("button", { name: "Dance" }));
    await waitFor(() => expect(last().getAll("emoteCategory")).toEqual(["dance"]));
    // The header's My Assets tab: /shop/my-items with no section, i.e. Wearables.
    await act(async () => void (await m.router.navigate("/shop/my-items")));
    await waitFor(() => expect(last().get("category")).toBe("wearable"));
    expect(last().getAll("wearableCategory")).toEqual([]);
    expect(last().get("emoteCategory")).toBeNull();
    expect(screen.queryByTestId("filter-chips")).toBeNull();
    await act(async () => void (await m.router.navigate(-1)));
    await waitFor(() => expect(last().get("category")).toBe("emote"));
    expect(last().getAll("emoteCategory")).toEqual([]);
    expect(screen.queryByTestId("filter-chips")).toBeNull();
  });

  it("from LAND, Wearables opens its group and a sub-category narrows the wearables", async () => {
    const m = mountShop({ path: "/shop/my-items?section=land", who: "web3", routes: [nfts(), importable(0)] });
    expect(await screen.findByRole("button", { name: "LAND" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(await screen.findByRole("button", { name: "Wearables" }));
    fireEvent.click(await screen.findByRole("button", { name: "Head" }));
    await waitFor(() => expect(m.shop.requests.some((r) => r.url.searchParams.get("category") === "wearable" && r.url.searchParams.get("isWearableHead") === "true")).toBe(true));
  });

  it("the toolbar's Clear all clears the chips and keeps the search; Not for sale is asked of the server", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "web3", routes: [nfts(), importable(0)] });
    const last = () => m.shop.requests.filter((r) => r.url.pathname.endsWith("/v1/nfts")).at(-1)!.url.searchParams;
    await screen.findAllByTestId("shop-asset-tile");
    fireEvent.click(screen.getByRole("button", { name: "Not for sale" }));
    await waitFor(() => expect(last().get("isOnSale")).toBe("false"));
    fireEvent.change(screen.getByRole("searchbox", { name: "Search your items" }), { target: { value: "hat" } });
    await waitFor(() => expect(last().get("search")).toBe("hat"));
    fireEvent.click(within(screen.getByTestId("filter-chips").parentElement!).getByRole("button", { name: /Clear all/i }));
    await waitFor(() => expect(last().get("isOnSale")).toBeNull());
    expect(last().get("search")).toBe("hat");
    expect(screen.getByRole("searchbox", { name: "Search your items" })).toHaveValue("hat");
  });

  it("on a phone, Wearables opened from LAND keeps the filters drawer open to choose a sub-category", async () => {
    const m = mountShop({ path: "/shop/my-items?section=land", who: "web3", routes: [nfts(), importable(0)] });
    fireEvent.click(await screen.findByTestId("open-filters"));
    expect(screen.getByTestId("browse-sidebar")).toHaveAttribute("data-open", "true");
    fireEvent.click(screen.getByRole("button", { name: "Wearables" }));
    await waitFor(() => expect(m.router.state.location.search).toContain("section=wearables"));
    expect(screen.getByTestId("browse-sidebar")).toHaveAttribute("data-open", "true");
    fireEvent.click(screen.getByRole("button", { name: "Head" }));
    await waitFor(() => expect(m.shop.requests.some((r) => r.url.searchParams.get("isWearableHead") === "true")).toBe(true));
    // A section without sub-categories closes the drawer on the way.
    fireEvent.click(screen.getByRole("button", { name: "NAMEs" }));
    await waitFor(() => expect(m.router.state.location.search).toContain("section=names"));
    expect(screen.getByTestId("browse-sidebar")).not.toHaveAttribute("data-open");
  });

  it("an empty list says what is empty: nothing owned, nothing matching the filters, nothing on sale", async () => {
    const none: FakeRoute = { method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ body: { data: [], total: 0 } }) };
    const m = mountShop({ path: "/shop/my-items?section=names", who: "web3", routes: [none, importable(0)] });
    expect(await screen.findByTestId("owned-empty")).toHaveTextContent("No NAMEs yet");
    // A status with no match is no match, not "no NAMEs".
    fireEvent.click(screen.getByRole("button", { name: "On sale" }));
    await waitFor(() => expect(screen.getByTestId("owned-empty")).toHaveTextContent("Nothing matches these filters"));
    expect(screen.getByTestId("owned-empty").querySelector("img")!.getAttribute("src")).toBe(salesEmptyArt);
    fireEvent.click(within(screen.getByTestId("owned-empty")).getByRole("button", { name: "Clear all" }));
    await waitFor(() => expect(screen.getByTestId("owned-empty")).toHaveTextContent("No NAMEs yet"));
    m.unmount();
    // A search with no match offers to clear it, never to explore the Shop.
    const w = mountShop({ path: "/shop/my-items", who: "web3", routes: [none, importable(0)] });
    expect(await screen.findByTestId("owned-empty")).toHaveTextContent("Nothing here... yet!");
    fireEvent.change(screen.getByRole("searchbox", { name: "Search your items" }), { target: { value: "zzz" } });
    await waitFor(() => expect(screen.getByTestId("owned-empty")).toHaveTextContent("Nothing matches these filters"));
    expect(within(screen.getByTestId("owned-empty")).queryByRole("link", { name: "Explore Shop" })).toBeNull();
    fireEvent.click(within(screen.getByTestId("owned-empty")).getByRole("button", { name: "Clear all" }));
    await waitFor(() => expect(screen.getByTestId("owned-empty")).toHaveTextContent("Nothing here... yet!"));
    expect(screen.getByRole("searchbox", { name: "Search your items" })).toHaveValue("");
    expect(w.shop.requests.filter((r) => r.url.pathname.endsWith("/v1/nfts")).at(-1)!.url.searchParams.get("search")).toBeNull();
  });

  it("an index that has not caught up is an outage with its reason, not an empty page", async () => {
    mountShop({
      path: "/shop/my-items",
      who: "web3",
      routes: [{ method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "no pass yet", source: "nft-indexer" } }) }, importable(0)],
    });
    expect(await screen.findByTestId("shop-outage")).toHaveTextContent("the NFT index is still catching up");
    expect(screen.queryByTestId("owned-empty")).toBeNull();
    // No count over an outage: "0 Items" would read as an empty backpack.
    expect(screen.getByTestId("browse-count")).not.toHaveTextContent("0 Items");
  });

  it("with the store dashboard off, creations stay reachable from My items (as upstream)", async () => {
    const { setShopCapabilities } = await import("../../seller/lib/featureFlags");
    setShopCapabilities({ myStore: false });
    try {
      mountShop({ path: "/shop/my-items?section=creations", who: "web3", routes: [nfts(), importable(0)] });
      expect(await screen.findByRole("button", { name: "My Creations" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.queryByRole("link", { name: "My store" })).toBeNull();
      expect(await screen.findByRole("group", { name: "Period" })).toBeInTheDocument();
    } finally {
      act(() => setShopCapabilities(null));
    }
  });

  it("nudges a creator with MANA-only creations toward the import tool", async () => {
    mountShop({ path: "/shop/my-items", who: "web3", routes: [nfts(), importable(2), quotes] });
    const nudge = await screen.findByTestId("shop-import-nudge");
    expect(nudge).toHaveTextContent("2 of your creations");
    expect(within(nudge).getByRole("link")).toHaveAttribute("href", "/shop/activity?section=listings");
  });

  it("bids: received ones can be reviewed or archived locally; made ones cancelled", async () => {
    const wallet = fakeWallet();
    const made = MADE_BIDS.map((b) => ({ ...b, bidder: wallet.address }));
    const received = RECEIVED_BIDS.map((b) => ({ ...b, seller: wallet.address }));
    mountShop({
      path: "/shop/my-items?section=bids",
      who: "web3",
      wallet,
      routes: [
        importable(0),
        { method: "GET", path: /^\/v1\/bids$/, reply: ({ url }) => ({ body: { ok: true, data: { results: url.searchParams.get("bidder") ? made : received, total: 2, page: 0, pages: 1, limit: 1000 } } }) },
      ],
    });
    const rows = await screen.findAllByTestId("shop-bid-row");
    expect(rows).toHaveLength(2);
    expect(within(rows[0]!).getByText("8")).toBeInTheDocument();
    fireEvent.click(within(rows[0]!).getByRole("button", { name: "Archive" }));
    await waitFor(() => expect(screen.getAllByTestId("shop-bid-row")).toHaveLength(1));
    fireEvent.click(screen.getByRole("tab", { name: "Made" }));
    expect(await screen.findByRole("button", { name: "Cancel bid" })).toBeInTheDocument();
  });
});

describe("old links naming a bid", () => {
  it("My items with ?bid= opens that received bid's review on its token's bids page", async () => {
    const wallet = fakeWallet();
    chain();
    const received = RECEIVED_BIDS.map((b) => ({ ...b, seller: wallet.address }));
    const token = nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address });
    mountShop({
      path: "/shop/my-items?section=bids&bid=b2",
      who: "web3",
      wallet,
      routes: [
        importable(0),
        nfts([token]),
        { method: "GET", path: /^\/v1\/bids$/, reply: ({ url }) => ({ body: { ok: true, data: { results: url.searchParams.get("bidder") ? [] : received, total: 2, page: 0, pages: 1, limit: 1000 } } }) },
      ],
    });
    const confirm = await screen.findByTestId("shop-accept-confirm");
    expect(confirm).toHaveTextContent("receive");
    expect(confirm).toHaveTextContent("9.75");
  });

  it("a bid that is no longer open says so on My items", async () => {
    const wallet = fakeWallet();
    mountShop({
      path: "/shop/my-items?section=bids&bid=gone",
      who: "web3",
      wallet,
      routes: [importable(0), { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: [], total: 0, page: 0, pages: 0, limit: 1000 } } }) }],
    });
    expect(await screen.findByTestId("shop-bid-from-link")).toHaveTextContent("The bid from your link is no longer open.");
  });
});

describe("Transfer", () => {
  const owned = (owner: string) => nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner });

  it("the owner transfers a Polygon wearable gasless, after an explicit confirmation", async () => {
    const wallet = fakeWallet();
    chain();
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web3", wallet, routes: [nfts([owned(wallet.address)])] });
    expect(await screen.findByRole("heading", { name: "Transfer Wearable" })).toBeInTheDocument();
    const input = await screen.findByPlaceholderText("0x...");
    fireEvent.change(input, { target: { value: wallet.address } });
    expect(screen.getByText(/your own address/)).toBeInTheDocument();
    fireEvent.change(input, { target: { value: BOOK.reseller } });
    fireEvent.click(screen.getByRole("button", { name: "Transfer" }));
    expect(relayed).toHaveLength(0);
    expect(await screen.findByTestId("shop-transfer-confirm")).toHaveTextContent(BOOK.reseller);
    fireEvent.click(screen.getByRole("button", { name: "Proceed" }));
    expect(await screen.findByTestId("shop-transfer-done")).toBeInTheDocument();
    const inner = decodeFunctionData({ abi: COLLECTION_V2_ABI, data: relayed[0]! }).args![1] as Hex;
    expect(decodeFunctionData({ abi: ERC721_ABI, data: inner }).functionName).toBe("transferFrom");
  });

  it("not the owner: no form; a web2 owner: told to connect a wallet", async () => {
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web3", routes: [nfts([owned(BOOK.reseller)])] });
    expect(await screen.findByTestId("shop-not-owner")).toHaveTextContent("You are not the owner of Neon Jacket.");
    expect(screen.getByRole("button", { name: "Transfer" })).toBeDisabled();
  });

  it("on sale: upstream's red notice, no transfer until the listing is removed", async () => {
    const wallet = fakeWallet();
    const listed = nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address, onSale: { price: MANA(3).toString() } });
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web3", wallet, routes: [nfts([listed])] });
    expect(await screen.findByTestId("shop-transfer-on-sale")).toHaveTextContent("Neon Jacket can't be transferred because it's on sale.");
    expect(screen.getByPlaceholderText("0x...")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Transfer" })).toBeDisabled();
  });

  it("signed out, the transfer page is not drawn: the visitor goes to the overview, with no read made", async () => {
    const m = mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "signedOut", routes: [nfts([owned(BOOK.reseller)])] });
    await waitFor(() => expect(m.router.state.location.pathname).toBe("/shop/overview"));
    expect(m.shop.requests.some((r) => r.url.pathname === "/v1/nfts")).toBe(false);
  });

  it("a wallet moved to another account never signs for the session: the switch is explicit", async () => {
    const { testIdentity } = await import("../testHarness");
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web3", host: { identity: testIdentity(BOOK.buyer) }, routes: [nfts([owned(BOOK.buyer)])] });
    const notice = await screen.findByTestId("shop-account-mismatch");
    expect(notice).toHaveTextContent("you are signed in as 0x0000…0b0b");
    expect(screen.queryByPlaceholderText("0x...")).toBeNull();
  });

  it("a web2 session owning the token never reaches the transfer page: transfers are the Marketplace's", async () => {
    const wallet = fakeWallet();
    const m = mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web2", wallet, routes: [nfts([owned(wallet.address)])] });
    await waitFor(() => expect(m.router.state.location.pathname).toBe("/shop/overview"));
  });
});

describe("Place a bid: MANA only", () => {
  const listed = nftRow({ tokenId: "77", itemId: "7", name: "Glow Visor", owner: BOOK.reseller });

  it("bids are a web3 session's: signed out or web2, a token or an item bid goes to the overview", async () => {
    for (const [who, path] of [
      ["signedOut", `/shop/token/${BOOK.collection}/77/bid`],
      ["signedOut", `/shop/item/${BOOK.collection}/7/bid`],
      ["web2", `/shop/token/${BOOK.collection}/77/bid`],
    ] as const) {
      const m = mountShop({ path, who, routes: [nfts([listed])] });
      await waitFor(() => expect(m.router.state.location.pathname).toBe("/shop/overview"));
      m.unmount();
    }
  });

  it("web3: places a bid (MANA approval relayed, trade signed and posted) with the phases shown", async () => {
    const wallet = fakeWallet();
    chain();
    const posted: unknown[] = [];
    mountShop({
      path: `/shop/token/${BOOK.collection}/77/bid`,
      who: "web3",
      wallet,
      routes: [
        nfts([listed]),
        { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: [], total: 0, page: 0, pages: 0, limit: 1000 } } }) },
        { method: "POST", path: /^\/v1\/trades$/, reply: ({ body }) => (posted.push(body), { status: 201, body: { ok: true, data: { ...(body as object), id: "bid-1", contract: MARKET } } }) },
      ],
    });
    fireEvent.change(await screen.findByPlaceholderText("1000"), { target: { value: "2.5" } });
    fireEvent.click(screen.getByRole("button", { name: "Bid" }));
    expect(posted).toHaveLength(0);
    const confirm = await screen.findByTestId("shop-bid-confirm");
    expect(within(confirm).getByRole("button", { name: "Proceed" })).toBeDisabled();
    fireEvent.change(within(confirm).getByLabelText("Confirm the price"), { target: { value: "2.5" } });
    fireEvent.click(within(confirm).getByRole("button", { name: "Proceed" }));
    expect(await screen.findByTestId("shop-bid-done")).toHaveTextContent("2.5 MANA");
    expect(posted[0]).toMatchObject({ type: "bid", sent: [{ assetType: 1, amount: MANA("2.5") }] });
    expect(decodeFunctionData({ abi: ERC20_ABI, data: decodeFunctionData({ abi: COLLECTION_V2_ABI, data: relayed[0]! }).args![1] as Hex }).functionName).toBe("approve");
  });

  it("a short MANA balance blocks the bid; the owner cannot bid on their own token", async () => {
    const wallet = fakeWallet();
    chain({ balance: 10n ** 18n });
    const m = mountShop({ path: `/shop/token/${BOOK.collection}/77/bid`, who: "web3", wallet, routes: [nfts([listed]), { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: [], total: 0 } } }) }] });
    fireEvent.change(await screen.findByPlaceholderText("1000"), { target: { value: "2" } });
    expect(await screen.findByText("You don't have enough MANA")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Bid" })).toBeDisabled();
    m.unmount();
    mountShop({ path: `/shop/token/${BOOK.collection}/77/bid`, who: "web3", wallet, routes: [nfts([{ ...listed, nft: { ...listed.nft, owner: wallet.address } }])] });
    expect(await screen.findByTestId("shop-own-item")).toBeInTheDocument();
  });
});

describe("Accept a bid", () => {
  it("the owner reviews a bid and accepts it on the marketplace; others see the bids read-only", async () => {
    const wallet = fakeWallet();
    chain();
    const token = nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: wallet.address });
    const legacy = { ...LEGACY_BID_ROW, seller: wallet.address };
    mountShop({
      path: `/shop/token/${BOOK.collection}/105312/bids`,
      who: "web3",
      wallet,
      routes: [nfts([token]), { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: [legacy], total: 1 } } }) }],
    });
    const row = await screen.findByTestId("shop-token-bid");
    fireEvent.click(within(row).getByRole("button", { name: "Accept" }));
    const confirm = await screen.findByTestId("shop-accept-confirm");
    expect(confirm).toHaveTextContent("receive");
    const typed = within(confirm).getByLabelText("Confirm the price") as HTMLInputElement;
    fireEvent.change(typed, { target: { value: typed.placeholder } });
    fireEvent.click(within(confirm).getByRole("button", { name: "Proceed" }));
    expect(await screen.findByTestId("shop-accept-done")).toBeInTheDocument();
    expect(wallet.sent[0]!.data.slice(0, 10)).toBe("0xb88d4fde");
  });

  it("a visitor sees the bids but no Review", async () => {
    const token = nftRow({ tokenId: "105312", itemId: "7", name: "Neon Jacket", owner: BOOK.reseller });
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/bids`, who: "web3", routes: [nfts([token]), { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: RECEIVED_BIDS, total: 2 } } }) }] });
    expect(await screen.findAllByTestId("shop-token-bid")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
  });
});

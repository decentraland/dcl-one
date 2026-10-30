import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CHAIN } from "../../model/chain";
import { BOOK } from "../../payment/fixtures";
import { resetArchivedBidsForTests } from "../store/archivedBids";
import { FIXTURE_NOW, LEGACY_BID_ROW, MADE_BIDS, MANA, nftRow, OWNED_NAMES, OWNED_WEARABLES, RECEIVED_BIDS } from "../fixtures";
import { COLLECTION_V2_ABI, ERC20_ABI, ERC721_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, manaToken } from "../lib/contracts";
import { fakeChain, fakeWallet, type FakeRoute } from "../testing";
import { mountShop } from "../testHarness";

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
  it("signed out: the sign-in gate, no request for anyone's items", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "signedOut", routes: [nfts()] });
    expect(await screen.findByTestId("shop-signin-gate")).toBeInTheDocument();
    expect(m.shop.requests).toHaveLength(0);
  });

  it("lists owned wearables with their MANA listing price, never a credit price, and the manage actions", async () => {
    const m = mountShop({ path: "/shop/my-items", who: "web2", routes: [nfts(), importable(0)] });
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

  it("the old Marketplace section names land on their Shop section; empty, error and outage each explain themselves", async () => {
    const m = mountShop({ path: "/shop/my-items?section=ens", who: "web3", routes: [nfts([]), importable(0)] });
    expect(await screen.findByRole("button", { name: "Names" })).toHaveAttribute("aria-current", "page");
    expect(await screen.findAllByTestId("shop-asset-tile")).toHaveLength(1);
    m.unmount();
    mountShop({ path: "/shop/my-items?section=emotes", who: "web3", routes: [nfts([]), importable(0)] });
    expect(await screen.findByTestId("shop-empty")).toHaveTextContent("You have no emotes yet");
  });

  it("an index that has not caught up is an outage with its reason, not an empty page", async () => {
    mountShop({
      path: "/shop/my-items",
      who: "web3",
      routes: [{ method: "GET", path: /^\/v1\/nfts$/, reply: () => ({ status: 503, body: { error: "Service Unavailable", message: "no pass yet", source: "nft-indexer" } }) }, importable(0)],
    });
    expect(await screen.findByTestId("shop-outage")).toHaveTextContent("the NFT index is still catching up");
    expect(screen.queryByTestId("shop-empty")).toBeNull();
  });

  it("with the store dashboard off, creations stay reachable from My items (as upstream)", async () => {
    const { setShopCapabilities } = await import("../../seller/lib/featureFlags");
    setShopCapabilities({ myStore: false });
    try {
      mountShop({ path: "/shop/my-items?section=creations", who: "web3", routes: [nfts(), importable(0)] });
      expect(await screen.findByRole("button", { name: "Creations" })).toHaveAttribute("aria-current", "page");
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

  it("signed out: the sign-in page, as upstream's protected route redirects there", async () => {
    const m = mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "signedOut", routes: [nfts([owned(BOOK.reseller)])] });
    expect(await screen.findByTestId("land-sign-in")).toHaveTextContent("Get Started");
    expect(m.shop.requests).toHaveLength(0);
  });

  it("a wallet moved to another account never signs for the session: the switch is explicit", async () => {
    const { testIdentity } = await import("../testHarness");
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web3", host: { identity: testIdentity(BOOK.buyer) }, routes: [nfts([owned(BOOK.buyer)])] });
    const notice = await screen.findByTestId("shop-account-mismatch");
    expect(notice).toHaveTextContent("you are signed in as 0x0000…0b0b");
    expect(screen.queryByPlaceholderText("0x...")).toBeNull();
  });

  it("a web2 session owning the token is told to connect a web3 wallet", async () => {
    const wallet = fakeWallet();
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/transfer`, who: "web2", wallet, routes: [nfts([owned(wallet.address)])] });
    expect(await screen.findByTestId("shop-wallet-required")).toBeInTheDocument();
  });
});

describe("Place a bid: MANA only", () => {
  const listed = nftRow({ tokenId: "77", itemId: "7", name: "Glow Visor", owner: BOOK.reseller });

  it("signed out, a token or an item bid: the sign-in page, never a credit option", async () => {
    const m = mountShop({ path: `/shop/token/${BOOK.collection}/77/bid`, who: "signedOut", routes: [nfts([listed])] });
    expect(await screen.findByTestId("land-sign-in")).toHaveTextContent("Get Started");
    expect(m.shop.requests).toHaveLength(0);
    m.unmount();
    mountShop({ path: `/shop/item/${BOOK.collection}/7/bid`, who: "signedOut", routes: [] });
    expect(await screen.findByTestId("land-sign-in")).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/credit/i);
  });

  it("web2: the wallet/network/funding guidance, keeping the item, and never credits", async () => {
    mountShop({ path: `/shop/token/${BOOK.collection}/77/bid`, who: "web2", routes: [nfts([listed])] });
    const guidance = await screen.findByRole("heading", { name: "This purchase is paid in MANA" });
    const section = guidance.closest("section")!;
    expect(section).toHaveTextContent("Polygon");
    expect(section).toHaveTextContent("The MANA you offer, held on Polygon.");
    expect(within(section).getByRole("link", { name: "Glow Visor" })).toHaveAttribute("href", `/shop/token/${BOOK.collection}/77`);
    expect(document.body.textContent).not.toMatch(/credit/i);
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
    mountShop({ path: `/shop/token/${BOOK.collection}/105312/bids`, who: "signedOut", routes: [nfts([token]), { method: "GET", path: /^\/v1\/bids$/, reply: () => ({ body: { ok: true, data: { results: RECEIVED_BIDS, total: 2 } } }) }] });
    expect(await screen.findAllByTestId("shop-token-bid")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Accept" })).toBeNull();
  });
});

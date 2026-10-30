import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { CHAIN } from "../../model/chain";
import { manaFromWhole } from "../../model/money";
import { BOOK } from "../../payment/fixtures";
import { WrongNetworkError } from "../../wallet/chain";
import { LEGACY_BID_ROW, RECEIVED_BIDS, MANA } from "../fixtures";
import { COLLECTION_V2_ABI, ERC20_ABI, ERC721_ABI, LEGACY_ACCEPT_BID_ABI, LEGACY_MARKETPLACE, LEGACY_MARKETPLACE_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, manaToken } from "../lib/contracts";
import { createManaListing, createUsdPeggedListing, type Trade } from "../lib/trades";
import { fakeChain, fakeWallet, noWait } from "../testing";
import { archiveBid, archivedBidIds, resetArchivedBidsForTests, unarchiveBid } from "../store/archivedBids";
import { acceptBid, cancelBid, isBidTrade, placeBid } from "./bids";
import { buyTradeWithMana, executeLegacyOrder, manaAllowanceFor } from "./executeOrder";
import { transferItem, transferProblem } from "./transfer";

const MARKET = latestOffChainMarketplace(CHAIN.POLYGON).address;
const lower = (args: readonly unknown[] | undefined) => (args ?? []).map((a) => (typeof a === "string" ? a.toLowerCase() : a));
const MANA_POLYGON = manaToken(CHAIN.POLYGON).address;
let restore: (() => void) | null = null;
const originalFetch = globalThis.fetch;
let relayed: { params: [string, Hex] }[] = [];

beforeEach(() => {
  relayed = [];
  globalThis.fetch = (async (url: string, init?: RequestInit) => {
    const u = String(url);
    if (u.endsWith("/transactions")) {
      relayed.push((JSON.parse(String(init?.body)) as { transactionData: { params: [string, Hex] } }).transactionData);
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"e".repeat(64)}` }));
    }
    throw new Error(`unexpected fetch ${u}`);
  }) as typeof fetch;
});

afterEach(() => {
  restore?.();
  restore = null;
  globalThis.fetch = originalFetch;
});

describe("bids are MANA only", () => {
  it("placing a bid approves exactly the bid's MANA (relayed on Polygon), signs, and posts", async () => {
    const wallet = fakeWallet();
    let allowance = 0n;
    restore = fakeChain({
      contracts: {
        [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
        [MANA_POLYGON]: { abi: [...ERC20_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "allowance" ? allowance : 0n) },
      },
    });
    const phases: string[] = [];
    const posted: unknown[] = [];
    const { setShopClient } = await import("../../api/shop");
    const { fakeShop } = await import("../testing");
    const shop = fakeShop([{ method: "POST", path: /^\/v1\/trades$/, reply: ({ body }) => (posted.push(body), { status: 201, body: { ok: true, data: { ...(body as object), id: "t-9", contract: MARKET } } }) }]);
    setShopClient(shop.client);
    try {
      const trade = await placeBid({
        provider: wallet,
        target: { contractAddress: BOOK.collection, chainId: CHAIN.POLYGON, tokenId: "42" },
        priceWei: manaFromWhole(3),
        expiresAtMs: Date.now() + 86_400_000,
        onPhase: (p) => phases.push(p),
        wait: noWait,
      });
      expect(trade.id).toBe("t-9");
    } finally {
      setShopClient(null);
    }
    expect(phases).toEqual(["approving", "signing", "publishing"]);
    const approve = decodeFunctionData({ abi: ERC20_ABI, data: decodeFunctionData({ abi: COLLECTION_V2_ABI, data: relayed[0]!.params[1] }).args![1] as Hex });
    expect(lower(approve.args)).toEqual([MARKET, manaFromWhole(3)]);
    expect((posted[0] as { type: string }).type).toBe("bid");
    expect(shop.requests[0]!.signed).toBe(true);
    allowance = manaFromWhole(3);
  });

  it("accepting a legacy bid sends the token to the Bid contract with the bid id; a trade bid accepts on the marketplace", async () => {
    const wallet = fakeWallet();
    restore = fakeChain({});
    const hash = await acceptBid({ provider: wallet, bid: { ...LEGACY_BID_ROW, seller: wallet.address }, wait: noWait });
    const call = decodeFunctionData({ abi: LEGACY_ACCEPT_BID_ABI, data: wallet.sent[0]!.data });
    expect(lower(call.args)).toEqual([wallet.address, LEGACY_BID_ROW.bidAddress, 105312n, LEGACY_BID_ROW.blockchainId]);
    expect(wallet.sent[0]!.to).toBe(LEGACY_BID_ROW.contractAddress);
    expect(hash).toBeDefined();
    expect(isBidTrade(RECEIVED_BIDS[0]!)).toBe(true);
    expect(isBidTrade(LEGACY_BID_ROW)).toBe(false);
  });

  it("only the bidder cancels", async () => {
    const wallet = fakeWallet();
    await expect(cancelBid({ provider: wallet, bid: RECEIVED_BIDS[0]!, wait: noWait })).rejects.toThrow(/placed this bid/);
    expect(wallet.sent).toHaveLength(0);
  });

  it("archiving a received bid is local and per account", () => {
    resetArchivedBidsForTests();
    localStorage.clear();
    archiveBid(BOOK.buyer, "b1");
    expect(archivedBidIds(BOOK.buyer)).toEqual(["b1"]);
    expect(archivedBidIds(BOOK.reseller)).toEqual([]);
    unarchiveBid(BOOK.buyer, "b1");
    expect(archivedBidIds(BOOK.buyer)).toEqual([]);
  });
});

describe("transfers", () => {
  it("refuses a bad, zero or own address before any prompt", () => {
    expect(transferProblem("nope", null)).toBe("invalidAddress");
    expect(transferProblem("0x0000000000000000000000000000000000000000", null)).toBe("zeroAddress");
    expect(transferProblem(BOOK.buyer.toUpperCase().replace("0X", "0x"), BOOK.buyer)).toBe("sameAddress");
    expect(transferProblem(BOOK.reseller, BOOK.buyer)).toBeNull();
  });

  it("relays a Polygon collection transfer; sends a NAME on Ethereum from the wallet, on its chain only", async () => {
    const wallet = fakeWallet({ chainId: CHAIN.POLYGON });
    restore = fakeChain({ contracts: { [BOOK.collection]: { abi: COLLECTION_V2_ABI, answer: () => 0n } } });
    await transferItem({ provider: wallet, contractAddress: BOOK.collection, chainId: CHAIN.POLYGON, tokenId: "42", to: BOOK.reseller, wait: noWait });
    const inner = decodeFunctionData({ abi: COLLECTION_V2_ABI, data: relayed[0]!.params[1] }).args![1] as Hex;
    expect(lower(decodeFunctionData({ abi: ERC721_ABI, data: inner }).args)).toEqual([wallet.address, BOOK.reseller, 42n]);
    expect(wallet.sent).toHaveLength(0);

    await expect(
      transferItem({ provider: wallet, contractAddress: BOOK.registrar.contract, chainId: CHAIN.ETHEREUM, tokenId: "42", to: BOOK.reseller, collection: false, wait: noWait }),
    ).rejects.toBeInstanceOf(WrongNetworkError);
    wallet.chainId = CHAIN.ETHEREUM;
    await transferItem({ provider: wallet, contractAddress: BOOK.registrar.contract, chainId: CHAIN.ETHEREUM, tokenId: "42", to: BOOK.reseller, collection: false, wait: noWait });
    expect(decodeFunctionData({ abi: ERC721_ABI, data: wallet.sent[0]!.data }).functionName).toBe("transferFrom");
  });
});

describe("buying a listed token is MANA only", () => {
  it("allows exactly a MANA listing's price, then accepts it", async () => {
    const seller = fakeWallet({ key: `0x${"7".repeat(64)}` });
    const buyer = fakeWallet();
    restore = fakeChain({
      contracts: {
        [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
        [MANA_POLYGON]: { abi: [...ERC20_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "allowance" ? manaFromWhole(1) : 0n) },
      },
    });
    const creation = await createManaListing({
      provider: seller,
      nft: { contractAddress: BOOK.collection, tokenId: "42", chainId: CHAIN.POLYGON },
      priceWei: manaFromWhole(1),
      expiresAtMs: Date.now() + 1000,
    });
    const trade: Trade = { ...creation, id: "t", contract: MARKET };
    expect(manaAllowanceFor(trade, null)).toBe(manaFromWhole(1));
    const phases: string[] = [];
    await buyTradeWithMana({ provider: buyer, trade, onPhase: (p) => phases.push(p), wait: noWait });
    expect(phases).toEqual(["approving", "confirming", "pending"]);
    expect(relayed).toHaveLength(0);
    expect(decodeFunctionData({ abi: MARKETPLACE_ABI, data: buyer.sent[0]!.data }).functionName).toBe("accept");
  });

  it("a USD-pegged resale needs the server's current MANA quote (with headroom), and refuses without one", async () => {
    const seller = fakeWallet({ key: `0x${"7".repeat(64)}` });
    restore = fakeChain({ contracts: { [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n } } });
    const creation = await createUsdPeggedListing({
      provider: seller,
      nft: { contractAddress: BOOK.collection, tokenId: "42", chainId: CHAIN.POLYGON },
      usdCents: 100,
      expiresAtMs: Date.now() + 1000,
    });
    const trade: Trade = { ...creation, id: "t", contract: MARKET };
    expect(() => manaAllowanceFor(trade, null)).toThrow(/unavailable/);
    expect(manaAllowanceFor(trade, manaFromWhole(2))).toBe(BigInt(MANA("2.1")));
  });
});

describe("a legacy on-chain order", () => {
  const LEGACY = LEGACY_MARKETPLACE[CHAIN.ETHEREUM]!.address;
  const MANA_ETH = manaToken(CHAIN.ETHEREUM).address;
  const WEI_4000 = 4_000n * 10n ** 18n;
  const order = { contractAddress: BOOK.land, tokenId: "42", price: (WEI_4000).toString(), chainId: CHAIN.ETHEREUM, marketplaceAddress: LEGACY };

  it("is executed on the chain's legacy marketplace at the order's price; an Estate binds the reviewed fingerprint", async () => {
    const wallet = fakeWallet({ chainId: CHAIN.ETHEREUM });
    restore = fakeChain({ contracts: { [MANA_ETH]: { abi: ERC20_ABI, answer: () => WEI_4000 } } });
    await executeLegacyOrder({ provider: wallet, order, wait: noWait });
    const fingerprint = `0x${"ab".repeat(32)}` as const;
    await executeLegacyOrder({ provider: wallet, order: { ...order, contractAddress: BOOK.land }, fingerprint, wait: noWait });
    const calls = wallet.sent.map((tx) => ({ to: tx.to.toLowerCase(), call: decodeFunctionData({ abi: LEGACY_MARKETPLACE_ABI, data: tx.data as Hex }) }));
    expect(calls.map((c) => [c.to, c.call.functionName])).toEqual([
      [LEGACY.toLowerCase(), "executeOrder"],
      [LEGACY.toLowerCase(), "safeExecuteOrder"],
    ]);
    expect(lower(calls[1]!.call.args)).toEqual([BOOK.land.toLowerCase(), 42n, WEI_4000, fingerprint]);
  });

  it("refuses an order naming any other contract before asking for an allowance", async () => {
    const wallet = fakeWallet({ chainId: CHAIN.ETHEREUM });
    await expect(executeLegacyOrder({ provider: wallet, order: { ...order, marketplaceAddress: "0x000000000000000000000000000000000000dead" }, wait: noWait })).rejects.toThrow(
      "This order cannot be executed from the Shop.",
    );
    expect(wallet.sent).toEqual([]);
  });
});


import { decodeFunctionData, type Hex } from "viem";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { CHAIN } from "@shop/model/chain";
import { manaFromWhole } from "@shop/model/money";
import { BOOK } from "@shop/payment/fixtures";
import { WrongNetworkError } from "../../wallet/chain";
import { LEGACY_BID_ROW, RECEIVED_BIDS, MANA } from "../fixtures";
import { COLLECTION_V2_ABI, ERC20_ABI, ERC721_ABI, LEGACY_ACCEPT_BID_ABI, LEGACY_MARKETPLACE, LEGACY_MARKETPLACE_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, manaToken } from "@shop/market/lib/contracts";
import { createManaListing, createUsdPeggedListing, type Trade } from "../lib/trades";
import { fakeChain, fakeWallet, noWait, STALL } from "../testing";
import { archiveBid, archivedBidIds, resetArchivedBidsForTests, unarchiveBid } from "../store/archivedBids";
import { acceptBid, cancelBid, isBidTrade, placeBid } from "./bids";
import { buyTradeWithMana, executeLegacyOrder, manaAllowanceFor } from "./executeOrder";
import { ensureManaAllowance, resetUnconfirmedApprovalsForTests } from "../lib/mana";
import { APPROVAL_REVERTED, purchaseFailure, TxPendingError, TxRevertedError, type WaitOptions } from "../lib/tx";
import type { Eip1193Provider } from "../../../data/auth/wallet";
import { transferItem, transferProblem } from "./transfer";

const MARKET = latestOffChainMarketplace(CHAIN.POLYGON).address;
const lower = (args: readonly unknown[] | undefined) => (args ?? []).map((a) => (typeof a === "string" ? a.toLowerCase() : a));
const MANA_POLYGON = manaToken(CHAIN.POLYGON).address;
let restore: (() => void) | null = null;
const originalFetch = globalThis.fetch;
let relayed: { params: [string, Hex] }[] = [];

beforeEach(() => {
  resetUnconfirmedApprovalsForTests();
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
    const { setShopClient } = await import("../../api/host");
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

describe("a MANA approval still confirming", () => {
  const SPENDER = `0x${"5".repeat(40)}`;
  const AMOUNT = manaFromWhole(3);
  /** Each relayed approval gets its own hash, in order. */
  const approvalHash = (n: number) => `0x${n.toString(16).padStart(64, "0")}`;
  let allowance = 0n;
  let allowanceDown = false;
  let nonceStalled = false;
  let nonceNow = 0n;
  const pending = new Set<string>();
  const reverted = new Set<string>();
  const stalled = new Set<string>();
  /** What the chain does when the n-th approval is relayed (by default, nothing until a test says so). */
  let onRelay: (n: number) => void = () => {};

  beforeEach(() => {
    allowance = 0n;
    allowanceDown = false;
    nonceStalled = false;
    nonceNow = 0n;
    pending.clear();
    reverted.clear();
    stalled.clear();
    onRelay = () => {};
    globalThis.fetch = (async (url: string, init?: RequestInit) => {
      if (!String(url).endsWith("/transactions")) throw new Error(`unexpected fetch ${String(url)}`);
      relayed.push((JSON.parse(String(init?.body)) as { transactionData: { params: [string, Hex] } }).transactionData);
      onRelay(relayed.length);
      return new Response(JSON.stringify({ ok: true, txHash: approvalHash(relayed.length) }));
    }) as typeof fetch;
    restore = fakeChain({
      contracts: {
        [MANA_POLYGON]: {
          abi: [...ERC20_ABI, ...COLLECTION_V2_ABI],
          answer: (fn) => {
            if (fn === "getNonce") return nonceStalled ? STALL : nonceNow;
            if (fn !== "allowance") return 0n;
            if (allowanceDown) throw new Error("the RPC is down");
            return allowance;
          },
        },
      },
      pending,
      reverted,
      stalled,
    });
  });

  /** A wallet whose account can change (or disconnect, or stop answering its account) between two purchases. */
  function switchable(first = fakeWallet()) {
    const box: { current: ReturnType<typeof fakeWallet> | null; stallAccounts: boolean; stallSign: boolean } = { current: first, stallAccounts: false, stallSign: false };
    const provider: Eip1193Provider = {
      request: (args) => {
        if (box.stallAccounts && args.method === "eth_accounts") return new Promise(() => {});
        if (box.stallSign && args.method === "eth_signTypedData_v4") return new Promise(() => {});
        if (box.current) return box.current.request(args);
        return args.method === "eth_accounts" ? Promise.resolve([]) : Promise.reject(new Error("disconnected"));
      },
    } as Eip1193Provider;
    return { provider, box };
  }

  const approve = (wallet: Eip1193Provider = fakeWallet(), amount = AMOUNT, wait: WaitOptions = noWait) =>
    ensureManaAllowance({ provider: wallet, chainId: CHAIN.POLYGON, spender: SPENDER, amount, wait });
  const quick = { ...noWait, readTimeoutMs: 20 };

  it("a purchase queued behind another ends unsigned and unsent once the wallet switched accounts", async () => {
    const { provider, box } = switchable();
    const other = fakeWallet({ key: `0x${"7".repeat(64)}` });
    onRelay = () => {
      box.current = other;
    };
    const [first, second] = await Promise.allSettled([approve(provider), approve(provider)]);
    expect(first).toMatchObject({ status: "fulfilled", value: approvalHash(1) });
    expect(second).toMatchObject({ status: "rejected", reason: { name: "WalletAccountChangedError" } });
    expect(relayed).toHaveLength(1);
    expect(other.signed).toHaveLength(0);
  });

  it("a legacy order whose wallet switched accounts after its approval went out sends no sale from the other account", async () => {
    const { provider, box } = switchable();
    const other = fakeWallet({ key: `0x${"9".repeat(64)}` });
    onRelay = () => {
      box.current = other;
    };
    const venue = LEGACY_MARKETPLACE[CHAIN.POLYGON]!.address;
    const order = { contractAddress: BOOK.collection, tokenId: "42", price: AMOUNT.toString(), chainId: CHAIN.POLYGON, marketplaceAddress: venue };
    const err = await executeLegacyOrder({ provider, order, wait: noWait }).catch((e: unknown) => e);
    expect(err).toMatchObject({ name: "WalletAccountChangedError", approvalSent: true });
    expect(relayed).toHaveLength(1);
    expect(other.sent).toHaveLength(0);
    expect(purchaseFailure(err, null)).toEqual({ kind: "error", message: expect.stringMatching(/approval went out.*nothing was bought/) });
  });

  it("a trade whose wallet switched accounts after its approval went out sends no accept from the other account", async () => {
    restore?.();
    restore = fakeChain({
      contracts: {
        [MARKET]: { abi: MARKETPLACE_ABI, answer: () => 0n },
        [MANA_POLYGON]: { abi: [...ERC20_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "allowance" ? allowance : 0n) },
      },
    });
    const creation = await createManaListing({
      provider: fakeWallet({ key: `0x${"7".repeat(64)}` }),
      nft: { contractAddress: BOOK.collection, tokenId: "42", chainId: CHAIN.POLYGON },
      priceWei: AMOUNT,
      expiresAtMs: Date.now() + 60_000,
    });
    const trade: Trade = { ...creation, id: "t", contract: MARKET };
    const { provider, box } = switchable();
    const other = fakeWallet({ key: `0x${"9".repeat(64)}` });
    onRelay = () => {
      box.current = other;
    };
    const err = await buyTradeWithMana({ provider, trade, wait: noWait }).catch((e: unknown) => e);
    expect(err).toMatchObject({ name: "WalletAccountChangedError", approvalSent: true });
    expect(relayed).toHaveLength(1);
    expect(other.sent).toHaveLength(0);
  });

  it("a nonce read that never answers ends the approval unprompted and unsent, and frees the next purchase", async () => {
    nonceStalled = true;
    const wallet = fakeWallet();
    await expect(approve(wallet, AMOUNT, quick)).rejects.toMatchObject({ name: "ReadTimeoutError" });
    expect(wallet.signed).toHaveLength(0);
    expect(relayed).toHaveLength(0);
    nonceStalled = false;
    await expect(approve(wallet, AMOUNT, quick)).resolves.toBe(approvalHash(1));
  });

  it("an account read that never answers when a queued purchase starts ends that purchase unsent", async () => {
    const { provider, box } = switchable();
    onRelay = () => {
      box.stallAccounts = true;
    };
    const [first, second] = await Promise.allSettled([approve(provider, AMOUNT, quick), approve(provider, AMOUNT, quick)]);
    expect(first).toMatchObject({ status: "fulfilled", value: approvalHash(1) });
    expect(second).toMatchObject({ status: "rejected", reason: { name: "ReadTimeoutError" } });
    expect(relayed).toHaveLength(1);
  });

  it("a purchase queued behind one whose wallet request never settles gives up unsent, and so does the next", async () => {
    const { provider, box } = switchable();
    box.stallSign = true;
    // The wallet dropped the signature request without rejecting it: this decision never ends.
    void approve(provider).catch(() => {});
    const behind = () => ensureManaAllowance({ provider, chainId: CHAIN.POLYGON, spender: SPENDER, amount: AMOUNT, wait: noWait, queueTimeoutMs: 20 });
    await expect(behind()).rejects.toMatchObject({ name: "ApprovalBusyError" });
    // Giving up does not let a later purchase past the stuck one.
    await expect(behind()).rejects.toMatchObject({ name: "ApprovalBusyError" });
    expect(relayed).toHaveLength(0);
  });

  it("a relayed approval whose hash never gets a receipt ends once the owner's nonce is past the one it signed", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    // Still unmined and the nonce has not moved: it still holds a second approval back.
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    // The relay's replacement ran (the nonce moved) and a sale spent that allowance; the first hash never mines.
    nonceNow = 1n;
    await expect(approve(wallet)).resolves.toBe(approvalHash(2));
    expect(relayed).toHaveLength(2);
  });

  it("a purchase queued behind another ends unsent once the wallet disconnected", async () => {
    const { provider, box } = switchable();
    onRelay = () => {
      box.current = null;
    };
    const [first, second] = await Promise.allSettled([approve(provider), approve(provider)]);
    expect(first.status).toBe("fulfilled");
    expect(second.status).toBe("rejected");
    expect(relayed).toHaveLength(1);
  });

  it("a first wait whose receipt read never answers ends pending on its hash, and the purchase queued behind it too", async () => {
    stalled.add(approvalHash(1));
    const wallet = fakeWallet();
    const both = await Promise.allSettled([approve(wallet, AMOUNT, quick), approve(wallet, AMOUNT, quick)]);
    expect(both.map((r) => r.status === "rejected" && (r.reason as TxPendingError).txHash)).toEqual([approvalHash(1), approvalHash(1)]);
    expect(relayed).toHaveLength(1);
  });

  it("an allowance read that fails before sending sends nothing, and frees the next purchase", async () => {
    allowanceDown = true;
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toThrow(/couldn't read your MANA allowance/);
    expect(relayed).toHaveLength(0);
    allowanceDown = false;
    await expect(approve(wallet)).resolves.toBe(approvalHash(1));
  });

  it("two buys at once send one approval: the second waits for the first, then reads its own allowance", async () => {
    onRelay = () => {
      allowance = AMOUNT;
    };
    const wallet = fakeWallet();
    expect(await Promise.all([approve(wallet), approve(wallet)])).toEqual([approvalHash(1), null]);
    expect(relayed).toHaveLength(1);
  });

  it("two buys at once with the first approval unmined: both stop on its hash", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    const both = await Promise.allSettled([approve(wallet), approve(wallet)]);
    expect(both.map((r) => r.status === "rejected" && (r.reason as TxPendingError).txHash)).toEqual([approvalHash(1), approvalHash(1)]);
    expect(relayed).toHaveLength(1);
  });

  it("a second buy for more, queued behind the first, approves its own amount once the first has confirmed", async () => {
    const MORE = manaFromWhole(5);
    onRelay = (n) => {
      allowance = n === 1 ? AMOUNT : MORE;
    };
    const wallet = fakeWallet();
    expect(await Promise.all([approve(wallet), approve(wallet, MORE)])).toEqual([approvalHash(1), approvalHash(2)]);
    expect(relayed).toHaveLength(2);
  });

  it("an allowance that covers a smaller buy lets it pass, and keeps the larger approval's hash", async () => {
    allowance = manaFromWhole(1);
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    await expect(approve(wallet, manaFromWhole(1))).resolves.toBeNull();
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    expect(relayed).toHaveLength(1);
  });

  it("a retry whose receipt read never answers stops on the known hash in bounded time, and keeps it", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toBeInstanceOf(TxPendingError);
    pending.delete(approvalHash(1));
    stalled.add(approvalHash(1));
    await expect(approve(wallet, AMOUNT, quick)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    await expect(approve(wallet, AMOUNT, quick)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    expect(relayed).toHaveLength(1);
  });

  it("sends no second approval while the first is unmined: a second buy stops on the first one's hash", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    await expect(approve(wallet)).rejects.toMatchObject({ name: "TxPendingError", txHash: approvalHash(1) });
    expect(relayed).toHaveLength(1);
  });

  it("once the first approval mines, the next buy reads the allowance again and sends none", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toBeInstanceOf(TxPendingError);
    pending.delete(approvalHash(1));
    allowance = AMOUNT;
    await expect(approve(wallet)).resolves.toBeNull();
    expect(relayed).toHaveLength(1);
  });

  it("a first approval that reverted is forgotten: the next buy sends a new one", async () => {
    pending.add(approvalHash(1));
    const wallet = fakeWallet();
    await expect(approve(wallet)).rejects.toBeInstanceOf(TxPendingError);
    pending.delete(approvalHash(1));
    reverted.add(approvalHash(1));
    await expect(approve(wallet)).resolves.toBe(approvalHash(2));
    expect(relayed).toHaveLength(2);
  });

  it("holds back only the same owner and spender", async () => {
    pending.add(approvalHash(1));
    await expect(approve(fakeWallet())).rejects.toBeInstanceOf(TxPendingError);
    await expect(approve(fakeWallet({ key: `0x${"7".repeat(64)}` }))).resolves.toBe(approvalHash(2));
    expect(relayed).toHaveLength(2);
  });
});

describe("how a MANA buy that threw ended", () => {
  const SALE = `0x${"d".repeat(64)}`;
  const APPROVAL = `0x${"a".repeat(64)}`;

  it("a wait that ran out before the sale went out is the approval's, and carries its hash", () => {
    expect(purchaseFailure(new TxPendingError(APPROVAL, "The MANA approval"), null)).toEqual({ kind: "approval-pending", txHash: APPROVAL });
    expect(purchaseFailure(new TxPendingError(SALE, "The purchase"), SALE)).toEqual({ kind: "pending", txHash: SALE });
  });

  it("a revert before the sale went out is the approval's, not a sold listing", () => {
    expect(purchaseFailure(new TxRevertedError(APPROVAL), null)).toEqual({ kind: "error", message: APPROVAL_REVERTED });
    expect(purchaseFailure(new TxRevertedError(SALE), SALE)).toEqual({ kind: "error", message: expect.stringMatching(/listing may have been sold/) });
  });
});

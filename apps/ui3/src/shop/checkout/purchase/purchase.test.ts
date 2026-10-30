import { describe, expect, it, vi } from "vitest";

import { partitionCart, type CartGroup } from "../../cart/partition";
import { createCartStore, type CartStorage, type CartStore } from "../../cart/store";
import type { CartLine, PurchaseOffer } from "../../model/purchase";
import type { Session } from "../../model/settlement";
import { presentPayment } from "../../payment/present";
import {
  BOOK,
  creatorOrder,
  estateResale,
  nameRegistration,
  nameResale,
  parcelResale,
  rental,
  storeMint,
  wearableResale,
  web2,
  web3,
} from "../../payment/fixtures";
import {
  batchAuthorization,
  createFakeChain,
  createFakeRelay,
  createFakeShop,
  createFakeWallet,
  creatorTrade,
  resaleTrade,
  type FakeHandlers,
} from "../fakes";
import { reviewCart, resolveLine, partitionReservations, couponForTrade, classifyTrade, classifyStoreMint, isLineBuyable, type ResolvedLine, type SettlementResolvers } from "./cart";
import { resumeCheckout, runGroup, watchGroup, type CheckoutDeps } from "./executor";
import { computePaymentOptions, distributeCreditsAcrossUnits, findOption, hasPayableOption } from "./payment-options";
import type { RailContext } from "./rail";
import { createSpendGuard } from "./spend-guard";

function memoryStorage(): CartStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), removeItem: (k) => void data.delete(k) };
}

type Spec = { offer: PurchaseOffer; tradeId?: string; listing?: CartLine["listing"]; name: string; method?: "credits" | "mana" };

function cartWith(offers: Spec[], storage = memoryStorage()): { store: CartStore; lines: CartLine[]; storage: typeof storage } {
  const store = createCartStore({ storage: () => storage });
  store.scopeTo(BOOK.buyer);
  for (const o of offers) {
    const line = store.add({ asset: o.offer.asset, listing: o.listing ?? (o.tradeId ? { tradeId: o.tradeId } : {}), display: { name: o.name }, seen: o.offer });
    if (o.method) store.setMethod(line.id, o.method);
  }
  return { store, lines: store.getSnapshot().lines, storage };
}

const noop = () => {};

describe("the settlement groups: a credits group never holds a restricted line", () => {
  const session = web3();
  const offers = [
    { offer: storeMint(session), name: "Hat" },
    { offer: creatorOrder(session), tradeId: "trade-creator-1", name: "Order" },
    { offer: wearableResale(session), tradeId: "trade-resale-1", name: "Resale" },
    { offer: parcelResale(session), listing: { orderId: "order-parcel" }, name: "Parcel" },
    { offer: estateResale(session), tradeId: "trade-estate", name: "Estate" },
    { offer: nameResale(session), tradeId: "trade-name", name: "NAME resale" },
    { offer: rental(session), listing: { rentalId: "rental-1" }, name: "Rental" },
    { offer: nameRegistration(session), listing: { name: "Nacho" }, name: "NAME" },
  ];

  it("partitions a mixed cart by method, chain and contract, restricted lines only ever in MANA groups", () => {
    const { lines } = cartWith(offers);
    expect(lines).toHaveLength(offers.length);
    const priced = lines.map((line, i) => ({ line, offer: offers[i]!.offer }));
    const part = partitionCart(priced, session, { report: noop });
    expect(part.ok).toBe(true);
    const restricted = new Set(priced.filter((p) => presentPayment(p.offer, session, { report: noop }).manaOnly).map((p) => p.line.id));
    expect(restricted.size).toBe(5);
    for (const g of part.groups.filter((g) => g.key.method === "credits")) {
      for (const id of g.lineIds) expect(restricted.has(id), id).toBe(false);
    }
    for (const id of restricted) {
      expect(part.groups.find((g) => g.lineIds.includes(id))!.key.method).toBe("mana");
    }
    expect(part.totals.credits.charged).toBeGreaterThan(0);
    expect(part.totals.mana).toBeGreaterThan(0n);
  });

  it("refuses a restricted line whose buyer asked for credits instead of moving it", () => {
    const { lines } = cartWith([{ offer: wearableResale(session), tradeId: "r", name: "Resale", method: "credits" }]);
    const part = partitionCart([{ line: lines[0]!, offer: wearableResale(session) }], session, { report: noop });
    expect(part.groups).toEqual([]);
    expect(part.refused).toEqual([expect.objectContaining({ code: "methodNotAllowed", method: "credits", allowed: ["mana"] })]);
  });
});

describe("payment options come from the rule, not from balances", () => {
  it("an eligible line stays credits-first with an explicit top-up when short; MANA second with credits+MANA inside it", () => {
    const view = presentPayment(creatorOrder(web3()), web3(), { creditsBalance: 30 as never, manaBalance: 5n * 10n ** 18n as never, report: noop });
    const o = computePaymentOptions(view);
    expect(o.preferred).toBe("credits");
    expect(o.options.map((x) => x.method)).toEqual(["credits", "combined", "mana"]);
    expect(o.topUp).toEqual({ short: 70, required: 100, balance: 30 });
    expect(findOption(o, "credits")!.state.kind).toBe("topUpRequired");
    expect(hasPayableOption(o)).toBe(true);
  });

  it("a resale is MANA only: no credit price, no top-up, no combined rail; web2 gets guidance", () => {
    const o = computePaymentOptions(presentPayment(wearableResale(web2()), web2(), { creditsBalance: 0 as never, report: noop }));
    expect(o.options.map((x) => x.method)).toEqual(["mana"]);
    expect(o.topUp).toBeNull();
    expect(o.manaOnly).toBe(true);
    expect(o.guidance).toMatchObject({ chainId: 137, account: BOOK.buyer });
    expect(hasPayableOption(o)).toBe(false);
  });

  it("spreads a balance across units in order", () => {
    expect(distributeCreditsAcrossUnits([100, 100, 100], 150)).toEqual([100, 50, 0]);
  });
});

function resolvers(over: Partial<SettlementResolvers> = {}): SettlementResolvers {
  return {
    trade: async (line) => (line.listing.tradeId === "trade-resale-1" ? resaleTrade() : line.listing.tradeId ? creatorTrade() : null),
    store: async () => ({ priceWei: (2_300n * 10n ** 15n).toString(), available: 10 }),
    coupon: async () => undefined,
    ...over,
  };
}

describe("resolving a line for settlement against the offer the buyer confirmed", () => {
  const session = web3();

  it("a creator order resolves to its trade; a store mint to its live price", async () => {
    const { lines } = cartWith([
      { offer: creatorOrder(session), tradeId: "trade-creator-1", name: "Order" },
      { offer: storeMint(session), name: "Hat" },
    ]);
    const order = await resolveLine(lines[0]!, creatorOrder(session), session, BOOK.buyer, resolvers());
    expect(order).toMatchObject({ status: "buyable", resolved: { acquisition: "trade", method: "credits", quote: { method: "credits", quoted: 100 } } });
    const mint = await resolveLine(lines[1]!, storeMint(session), session, BOOK.buyer, resolvers());
    expect(mint).toMatchObject({ status: "buyable", resolved: { acquisition: "store" } });
  });

  it("stale price, sold listing, own listing and unsupported settlement are each named, never charged", async () => {
    const { lines } = cartWith([{ offer: creatorOrder(session), tradeId: "trade-creator-1", name: "Order" }]);
    const moved = creatorTrade({ received: [{ assetType: 2, contractAddress: BOOK.mana, amount: (2n * 10n ** 18n).toString() }] });
    expect((await resolveLine(lines[0]!, creatorOrder(session), session, BOOK.buyer, resolvers({ trade: async () => moved }))).status).toBe("stale");
    expect((await resolveLine(lines[0]!, creatorOrder(session), session, BOOK.buyer, resolvers({ trade: async () => null }))).status).toBe("gone");
    expect((await resolveLine(lines[0]!, creatorOrder(session), session, BOOK.creator, resolvers())).status).toBe("own");
    const name = cartWith([{ offer: nameRegistration(session), name: "NAME" }]).lines[0]!;
    expect((await resolveLine(name, nameRegistration(session), session, BOOK.buyer, resolvers())).status).toBe("unsupported");
    const mint = cartWith([{ offer: storeMint(session), name: "Hat" }]).lines[0]!;
    const repriced = resolvers({ store: async () => ({ priceWei: "1", available: 10 }) });
    mint.method = "mana";
    expect((await resolveLine(mint, storeMint(session), session, BOOK.buyer, repriced)).status).toBe("stale");
  });

  it("reviewCart sorts every line and never aborts on one bad row", async () => {
    const { lines } = cartWith([
      { offer: creatorOrder(session), tradeId: "trade-creator-1", name: "Order" },
      { offer: wearableResale(session), tradeId: "trade-resale-1", name: "Resale" },
    ]);
    const review = await reviewCart(
      [
        { line: lines[0]!, offer: creatorOrder(session) },
        { line: lines[1]!, offer: null },
      ],
      session,
      BOOK.buyer,
      resolvers({ trade: async () => Promise.reject(new Error("boom")) }),
    );
    expect(review.buyable).toHaveLength(0);
    expect(review.unavailable).toEqual([lines[0]!.id, lines[1]!.id]);
  });

  it("availability and coupons fail closed", () => {
    expect(isLineBuyable(undefined)).toBe(true);
    expect(classifyTrade({ asset: storeMint(session).asset }, null)).toBe("sold-out");
    expect(classifyTrade({ asset: wearableResale(session).asset }, null)).toBe("unavailable");
    expect(classifyTrade({ asset: wearableResale(session).asset }, resaleTrade({ checks: { ...resaleTrade().checks, expiration: 1 } }))).toBe("unavailable");
    expect(classifyStoreMint({ priceWei: "5", available: 0 })).toBe("sold-out");
    expect(couponForTrade(undefined, creatorTrade())).toBeUndefined();
    expect(partitionReservations({ reservations: [{ salt: "a", lineId: "1" }, { salt: "b", lineId: "2" }], spent: new Set(["a"]), settled: new Set(["a"]) })).toEqual({
      toRelease: ["b"],
      boughtLineIds: ["1"],
    });
  });
});

type Harness = {
  deps: CheckoutDeps;
  store: CartStore;
  group: CartGroup;
  resolved: Map<string, ResolvedLine>;
  shop: ReturnType<typeof createFakeShop>;
  chain: ReturnType<typeof createFakeChain>;
  wallet: ReturnType<typeof createFakeWallet>;
  relay: ReturnType<typeof createFakeRelay>;
  storage: ReturnType<typeof memoryStorage>;
};

async function harness(opts: { method?: "credits" | "mana"; handlers?: FakeHandlers; session?: Session; relay?: "ok" | "refuse" | "down"; confirm?: "success" | "reverted" | "none" } = {}): Promise<Harness> {
  const session = opts.session ?? web3();
  const offer = opts.method === "mana" ? wearableResale(session) : creatorOrder(session);
  const tradeId = opts.method === "mana" ? "trade-resale-1" : "trade-creator-1";
  const { store, lines, storage } = cartWith([{ offer, tradeId, name: "Line" }]);
  const chain = createFakeChain();
  const relay = createFakeRelay({
    mode: opts.relay ?? "ok",
    onRelay: (hash) => {
      if ((opts.confirm ?? "success") !== "none") chain.receipts.set(hash.toLowerCase(), opts.confirm === "reverted" ? "reverted" : "success");
    },
  });
  const shop = createFakeShop({
    authorizeCreditBatch: ({ body }) => batchAuthorization((body as { items: { usdPriceCents: number }[] }).items),
    cancelCreditAuthorizations: ({ body }) => ({ released: (body as { salts: string[] }).salts.length, retired: 0, spendableAtSeconds: null }),
    reportCreditSubmission: () => ({ recorded: 1 }),
    ...opts.handlers,
  });
  const wallet = createFakeWallet();
  let clock = 0;
  const ctx: RailContext = {
    buyer: BOOK.buyer,
    provider: wallet,
    reader: chain,
    client: shop.client,
    walletKind: session.kind === "web3" ? "web3" : "managed",
    relayerUrl: "https://node.test/transactions-api/v1",
    fetchImpl: relay.fetchImpl,
    gasless: true,
    wait: {
      timeoutMs: 30,
      pollMs: 10,
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
    },
  };
  const part = partitionCart([{ line: lines[0]!, offer }], session, { report: noop });
  const review = await reviewCart([{ line: lines[0]!, offer }], session, BOOK.buyer, resolvers());
  store.startCheckout(part.groups);
  return {
    deps: { ctx, store, guard: createSpendGuard() },
    store,
    group: part.groups[0]!,
    resolved: new Map(review.buyable.map((r) => [r.line.id, r])),
    shop,
    chain,
    wallet,
    relay,
    storage,
  };
}

const statusOf = (h: Harness) => h.store.getSnapshot().checkout!.groups[0]!;

describe("running a credits group through the state machine", () => {
  it("authorizes ONE credit for the group with the server's quoted cents, relays useCredits and confirms", async () => {
    const h = await harness();
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "confirmed" });
    expect(statusOf(h)).toMatchObject({ status: "confirmed", creditIds: [`0x${"a1".repeat(32)}`] });
    const auth = h.shop.log.find((r) => r.name === "authorizeCreditBatch")!;
    expect(auth.body).toEqual({ items: [{ usdPriceCents: 100, tradeId: "trade-creator-1", contractAddress: BOOK.collection, itemId: "7" }], source: "website" });
    expect(auth.req.url).toBe("https://node.test/shop-api/v1/credit-authorizations/batch");
    expect(h.relay.posts).toHaveLength(1);
    await vi.waitFor(() => expect(h.shop.log.some((r) => r.name === "reportCreditSubmission")).toBe(true));
    expect(h.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
  });

  it("wallet rejection: the group fails as rejected and its unbroadcast credit is released", async () => {
    const h = await harness();
    h.wallet.next = "reject";
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "failed", failure: { kind: "rejected" } });
    expect(statusOf(h).status).toBe("failed");
    expect(h.shop.log.find((r) => r.name === "cancelCreditAuthorizations")?.body).toEqual({ salts: [`0x${"a1".repeat(32)}`] });
    expect(h.relay.posts).toHaveLength(0);
  });

  it("insufficient credits: a refused group with an explicit top-up, never a switch to MANA", async () => {
    const h = await harness({ handlers: { authorizeCreditBatch: () => ({ status: 402, body: { error: "Insufficient USD credits", balanceCents: 40, requiredCents: 100 } }) } });
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "failed", failure: { kind: "refused" }, topUp: { requiredCents: 100, balanceCents: 40 } });
    expect(h.store.getSnapshot().lines[0]!.method).toBeNull();
    expect(h.relay.posts).toHaveLength(0);
  });

  it("a mined revert fails the group and releases the credit; nothing leaves the cart", async () => {
    const h = await harness({ confirm: "reverted" });
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "failed", failure: { kind: "reverted" } });
    expect(h.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(true);
    expect(h.store.getSnapshot().lines).toHaveLength(1);
  });

  it("no receipt in the window: pending, reservation kept; a late receipt confirms it on watch", async () => {
    const h = await harness({ confirm: "none" });
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "pending" });
    expect(statusOf(h).status).toBe("pending");
    expect(h.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    h.chain.receipts.set(statusOf(h).txHash!.toLowerCase(), "success");
    expect(await watchGroup(h.deps, statusOf(h))).toMatchObject({ status: "confirmed" });
    expect(statusOf(h).status).toBe("confirmed");
  });

  it("a relay that gave no usable answer after the hand-off stays submitted; the credit's record settles it", async () => {
    const h = await harness({
      relay: "down",
      handlers: {
        userPurchases: () => ({ purchases: [{ id: `0x${"a1".repeat(32)}`, status: "SETTLED", usdCents: 100, createdAt: 1, txHash: `0x${"b".repeat(64)}` }] }),
      },
    });
    const result = await runGroup(h.deps, h.group, h.resolved);
    expect(result).toMatchObject({ status: "pending", txHash: null });
    expect(statusOf(h).status).toBe("submitted");
    expect(h.shop.log.some((r) => r.name === "cancelCreditAuthorizations")).toBe(false);
    expect(await watchGroup(h.deps, statusOf(h))).toMatchObject({ status: "confirmed", txHash: `0x${"b".repeat(64)}` });
  });
});

describe("process death between boundaries, then resume", () => {
  it("a reloaded page reads the persisted checkout; resume releases an authorizing group's credit and re-runs it", async () => {
    const h = await harness();
    h.store.dispatch(h.group.keyId, { type: "authorize" });
    h.store.dispatch(h.group.keyId, { type: "authorized", creditIds: ["0xdead"] });
    const reloaded = createCartStore({ storage: () => h.storage });
    reloaded.scopeTo(BOOK.buyer);
    const persisted = reloaded.getSnapshot().checkout!;
    expect(persisted.groups[0]).toMatchObject({ status: "authorizing", creditIds: ["0xdead"] });
    const deps = { ...h.deps, store: reloaded };
    const results = await resumeCheckout(deps, persisted, [h.group], h.resolved);
    expect(h.shop.log.find((r) => r.name === "cancelCreditAuthorizations")?.body).toEqual({ salts: ["0xdead"] });
    expect(results[h.group.keyId]).toMatchObject({ status: "confirmed" });
    expect(reloaded.getSnapshot().checkout!.groups[0]!.attempts).toBe(2);
  });

  it("a group that may have been broadcast is only watched on resume, never re-run", async () => {
    const h = await harness({ confirm: "none" });
    await runGroup(h.deps, h.group, h.resolved);
    const before = h.relay.posts.length;
    const state = h.store.getSnapshot().checkout!;
    const results = await resumeCheckout(h.deps, state, [h.group], h.resolved);
    expect(results[h.group.keyId]).toMatchObject({ status: "pending" });
    expect(h.relay.posts).toHaveLength(before);
  });
});

describe("running a MANA group", () => {
  it("a self-custody wallet approves MANA spending explicitly; declining buys nothing", async () => {
    const h = await harness({ method: "mana" });
    const requestApproval = vi.fn(async () => false);
    const result = await runGroup({ ...h.deps, requestApproval }, h.group, h.resolved);
    expect(requestApproval).toHaveBeenCalledWith(expect.objectContaining({ index: 1, total: 1, auth: expect.objectContaining({ spenderAddress: BOOK.marketplace.contract }) }));
    expect(result).toMatchObject({ status: "failed", failure: { kind: "rejected" } });
    expect(h.relay.posts).toHaveLength(0);
  });

  it("approved, the accept is relayed and confirmed; a sold listing reverts and fails the group", async () => {
    const h = await harness({ method: "mana" });
    const requestApproval = vi.fn(async ({ auth, requiredWei }: { auth: { contractAddress: string; spenderAddress: string; chainId: number }; requiredWei: bigint }) => {
      h.chain.allowances.set(`${auth.chainId}:${BOOK.buyer}:${auth.spenderAddress}`, requiredWei);
      return true;
    });
    expect(await runGroup({ ...h.deps, requestApproval }, h.group, h.resolved)).toMatchObject({ status: "confirmed" });
    const sold = await harness({ method: "mana", confirm: "reverted" });
    sold.chain.allowances.set(`137:${BOOK.buyer}:${BOOK.marketplace.contract}`, 10n ** 20n);
    expect(await runGroup(sold.deps, sold.group, sold.resolved)).toMatchObject({ status: "failed", failure: { kind: "reverted" } });
  });
});

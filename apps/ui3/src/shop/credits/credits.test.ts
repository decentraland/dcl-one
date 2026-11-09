import { afterEach, describe, expect, it, vi } from "vitest";

import { createFakeShop } from "../checkout/fakes";
import { creditsNoun, creditsSrLabel, formatAmount, usdCentsToCredits, usdCentsToCreditsFloor } from "../checkout/lib/currency";
import {
  cancelUsdIntents,
  creditOrderPill,
  decodeUsdBalance,
  fetchUserCreditOrders,
  fetchUserPurchases,
  getUsdBalance,
  resumeCreditOrder,
  authorizeUsdCredit,
  reportIntentSubmission,
} from "./credits";
import { nextBalancePoll } from "./hooks";
import { CREDIT_PACKS, createPackCheckout, offerablePacks, packBonus, fetchCreditPacks, creditsForUsd, pollCreditGrant, usdForCredits } from "./payments";
import { pollCreditGrantReal } from "./payments-stripe";
import { parseSseChunk } from "./program";
import { RESUME_CART_KEY } from "../checkout/purchase/cart";
import { stashNameResume } from "../names/resume";
import { bindBuyNowTopUp, bindTopUpReturn, clearTopUpHandoffs, dropBuyNowTopUp, keepHandoffsOf, stashBuyNowTopUp, stashTopUpReturn, takeBuyNowReturn, takeTopUpReturn } from "./returns";

afterEach(() => vi.unstubAllGlobals());

describe("the credits account", () => {
  it("a balance is whole credits, floored: 107 cents is 10 credits", async () => {
    expect(decodeUsdBalance({ balanceCents: 107, credits: 11 })).toEqual({ balanceCents: 107, credits: 10 });
    expect(decodeUsdBalance({ balanceCents: "250", held: { cents: 90, credits: 9, releasesAtSeconds: null, purchases: [{ credits: 9 }] } })).toMatchObject({
      credits: 25,
      held: { credits: 9, releasesAtSeconds: null },
    });
    expect(decodeUsdBalance(null)).toEqual({ balanceCents: 0, credits: 0 });
    const shop = createFakeShop({ userCredits: () => ({ credits: [], totalCredits: 0, totals: { expiring: 0, nonExpiring: 0 }, usd: { balanceCents: 1234 } }) });
    expect(await getUsdBalance("0xABC", shop.client)).toEqual({ balanceCents: 1234, credits: 123 });
    expect(shop.log[0]!.req.url).toBe("https://node.test/shop-api/v1/users/0xabc/credits");
    expect(shop.log[0]!.req.signed).toBe(true);
  });

  it("purchase history normalises both settlement-hash spellings and older servers", async () => {
    const shop = createFakeShop({
      userPurchases: () => ({
        purchases: [
          { id: "a", status: "SETTLED", usdCents: 115, credits: 12, createdAt: 2, transactionHash: "0xaa" },
          { id: "b", status: "EXPIRED", usdCents: 50, createdAt: 1, submittedTxHash: "0xbb" },
          { id: "c", status: "WEIRD" },
        ],
        total: 3,
      }),
    });
    const { items, total } = await fetchUserPurchases("0xabc", { all: true, first: 24 }, shop.client);
    expect(total).toBe(3);
    expect(items).toEqual([
      expect.objectContaining({ id: "a", txHash: "0xaa", credits: 12, contractAddress: null }),
      expect.objectContaining({ id: "b", status: "EXPIRED", submittedTxHash: "0xbb", credits: 5 }),
    ]);
    expect(shop.log[0]!.query.get("status")).toBe("all");
    expect(shop.log[0]!.query.get("limit")).toBe("24");
  });

  it("credit orders never throw and speak credits-server's vocabulary", async () => {
    const down = createFakeShop({ userCreditOrders: () => ({ status: 503, body: { error: "unavailable" } }) });
    expect(await fetchUserCreditOrders("0xabc", {}, down.client)).toEqual({ items: [], total: 0, payouts: [] });
    expect(creditOrderPill("credited")).toBe("SETTLED");
    expect(creditOrderPill("initiated")).toBe("UNFINISHED");
    expect(creditOrderPill("crediting")).toBe("PENDING");
    expect(creditOrderPill("abandoned")).toBe("FAILED");
  });

  it("resuming a pack checkout says four different things", async () => {
    const answer = (status: number, body: unknown) => createFakeShop({ resumePackOrder: () => ({ status, body }) }).client;
    expect(await resumeCreditOrder("o", answer(200, { url: "https://checkout.stripe.com/x" }))).toEqual({ kind: "url", url: "https://checkout.stripe.com/x" });
    expect(await resumeCreditOrder("o", answer(409, { status: "processing" }))).toEqual({ kind: "paid" });
    expect(await resumeCreditOrder("o", answer(409, { status: "abandoned" }))).toEqual({ kind: "expired" });
    expect(await resumeCreditOrder("o", answer(502, { error: "stripe" }))).toEqual({ kind: "unavailable" });
  });

  it("release and submission reports go through the canonical routes, salts de-duplicated", async () => {
    const shop = createFakeShop({
      cancelCreditAuthorizations: ({ body }) => ({ released: (body as { salts: string[] }).salts.length, retired: 0, spendableAtSeconds: null }),
      reportCreditSubmission: () => ({ recorded: 1 }),
      authorizeCredit: ({ body }) => ({ credit: { id: "0x1" }, maxCreditedValue: "1", usdCents: (body as { usdPriceCents: number }).usdPriceCents, oracleRate: "1" }),
    });
    expect(await cancelUsdIntents(["0x1", "0x1", "0x2"], shop.client)).toBe(2);
    expect(await cancelUsdIntents([], shop.client)).toBe(0);
    expect(await reportIntentSubmission(["0x1"], "0xhash", shop.client)).toBe(1);
    await authorizeUsdCredit(1000, { registeredName: "Nacho" }, shop.client);
    expect(shop.log.map((r) => r.req.url)).toEqual([
      "https://node.test/shop-api/v1/credit-authorizations/cancel",
      "https://node.test/shop-api/v1/credit-authorizations/submitted",
      "https://node.test/shop-api/v1/credit-authorizations",
    ]);
    expect(shop.log[2]!.body).toEqual({ usdPriceCents: 1000, name: "Nacho", source: "website" });
  });

  it("the balance polls only while credits are held, aimed at their release", () => {
    expect(nextBalancePoll({ balanceCents: 0 as never, credits: 0 })).toBe(false);
    expect(nextBalancePoll({ balanceCents: 0 as never, credits: 0, held: { cents: 10 as never, credits: 1, releasesAtSeconds: null, purchases: [] } })).toBe(15_000);
    expect(nextBalancePoll({ balanceCents: 0 as never, credits: 0, held: { cents: 10 as never, credits: 1, releasesAtSeconds: 105, purchases: [] } }, 100_000)).toBe(6_000);
  });
});

describe("packs", () => {
  it("offers only packs that finish the purchase and recommends the cheapest that does", () => {
    const shortfall = offerablePacks(CREDIT_PACKS, 57);
    expect(shortfall.packs.map((p) => p.credits)).toEqual([100, 260, 540]);
    expect(shortfall).toMatchObject({ recommended: { credits: 100 }, closesGap: true });
    const huge = offerablePacks(CREDIT_PACKS, 5000);
    expect(huge).toMatchObject({ closesGap: false, recommended: { credits: 540 } });
    expect(huge.packs).toHaveLength(4);
  });

  it("states a bonus only against the entry pack's real rate", () => {
    expect(packBonus(CREDIT_PACKS[0]!, CREDIT_PACKS)).toBeNull();
    expect(packBonus(CREDIT_PACKS[1]!, CREDIT_PACKS)).toEqual({ baseline: 80, bonus: 20 });
    expect(creditsForUsd(1.2)).toBe(12);
    expect(usdForCredits(12)).toBe(1.2);
  });

  it("reads the server catalogue in order, webp first, and starts a hosted checkout with the zone", async () => {
    const shop = createFakeShop({
      packs: () => ({ packs: [{ id: "b", usd: 2, credits: 20, order: 2 }, { id: "a", usd: 1, credits: 10, order: 1, recommended: true, imageUrl: "a.png", imageUrlWebp: "a.webp" }] }),
      createPackOrder: () => ({ orderId: "order-1", url: "https://checkout.stripe.com/c/1" }),
    });
    expect(await fetchCreditPacks(shop.client)).toEqual([
      { id: "a", usd: 1, credits: 10, bestValue: true, artUrl: "a.webp" },
      { id: "b", usd: 2, credits: 20 },
    ]);
    expect(await createPackCheckout("a", shop.client)).toEqual({ orderId: "order-1", url: "https://checkout.stripe.com/c/1", mock: false });
    const order = shop.log.find((r) => r.name === "createPackOrder")!;
    expect(order.req.url).toBe("https://node.test/shop-api/v1/pack-orders");
    expect(order.body).toMatchObject({ packId: "a", source: "website" });
  });

  it("the grant poll: a 404 right after the return is lag, money in past the deadline is 'pending', unpaid stays 'initiated'", async () => {
    let calls = 0;
    const lagging = createFakeShop({
      packOrder: () => {
        calls += 1;
        return calls < 3 ? { status: 404, body: { error: "not found" } } : { status: "credited", creditsGranted: 100, newBalance: 100 };
      },
    });
    expect(await pollCreditGrantReal("o", { intervalMs: 1 }, lagging.client)).toMatchObject({ status: "credited", creditsGranted: 100 });
    let t = 0;
    const now = () => (t += 1000);
    const processing = createFakeShop({ packOrder: () => ({ status: "processing" }) });
    expect(await pollCreditGrantReal("o", { intervalMs: 1, timeoutMs: 2500, now }, processing.client)).toEqual({ status: "pending" });
    t = 0;
    const unpaid = createFakeShop({ packOrder: () => ({ status: "initiated" }) });
    expect(await pollCreditGrantReal("o", { intervalMs: 1, timeoutMs: 2500, now }, unpaid.client)).toEqual({ status: "initiated" });
  });

  it("a mock order id is granted only with mock payments on; otherwise the server is asked, so a link shows no Credits never bought", async () => {
    const shop = createFakeShop({ packOrder: () => ({ status: 404, body: { error: "not found" } }) });
    let t = 0;
    const now = () => (t += 1000);
    // The server does not know it: past the grace it is no order at all.
    expect(await pollCreditGrant("mock_cs_pack_10_1", { intervalMs: 1, now }, shop.client)).toEqual({ status: "missing" });
    expect(shop.log.some((r) => r.name === "packOrder")).toBe(true);
    vi.stubGlobal("__SHOP_PAYMENTS__", "mock");
    const asked = shop.log.length;
    expect(await pollCreditGrant("mock_cs_pack_10_1", { intervalMs: 1 }, shop.client)).toMatchObject({ status: "credited" });
    // Granted without asking the server.
    expect(shop.log.length).toBe(asked);
  });

  it("an order the server never finds is missing past the grace a lagging replica needs; one it saw stays pending at the deadline", async () => {
    let t = 0;
    const now = () => (t += 1000);
    const unknown = createFakeShop({ packOrder: () => ({ status: 404, body: { error: "not found" } }) });
    expect(await pollCreditGrantReal("o", { intervalMs: 1, missingGraceMs: 5000, now }, unknown.client)).toEqual({ status: "missing" });
    expect(unknown.log.filter((r) => r.name === "packOrder").length).toBeLessThan(8);
    // Seen once (money in), then a 404 at the deadline: not "missing", the payment may still land.
    t = 0;
    let calls = 0;
    const flaky = createFakeShop({ packOrder: () => (++calls === 1 ? { status: "processing" } : { status: 404, body: { error: "not found" } }) });
    expect(await pollCreditGrantReal("o", { intervalMs: 1, timeoutMs: 20_000, missingGraceMs: 5000, now }, flaky.client)).toEqual({ status: "pending" });
  });
});

describe("hand-offs and the stream", () => {
  it("a top-up return is one entry, same-origin /shop only, cleared when a top-up ends without resuming", () => {
    stashTopUpReturn("https://evil.example/shop");
    expect(takeTopUpReturn()).toBeNull();
    stashTopUpReturn("/shop/cart");
    expect(takeTopUpReturn()?.path).toBe("/shop/cart");
    expect(takeTopUpReturn()).toBeNull();
    stashTopUpReturn("/shop/cart");
    clearTopUpHandoffs();
    expect(takeTopUpReturn()).toBeNull();
  });

  describe("Buy now's record of what the buyer confirmed", () => {
    const PATH = "/shop/item/0xC0/7";
    const BUYER = "0xAbC0000000000000000000000000000000000001";
    const T0 = 1_700_000_000_000;
    /** Buy now leaving for the pack checkout, which created `order` (none when it never answered). */
    const leave = (at = T0, order: string | null = "order-1") => {
      const attempt = stashBuyNowTopUp({ path: PATH, account: BUYER, lineId: "line-7", credits: 12 }, at);
      if (order) bindBuyNowTopUp(attempt, order);
      return attempt;
    };

    it("comes back to the item page for that order's grant, for the same page and account, once", () => {
      leave();
      expect(takeTopUpReturn(T0, "order-1")?.path).toBe(`${PATH}?buy=resume`);
      expect(takeBuyNowReturn(PATH.toUpperCase().replace("/SHOP/ITEM", "/shop/item"), BUYER.toLowerCase(), "order-1", T0 + 1000)).toMatchObject({ account: BUYER.toLowerCase(), lineId: "line-7", credits: 12 });
      expect(takeBuyNowReturn(PATH, BUYER, "order-1", T0 + 1000)).toBeNull();
    });

    it("another order's grant (the buyer walked away from this top-up) takes nothing back, and drops both records", () => {
      leave();
      expect(takeTopUpReturn(T0, "order-2")).toBeNull();
      expect(takeBuyNowReturn(PATH, BUYER, "order-1", T0)).toBeNull();
    });

    it("is nothing for another page, another account, no account, another or no order, unbound, or two hours old or dated ahead", () => {
      for (const [path, account, order, at, bound] of [
        ["/shop/item/0xC0/8", BUYER, "order-1", T0, "order-1"],
        [PATH, "0x0000000000000000000000000000000000000002", "order-1", T0, "order-1"],
        [PATH, null, "order-1", T0, "order-1"],
        [PATH, BUYER, "order-2", T0, "order-1"],
        [PATH, BUYER, null, T0, "order-1"],
        [PATH, BUYER, null, T0, null],
        [PATH, BUYER, "order-1", T0 + 2 * 60 * 60 * 1000 + 1, "order-1"],
        [PATH, BUYER, "order-1", T0 - 1, "order-1"],
      ] as const) {
        leave(T0, bound);
        expect(takeBuyNowReturn(path, account, order, at)).toBeNull();
      }
    });

    it("an attempt drops or binds only what it wrote; any other top-up, or an ended one, drops it", () => {
      const first = leave(T0, null);
      const second = leave(T0 + 10, null);
      dropBuyNowTopUp(first);
      bindBuyNowTopUp(first, "order-old");
      bindBuyNowTopUp(second, "order-2");
      expect(takeBuyNowReturn(PATH, BUYER, "order-2", T0 + 20)).toMatchObject({ attempt: second });
      leave();
      stashTopUpReturn("/shop/cart");
      expect(takeBuyNowReturn(PATH, BUYER, "order-1", T0)).toBeNull();
      leave();
      clearTopUpHandoffs();
      expect(takeTopUpReturn(T0, "order-1")).toBeNull();
      expect(takeBuyNowReturn(PATH, BUYER, "order-1", T0)).toBeNull();
      leave();
      stashNameResume("nacho");
      expect(takeBuyNowReturn(PATH, BUYER, "order-1", T0)).toBeNull();
    });

    it("the cart's top-up, bound to its order, comes back for that order (also when continued) and for no other", () => {
      stashTopUpReturn("/shop/cart", T0);
      bindTopUpReturn("order-c");
      expect(takeTopUpReturn(T0, "order-c")).toEqual({ path: "/shop/cart", at: T0, orderId: "order-c" });
      // Continued later, the basket may have changed: it comes back, waiting for the buyer's press, and nothing auto-pays it.
      stashTopUpReturn("/shop/cart", T0);
      sessionStorage.setItem(RESUME_CART_KEY, "1");
      bindTopUpReturn("order-c");
      keepHandoffsOf("order-c");
      expect(sessionStorage.getItem(RESUME_CART_KEY)).toBeNull();
      expect(takeTopUpReturn(T0, "order-c")).toEqual({ path: "/shop/cart", at: T0, orderId: "order-c", confirmAgain: true });
      stashTopUpReturn("/shop/cart", T0);
      bindTopUpReturn("order-c");
      keepHandoffsOf("order-x");
      expect(takeTopUpReturn(T0, "order-c")).toBeNull();
      stashTopUpReturn("/shop/cart", T0);
      bindTopUpReturn("order-c");
      expect(takeTopUpReturn(T0, "order-x")).toBeNull();
    });

    it("drops a cart top-up the buyer walked away from: the basket is never paid after Buy now's", () => {
      sessionStorage.setItem(RESUME_CART_KEY, "1");
      leave();
      expect(sessionStorage.getItem(RESUME_CART_KEY)).toBeNull();
    });
  });

  it("parses server-sent events across chunk boundaries", () => {
    const a = parseSseChunk('data: {"totalCredits":1}\n\ndata: {"tot');
    expect(a.events).toEqual(['{"totalCredits":1}']);
    const b = parseSseChunk(`${a.rest}alCredits":2}\n\n`);
    expect(b.events).toEqual(['{"totalCredits":2}']);
  });
});

describe("the credit unit", () => {
  it("one credit is ten cents: a price rounds up, a balance down, and the noun agrees with a formatted amount", () => {
    expect(usdCentsToCredits(115)).toBe(12);
    expect(usdCentsToCreditsFloor(115)).toBe(11);
    expect(formatAmount(1)).toBe("1 Credit");
    expect(formatAmount(270)).toBe("270 Credits");
    expect(creditsNoun("1")).toBe("credit");
    expect(creditsNoun("1,250", true)).toBe("Credits");
    expect(creditsSrLabel(1)).toBe(" credit");
    expect(creditsSrLabel("12")).toBe(" credits");
  });
});

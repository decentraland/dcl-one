import { describe, expect, it } from "vitest";

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
import { CREDIT_PACKS, createPackCheckout, offerablePacks, packBonus, fetchCreditPacks, creditsForUsd, usdForCredits } from "./payments";
import { pollCreditGrantReal } from "./payments-stripe";
import { parseSseChunk } from "./program";
import { clearTopUpHandoffs, stashTopUpReturn, takeTopUpReturn } from "./returns";

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

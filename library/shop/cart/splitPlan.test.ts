import { describe, expect, it } from "vitest";

import { chargeForQuote, manaFromWhole, usdCents, type ManaWei, type UsdCents } from "../model/money";
import type { CartLine, PurchaseOffer } from "../model/purchase";
import type { MissingFact, PaymentMethod, Quote } from "../model/settlement";
import { creatorOrder, ITEM, parcelResale, SIGNED_OUT, storeMint, web2, web3 } from "../payment/fixtures";
import { lineUnits, type PricedLine } from "./partition";
import { planCartSplit, type CartSplitResult } from "./splitPlan";

const s = web3();
let nextItem = ITEM;

type Terms = { cents: number | null; mana?: number | null; splittable?: boolean; available?: number | null };

/** A store mint (Polygon, one Credits group for all of them) with the given quotes; null means that method is out. */
function mint({ cents, mana = 1, splittable = true, available = null }: Terms): PurchaseOffer {
  const base = storeMint(s, "wearable", nextItem++);
  const quotes: Quote[] = [];
  const unavailable: { method: PaymentMethod; missing: MissingFact }[] = [];
  if (cents === null) unavailable.push({ method: "credits", missing: "manaUsdRate" });
  else quotes.push({ method: "credits", quoted: usdCents(cents) });
  if (mana === null) unavailable.push({ method: "mana", missing: "manaUsdRate" });
  else quotes.push({ method: "mana", amount: manaFromWhole(mana) });
  return { ...base, quotes, unavailable, available, splittable };
}

function line(id: string, offer: PurchaseOffer | null, quantity = 1, method: PaymentMethod | null = null): PricedLine {
  const asset = offer?.asset ?? storeMint(s).asset;
  return {
    line: {
      id,
      asset,
      quantity,
      method,
      listing: {},
      display: { name: id },
      provenance: {},
      seen: null,
      addedAt: 0,
    } satisfies CartLine,
    offer,
  };
}

const plan = (cart: PricedLine[], credits: number, mana?: ManaWei | null): CartSplitResult =>
  planCartSplit(cart, s, { creditsBalance: usdCents(credits), manaBalance: mana });

function planned(result: CartSplitResult) {
  if (!result.ok) throw new Error(`no plan: ${JSON.stringify(result.refusal)}`);
  return result.plan;
}

describe("planCartSplit: a cart's Credits and MANA parts", () => {
  it("charges each unit up to a whole credit: two units quoted 11 cents cost 40, which 30 does not cover", () => {
    const cart = [line("pair", mint({ cents: 11 }), 2), line("one", mint({ cents: 20, splittable: false }))];
    expect(planned(plan(cart, 30))).toEqual({ credits: ["one"], split: null, mana: ["pair"], creditsCents: 20 });
    expect(planned(plan(cart, 40)), "40 covers the pair, and nothing is left for the next line").toEqual({
      credits: ["pair"],
      split: null,
      mana: ["one"],
      creditsCents: 40,
    });
  });

  it("a line only MANA can pay goes to MANA, however many Credits are left", () => {
    const cart = [line("manaOnly", mint({ cents: null })), line("parcel", parcelResale(s)), line("credits", mint({ cents: 100 }))];
    expect(planned(plan(cart, 10_000))).toEqual({ credits: ["credits"], split: null, mana: ["manaOnly", "parcel"], creditsCents: 100 });
  });

  it("a line neither method can pay leaves the cart without a split", () => {
    const ok = line("ok", mint({ cents: 100 }));
    expect(plan([ok, line("gone", null)], 50)).toEqual({ ok: false, refusal: { reason: "noMethod", lineId: "gone" } });
    expect(plan([ok, line("priceless", mint({ cents: null, mana: null }))], 50)).toEqual({
      ok: false,
      refusal: { reason: "noMethod", lineId: "priceless" },
    });
    expect(plan([ok, line("soldOut", mint({ cents: 100, available: 1 }), 2)], 50), "not enough stock for its units").toEqual({
      ok: false,
      refusal: { reason: "noMethod", lineId: "soldOut" },
    });
  });

  it("covers the lines only Credits can pay first, or offers no split", () => {
    const cart = [line("either", mint({ cents: 100 })), line("creditsOnly", mint({ cents: 120, mana: null }))];
    expect(planned(plan(cart, 150)), "the Credits-only line first, though it comes second").toEqual({
      credits: ["creditsOnly"],
      split: { lineId: "either", legCents: 30, priceCents: 100 },
      mana: [],
      creditsCents: 150,
    });
    expect(plan(cart, 110)).toEqual({ ok: false, refusal: { reason: "creditsShort", required: 120, balance: 110 } });
  });

  it("a line of several units that does not fit goes to MANA whole, never split by units, and the walk goes on", () => {
    const cart = [line("first", mint({ cents: 100 })), line("three", mint({ cents: 115 }), 3), line("after", mint({ cents: 90 }))];
    expect(planned(plan(cart, 200))).toEqual({ credits: ["first", "after"], split: null, mana: ["three"], creditsCents: 190 });
  });

  it("a line that does not fit and that the node does not split goes to MANA, and the walk goes on", () => {
    const cart = [line("big", mint({ cents: 500, splittable: false })), line("a", mint({ cents: 115 })), line("b", mint({ cents: 201 }))];
    expect(planned(plan(cart, 150))).toEqual({
      credits: ["a"],
      split: { lineId: "b", legCents: 30, priceCents: 201 },
      mana: ["big"],
      creditsCents: 150,
    });
    const usdPegged = [line("pegged", creatorOrder(s)), line("store", storeMint(s))];
    expect(planned(plan(usdPegged, 50)), "a USD-pegged listing is never split").toEqual({
      credits: [],
      split: { lineId: "store", legCents: 50, priceCents: 115 },
      mana: ["pegged"],
      creditsCents: 50,
    });
  });

  it("the first line that does not fit and can be split takes the whole credits left, and MANA pays every line after it", () => {
    const cart = [
      line("a", mint({ cents: 115 })),
      line("b", mint({ cents: 201 })),
      line("c", mint({ cents: 115 })),
      line("d", mint({ cents: 20, mana: null })),
      line("e", mint({ cents: 5 })),
    ];
    // d (Credits only) is covered first: 257 - 20 leaves 237. a costs 120 (117 left); b's 210 does not fit, so it takes 110.
    expect(planned(plan(cart, 257))).toEqual({
      credits: ["a", "d"],
      split: { lineId: "b", legCents: 110, priceCents: 201 },
      mana: ["c", "e"],
      creditsCents: 250,
    });
  });

  it("splits only for a Credits part above nothing and below the line's own quote", () => {
    const one = (cents: number) => [line("one", mint({ cents }))];
    expect(plan(one(115), 9), "less than a whole credit left").toEqual({ ok: false, refusal: { reason: "noCreditsPart" } });
    expect(plan(one(115), 120), "enough for the whole line").toEqual({ ok: false, refusal: { reason: "noManaPart" } });
    expect(planned(plan(one(115), 117)), "115 rounds up to 120, so 117 does not cover it: 110 of it").toEqual({
      credits: [],
      split: { lineId: "one", legCents: 110, priceCents: 115 },
      mana: [],
      creditsCents: 110,
    });
    expect(planned(plan(one(111), 119)).split, "against the quote, not its rounded-up charge").toEqual({
      lineId: "one",
      legCents: 110,
      priceCents: 111,
    });
  });

  it("Credits alone or MANA alone is no split: the cart pays as it does today", () => {
    const cart = [line("a", mint({ cents: 100 })), line("b", mint({ cents: 200 }))];
    expect(plan(cart, 300)).toEqual({ ok: false, refusal: { reason: "noManaPart" } });
    expect(plan(cart, 0)).toEqual({ ok: false, refusal: { reason: "noCreditsPart" } });
    expect(plan([line("m", mint({ cents: null })), line("p", parcelResale(s))], 10_000)).toEqual({
      ok: false,
      refusal: { reason: "noCreditsPart" },
    });
  });

  it("only a session with a web3 wallet has a split, a Credits-and-MANA-lines one included", () => {
    for (const session of [web2(), SIGNED_OUT]) {
      // A web2 session's MANA quote is still served, with a wallet required: a cart could otherwise put the parcel in MANA.
      const cart = [line("mint", storeMint(session)), line("parcel", parcelResale(session))];
      expect(planCartSplit(cart, session, { creditsBalance: usdCents(120) }), session.kind).toEqual({
        ok: false,
        refusal: { reason: "noWallet" },
      });
    }
    expect(planned(plan([line("mint", storeMint(s)), line("parcel", parcelResale(s))], 120))).toEqual({
      credits: ["mint"],
      split: null,
      mana: ["parcel"],
      creditsCents: 120,
    });
  });

  it("keeps every Credits group within 30 units, naming the lines where they sit in the cart", () => {
    const cart = [line("m", mint({ cents: null })), line("a", mint({ cents: 10 }), 16), line("b", mint({ cents: 10 }), 16)];
    const refused = (result: CartSplitResult) => {
      if (result.ok || result.refusal.reason !== "refused") throw new Error(`expected refused lines: ${JSON.stringify(result)}`);
      return result.refusal.refusals.map((r) => [r.lineId, r.index, r.code]);
    };
    expect(refused(plan(cart, 10_000))).toEqual([
      ["a", 1, "groupTooLarge"],
      ["b", 2, "groupTooLarge"],
    ]);
    expect(planned(plan([...cart.slice(0, 2), line("b", mint({ cents: 10 }), 14)], 10_000)).credits, "30 units fit").toEqual(["a", "b"]);

    const alone = [line("one", mint({ cents: 10 })), line("many", mint({ cents: 10 }), 31), line("m", mint({ cents: null }))];
    expect(refused(plan(alone, 320)), "a line over the cap on its own, within the balance").toEqual([
      ["one", 0, "groupTooLarge"],
      ["many", 1, "groupTooLarge"],
    ]);
    expect(planned(plan(alone, 100)), "one the balance does not cover goes to MANA, which has no cap").toEqual({
      credits: ["one"],
      split: null,
      mana: ["many", "m"],
      creditsCents: 10,
    });
  });

  it("gives no plan on a sum that does not fit, across groups included", () => {
    const huge = usdCents(5_000_000_000_000_000);
    const creditsOnly = (offer: PurchaseOffer): PurchaseOffer => ({
      ...offer,
      quotes: [{ method: "credits", quoted: huge }],
      unavailable: [{ method: "mana", missing: "manaUsdRate" }],
    });
    const store = creditsOnly(mint({ cents: 10 }));
    const market = creditsOnly(creatorOrder(s));
    const cents = [line("store", store), line("market", market), line("m", mint({ cents: null }))];
    expect(plan(cents, Number.MAX_SAFE_INTEGER)).toEqual({ ok: false, refusal: { reason: "overflow" } });
    const twice = [line("twice", creditsOnly(mint({ cents: 10 })), 2), line("m", mint({ cents: null }))];
    expect(plan(twice, 100), "one line's own charge, not a line no method pays").toEqual({ ok: false, refusal: { reason: "overflow" } });

    const maxWei = ((1n << 256n) - 1n) as ManaWei;
    const manaOnly = (offer: PurchaseOffer): PurchaseOffer => ({ ...offer, quotes: [{ method: "mana", amount: maxWei }] });
    const wei = [line("k", mint({ cents: 100 })), line("mint", manaOnly(mint({ cents: null }))), line("parcel", manaOnly(parcelResale(s)))];
    expect(plan(wei, 100, maxWei)).toEqual({ ok: false, refusal: { reason: "overflow" } });
    expect(plan(wei, 100), "with no MANA balance to check either").toEqual({ ok: false, refusal: { reason: "overflow" } });
  });

  it("places every line once with all its units, and takes no more Credits than the balance", () => {
    const quotes = [7, 10, 11, 20, 95, 115, 201, 499];
    const outcomes = new Map<string, number>();
    for (let seed = 0; seed < 400; seed++) {
      const cart = Array.from({ length: 1 + (seed % 6) }, (_, i) => {
        const pick = (seed * 7 + i * 13) % 97;
        const cents = pick % 11 === 0 ? null : quotes[pick % quotes.length]!;
        const mana = pick % 7 === 0 && cents !== null ? null : 1 + (pick % 3);
        return line(`l${i}`, mint({ cents, mana, splittable: pick % 5 !== 0 }), 1 + (pick % 4 === 0 ? pick % 3 : 0));
      });
      const balance = (seed * 37) % 900;
      const result = plan(cart, balance);
      const outcome = result.ok ? (result.plan.split ? "split" : "whole") : result.refusal.reason;
      outcomes.set(outcome, (outcomes.get(outcome) ?? 0) + 1);
      if (!result.ok) continue;
      const { credits, split, mana, creditsCents } = result.plan;
      const placed = [...credits, ...(split ? [split.lineId] : []), ...mana];
      expect(placed.slice().sort()).toEqual(cart.map((p) => p.line.id).sort());
      expect(credits.length > 0 || split !== null).toBe(true);
      expect(mana.length > 0 || split !== null).toBe(true);
      const byId = new Map(cart.map((p) => [p.line.id, p]));
      if (split) {
        const entry = byId.get(split.lineId)!;
        expect(lineUnits(entry.line)).toBe(1);
        expect(split.legCents % 10).toBe(0);
        expect(split.legCents > 0 && split.legCents < split.priceCents).toBe(true);
      }
      const charged = credits.reduce((sum, id) => {
        const entry = byId.get(id)!;
        const quote = entry.offer!.quotes.find((q) => q.method === "credits")!;
        return sum + chargeForQuote(quote.method === "credits" ? quote.quoted : usdCents(0))! * lineUnits(entry.line);
      }, 0);
      expect(creditsCents).toBe(charged + (split?.legCents ?? 0));
      expect(creditsCents).toBeLessThanOrEqual(balance);
    }
    // A planner that refused every cart would check nothing above. So at least 50 of these carts must get a plan with a split
    // line and 50 one without, and the rest must be refused for reasons these carts can give (no line lacks both methods, none
    // sits in a group over the cap, no MANA balance is passed).
    expect([...outcomes.keys()].filter((o) => !["split", "whole", "creditsShort", "noCreditsPart", "noManaPart"].includes(o))).toEqual([]);
    expect(outcomes.get("split") ?? 0, "plans with a split line").toBeGreaterThanOrEqual(50);
    expect(outcomes.get("whole") ?? 0, "plans of whole lines only").toBeGreaterThanOrEqual(50);
  });

  it("takes a balance that is not a whole number of cents as no Credits", () => {
    const cart = [line("creditsOnly", mint({ cents: 100, mana: null })), line("either", mint({ cents: 200 }))];
    for (const given of [Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 2, Number.NaN, 12.5, -10]) {
      expect(planCartSplit(cart, s, { creditsBalance: given as UsdCents }), String(given)).toEqual({
        ok: false,
        refusal: { reason: "creditsShort", required: 100, balance: 0 },
      });
    }
  });

  it("decides each line's method itself, whatever method the line was added with", () => {
    const cart = [line("fits", mint({ cents: 100 }), 1, "mana"), line("manaOnly", mint({ cents: null }), 1, "credits")];
    expect(planned(plan(cart, 100))).toEqual({ credits: ["fits"], split: null, mana: ["manaOnly"], creditsCents: 100 });
  });

  it("checks the MANA lines against the wallet, leaving the split line's MANA to the host", () => {
    const cart = [line("k", mint({ cents: 100 })), line("m", mint({ cents: 300, mana: 2 }), 2), line("n", mint({ cents: null, mana: 1 }))];
    expect(planned(plan(cart, 100, manaFromWhole(5))).mana).toEqual(["m", "n"]);
    expect(plan(cart, 100, manaFromWhole(4))).toEqual({
      ok: false,
      refusal: { reason: "manaShort", required: manaFromWhole(5), balance: manaFromWhole(4) },
    });
    expect(planned(plan([line("one", mint({ cents: 115 }))], 50, manaFromWhole(0))).split?.legCents).toBe(50);
  });

  it("an empty cart or a line listed twice has no split", () => {
    expect(plan([], 100)).toEqual({ ok: false, refusal: { reason: "empty" } });
    const twice = mint({ cents: 100 });
    expect(plan([line("x", twice), line("x", twice)], 50)).toEqual({ ok: false, refusal: { reason: "duplicateLine", lineId: "x" } });
  });
});

// Ports the cart tests of tests/shop/policy.rs to the client partition over server offers.

import { describe, expect, it } from "vitest";

import { CHAIN } from "../model/chain";
import { manaFromWhole, usdCents } from "../model/money";
import type { CartLine, PurchaseOffer } from "../model/purchase";
import { MAX_CREDIT_LINES_PER_GROUP, type PaymentMethod, type Session } from "../model/settlement";
import {
  BOOK,
  creatorOrder,
  nameRegistration,
  parcelResale,
  storeMint,
  wearableResale,
  web2,
  web3,
} from "../payment/fixtures";
import { partitionCart, type PricedLine } from "./partition";

function priced(entries: [PurchaseOffer, PaymentMethod | null, number?][]): PricedLine[] {
  return entries.map(([offer, method, quantity], i) => ({
    line: {
      id: `line-${i}`,
      asset: offer.asset,
      quantity: quantity ?? 1,
      method,
      listing: {},
      display: { name: `line ${i}` },
      provenance: {},
      seen: null,
      addedAt: 0,
    } satisfies CartLine,
    offer,
  }));
}

const indexes = (ids: string[]) => ids.map((id) => Number(id.slice("line-".length)));
const quiet = { report: () => {} };

describe("the cart partition (tests/shop/policy.rs)", () => {
  it("a_mixed_cart_partitions_by_method_chain_and_contract", () => {
    const s = web3();
    const cart = priced([
      [storeMint(s, "wearable"), "credits"],
      [wearableResale(s), "mana"],
      [storeMint(s, "emote"), "credits"],
      [nameRegistration(s), "credits"],
      [parcelResale(s), "mana"],
      [creatorOrder(s), "mana"],
      [nameRegistration(s), "mana"],
    ]);
    const got = partitionCart(cart, s, quiet);
    expect(got.ok).toBe(true);
    const name = { kind: "one", asset: nameRegistration(s).asset };
    const parcel = { kind: "one", asset: parcelResale(s).asset };
    expect(got.groups.map((g) => [g.key, indexes(g.lineIds), g.total, g.manaRemainderAllowed])).toEqual([
      [{ method: "credits", chainId: 137, contract: BOOK.store.contract, call: { kind: "storeBuy" } }, [0, 2], { method: "credits", quoted: 316, charged: 330 }, true],
      [{ method: "mana", chainId: 137, contract: BOOK.marketplace.contract, call: { kind: "accept" } }, [1, 5], { method: "mana", amount: manaFromWhole(4) }, false],
      [{ method: "credits", chainId: 137, contract: BOOK.executor.contract, call: name }, [3], { method: "credits", quoted: 5000, charged: 5000 }, true],
      [{ method: "mana", chainId: CHAIN.ETHEREUM, contract: BOOK.ethLegacy.contract, call: parcel }, [4], { method: "mana", amount: manaFromWhole(4000) }, false],
      [{ method: "mana", chainId: CHAIN.ETHEREUM, contract: BOOK.controller.contract, call: name }, [6], { method: "mana", amount: manaFromWhole(100) }, false],
    ]);
    expect(got.totals).toEqual({ credits: { quoted: 5316, charged: 5330 }, mana: manaFromWhole(4104) });
    expect(got.creditsShort).toBeNull();
    const short = partitionCart(cart, s, { ...quiet, creditsBalance: usdCents(5000) });
    expect(short.creditsShort, "the whole-credit charge the balance does not cover").toBe(330);
    expect(short.ok, "a shortfall is a top-up, not a refused or re-routed line").toBe(true);
    expect(short.groups.map((g) => g.key.method)).toEqual(got.groups.map((g) => g.key.method));
  });

  it("a_land_line_mixed_into_a_credits_group_is_refused: named, not moved to MANA or dropped", () => {
    const s = web3();
    const got = partitionCart(
      priced([
        [storeMint(s, "wearable"), "credits"],
        [parcelResale(s), "credits"],
        [storeMint(s, "emote"), "credits"],
      ]),
      s,
      quiet,
    );
    expect(got.ok).toBe(false);
    expect(got.refused).toEqual([{ lineId: "line-1", index: 1, code: "methodNotAllowed", method: "credits", allowed: ["mana"], missing: undefined }]);
    expect(got.groups.flatMap((g) => g.lineIds)).toEqual(["line-0", "line-2"]);
    expect(got.groups.every((g) => g.key.method === "credits")).toBe(true);
  });

  it("discounted_and_plain_trades_settle_in_separate_groups", () => {
    const s = web3();
    const plain = creatorOrder(s, "wearable");
    const discounted = creatorOrder(s, "emote", 300_000);
    const got = partitionCart(priced([[plain, "credits"], [discounted, "credits"], [plain, "credits"]]), s, quiet);
    expect(got.groups.map((g) => [g.key.call.kind, indexes(g.lineIds)])).toEqual([
      ["accept", [0, 2]],
      ["acceptWithCoupon", [1]],
    ]);
    expect(got.groups[0]!.key.contract).toBe(got.groups[1]!.key.contract);
    expect(got.groups[1]!.total).toEqual({ method: "credits", quoted: 70, charged: 70 });
  });

  it("two_name_registrations_are_two_credits_groups", () => {
    const s = web3();
    const got = partitionCart(priced([[nameRegistration(s, "alice"), "credits"], [nameRegistration(s, "bobby"), "credits"]]), s, quiet);
    expect(got.groups.map((g) => indexes(g.lineIds))).toEqual([[0], [1]]);
  });

  it("a_cart_never_reroutes_a_line", () => {
    const s: Session = web2();
    const primary = storeMint(s);
    const ok = partitionCart(priced([[primary, "credits"], [wearableResale(s), "mana"]]), s, quiet);
    expect(ok.groups[0]!.manaRemainderAllowed, "no MANA leg without a web3 wallet").toBe(false);

    const free: PurchaseOffer = { ...primary, quotes: [{ method: "credits", quoted: usdCents(0) }] };
    const refused = partitionCart(priced([[primary, "mana"], [primary, "credits"], [free, "credits"]]), s, quiet).refused;
    expect(refused.map((r) => [r.index, r.code, r.method, r.allowed])).toEqual([
      [0, "methodNotAllowed", "mana", ["credits"]],
      [2, "nonPositiveQuote", "credits", ["credits"]],
    ]);

    const noRate: PurchaseOffer = { ...primary, quotes: [], unavailable: [{ method: "credits", missing: "manaUsdRate" }] };
    expect(partitionCart(priced([[noRate, "credits"]]), s, quiet).refused).toEqual([
      { lineId: "line-0", index: 0, code: "quoteUnavailable", method: "credits", allowed: ["credits"], missing: "manaUsdRate" },
    ]);

    const many = priced(Array.from({ length: MAX_CREDIT_LINES_PER_GROUP + 1 }, () => [primary, "credits"] as [PurchaseOffer, PaymentMethod]));
    const tooLarge = partitionCart(many, s, quiet).refused;
    expect(tooLarge).toHaveLength(MAX_CREDIT_LINES_PER_GROUP + 1);
    expect(tooLarge.every((r) => r.code === "groupTooLarge")).toBe(true);
  });

  it("a mint's quantity counts as units: toward totals and the 30-line credit cap", () => {
    const s = web3();
    const got = partitionCart(priced([[storeMint(s), "credits", 3]]), s, quiet);
    expect(got.groups[0]!.units).toBe(3);
    expect(got.groups[0]!.total).toEqual({ method: "credits", quoted: 345, charged: 360 });
    const capped = partitionCart(priced([[{ ...storeMint(s), available: null }, "credits", 31]]), s, quiet);
    expect(capped.refused.map((r) => r.code)).toEqual(["groupTooLarge"]);
    expect(partitionCart(priced([[storeMint(s), "credits", 11]]), s, quiet).refused.map((r) => r.code)).toEqual(["insufficientStock"]);
  });

  it("an unset method takes the presentation default: credits for an eligible line, MANA for a restricted one", () => {
    const s = web3();
    const got = partitionCart(priced([[storeMint(s), null], [parcelResale(s), null]]), s, quiet);
    expect(got.groups.map((g) => g.key.method)).toEqual(["credits", "mana"]);
  });

  it("a line without a current offer is refused, never priced from the cart", () => {
    const s = web3();
    const [line] = priced([[storeMint(s), "credits"]]);
    expect(partitionCart([{ line: line!.line, offer: null }], s, quiet).refused[0]!.code).toBe("offerUnavailable");
  });
});

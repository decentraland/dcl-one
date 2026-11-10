import { describe, expect, it } from "vitest";

import { evmAddress } from "@shop/model/address";
import { itemAsset, tokenAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import type { ManaWei, UsdCents } from "@shop/model/money";
import type { CartLine } from "@shop/model/purchase";
import type { OptionState, PaymentOptionView, PaymentPresentation } from "@shop/payment/present";
import { cartRails } from "./cart-rails";

const collection = evmAddress("0x000000000000000000000000000000000000c011");
const MANA = 10n ** 18n;

function line(id: string, quantity = 1, token = false, method: CartLine["method"] = null): CartLine {
  return {
    id,
    asset: token ? tokenAsset(CHAIN.POLYGON, collection, 42n) : itemAsset(CHAIN.POLYGON, collection, 7n),
    quantity,
    method,
    listing: {},
    display: { name: id },
    provenance: {},
    seen: null,
    addedAt: 1,
  };
}

const ready: OptionState = { kind: "ready" };
const credits = (state: OptionState = ready): PaymentOptionView => ({ method: "credits", quote: { method: "credits", quoted: 120 as UsdCents }, charge: 120 as UsdCents, state, mixed: null });
const mana = (amount: bigint, state: OptionState = ready): PaymentOptionView => ({ method: "mana", quote: { method: "mana", amount: amount as ManaWei }, charge: null, state, mixed: null });
const view = (options: PaymentOptionView[], manaOnly = false): PaymentPresentation => ({ primary: manaOnly ? "mana" : "credits", options, guidance: null, reasons: [], manaOnly, inconsistencies: [] });

// A credit is 10 cents: the default balance (1,000 credits) covers anything below unless a test says otherwise.
const rails = (lines: CartLine[], views: Record<string, PaymentPresentation | null>, opts: { creditsCents?: number | null; manaBalance?: bigint | null; chain?: (l: CartLine) => number } = {}) =>
  cartRails({
    lines,
    views,
    creditsBalance: (opts.creditsCents === undefined ? 10_000 : opts.creditsCents) as UsdCents | null,
    manaBalance: (opts.manaBalance ?? null) as ManaWei | null,
    manaChain: CHAIN.POLYGON,
    manaChainOf: opts.chain ?? ((l) => l.asset.chainId),
  });

describe("the cart's payment rails, as upstream's summaryRails", () => {
  it("credits pay the cart when every line takes credits and the balance covers it", () => {
    expect(rails([line("a"), line("b")], { a: view([credits()]), b: view([credits()]) }).rails).toEqual([{ method: "credits" }]);
  });

  it("a short balance takes the credits rail away: the Checkout fallback then opens the top-up", () => {
    const short = credits({ kind: "topUpRequired", balance: 0 as UsdCents, required: 120 as UsdCents, short: 120 as UsdCents });
    expect(rails([line("a")], { a: view([short]) }, { creditsCents: 0 }).rails).toEqual([]);
  });

  it("credits are priced from the offers, never from the method a line is on: lines left on MANA do not hide a short balance", () => {
    const views = { a: view([credits(), mana(MANA)]), b: view([credits(), mana(MANA)]) };
    // 3 units at 120 cents is 360: 359 cents is short, 360 is enough, whatever the lines are set to.
    expect(rails([line("a", 2, false, "mana"), line("b", 1, false, "mana")], views, { creditsCents: 359 }).rails).toEqual([]);
    expect(rails([line("a", 2, false, "mana"), line("b", 1, false, "mana")], views, { creditsCents: 360 }).rails).toEqual([{ method: "credits" }]);
    expect(rails([line("a")], { a: view([credits()]) }, { creditsCents: null }).rails).toEqual([]);
  });

  it("MANA pays the cart when every line has a MANA quote and the wallet holds their sum", () => {
    const views = { a: view([credits(), mana(2n * MANA)]), b: view([credits(), mana(3n * MANA)]) };
    const r = rails([line("a", 2), line("b")], views, { manaBalance: 7n * MANA });
    expect(r.rails).toEqual([{ method: "credits" }, { method: "mana", manaWei: 7n * MANA, chainId: CHAIN.POLYGON }]);
    expect(r.manaShortfall).toBeNull();
  });

  it("MANA the wallet holds that cannot pay is a disabled button, never hidden; none held is no button", () => {
    const views = { a: view([credits(), mana(5n * MANA, { kind: "insufficientMana", balance: MANA as ManaWei, required: 5n * MANA as ManaWei, short: 4n * MANA as ManaWei })]) };
    expect(rails([line("a")], views, { manaBalance: MANA })).toEqual({ rails: [{ method: "credits" }], manaShortfall: { manaWei: MANA, chainId: CHAIN.POLYGON } });
    expect(rails([line("a")], views, { manaBalance: 0n })).toEqual({ rails: [{ method: "credits" }], manaShortfall: null });
  });

  it("a MANA-only line takes the credits rail away; a line without a MANA quote takes the MANA rail away", () => {
    const resale = view([mana(2n * MANA)], true);
    expect(rails([line("a"), line("r", 1, true)], { a: view([credits(), mana(MANA)]), r: resale }, { manaBalance: 10n * MANA }).rails).toEqual([
      { method: "mana", manaWei: 3n * MANA, chainId: CHAIN.POLYGON },
    ]);
    expect(rails([line("a"), line("b")], { a: view([credits(), mana(MANA)]), b: view([credits()]) }, { manaBalance: 10n * MANA }).rails).toEqual([{ method: "credits" }]);
  });

  it("an empty cart has no rail, and a credits sum past what cents can hold is no credits rail", () => {
    expect(rails([], {})).toEqual({ rails: [], manaShortfall: null });
    const huge: PaymentOptionView = { method: "credits", quote: { method: "credits", quoted: Number.MAX_SAFE_INTEGER as UsdCents }, charge: Number.MAX_SAFE_INTEGER as UsdCents, state: ready, mixed: null };
    expect(rails([line("a", 3)], { a: view([huge]) }, { creditsCents: Number.MAX_SAFE_INTEGER }).rails).toEqual([]);
  });

  it("MANA paid on another chain than the wallet's MANA was read on is no MANA rail, and an unpriced line is no rail at all", () => {
    const views = { a: view([credits(), mana(MANA)]), b: view([credits(), mana(MANA)]) };
    const chain = (l: CartLine) => (l.id === "a" ? CHAIN.POLYGON : CHAIN.ETHEREUM);
    expect(rails([line("a"), line("b")], views, { manaBalance: 10n * MANA, chain }).rails).toEqual([{ method: "credits" }]);
    expect(rails([line("a")], { a: view([credits(), mana(MANA)]) }, { manaBalance: 10n * MANA, chain: () => CHAIN.ETHEREUM })).toEqual({ rails: [{ method: "credits" }], manaShortfall: null });
    expect(rails([line("a"), line("b")], { a: view([credits()]), b: null })).toEqual({ rails: [], manaShortfall: null });
  });
});

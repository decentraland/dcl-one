// Ports the payment-rule tests of src/shop/policy.rs and tests/shop/policy.rs to the presentation
// mirror, then checks that presentPayment renders the server's answer and reports an inconsistent one.

import { describe, expect, it, vi } from "vitest";

import { ASSET_KINDS, SALE_KINDS, type AssetKind, type SaleKind } from "../model/asset";
import { CHAIN } from "../model/chain";
import { manaFromWhole, manaWei, usdCents } from "../model/money";
import type { PurchaseOffer } from "../model/purchase";
import { CREDIT_REASONS, hasWallet, type Session } from "../model/settlement";
import { optionStateText, reasonText } from "./copy";
import {
  BOOK,
  SESSIONS,
  SIGNED_OUT,
  bundleOrder,
  creatorOrder,
  foreignMint,
  l1Mint,
  nameRegistration,
  nameResale,
  offerFor,
  parcelResale,
  rental,
  storeMint,
  web2,
  web3,
  wearableResale,
} from "./fixtures";
import { presentPayment, type Inconsistency } from "./present";
import { present, restrictions } from "./rule";

function quiet() {
  const issues: Inconsistency[] = [];
  return { issues, report: (issue: Inconsistency) => void issues.push(issue) };
}

function show(offer: PurchaseOffer, session: Session, ctx: Parameters<typeof presentPayment>[2] = {}) {
  const sink = quiet();
  const view = presentPayment(offer, session, { report: sink.report, ...ctx });
  return { view, issues: sink.issues };
}

const methods = (offer: PurchaseOffer, session: Session) => show(offer, session).view.options.map((o) => o.method);

describe("the rule mirror (src/shop/policy.rs tests)", () => {
  it("the_rule_table_is_exhaustive: every AssetKind x SaleKind x session, Polygon and Ethereum, single and bundled", () => {
    for (const kind of ASSET_KINDS) {
      for (const sale of SALE_KINDS) {
        for (const chainId of [CHAIN.POLYGON, CHAIN.ETHEREUM]) {
          for (const bundle of [false, true]) {
            for (const session of SESSIONS) {
              const eligible =
                !bundle &&
                (((sale === "primaryMint" && (kind === "wearable" || kind === "emote")) && chainId === CHAIN.POLYGON) ||
                  (sale === "primaryNameRegistration" && kind === "name"));
              const got = present(restrictions(kind, sale, chainId, bundle), session, chainId);
              const kase = `${kind} ${sale} ${chainId} bundle=${bundle} ${session.kind}`;
              if (eligible) {
                expect(got.default, kase).toBe("credits");
                expect(got.guidance, kase).toBeNull();
                expect(got.allowed, kase).toEqual(hasWallet(session) ? ["credits", "mana"] : ["credits"]);
              } else {
                expect(got.allowed, kase).toEqual(["mana"]);
                expect(got.default, kase).toBe("mana");
                expect(got.reasons.length, kase).toBeGreaterThan(0);
                expect(got.reasons.some((r) => CREDIT_REASONS.includes(r)), `${kase}: a MANA-only line never mentions credits`).toBe(false);
                expect(got.guidance !== null, kase).toBe(!hasWallet(session));
                if (kind === "parcel" || kind === "estate" || kind === "rental") expect(got.reasons, kase).toContain("landOrRental");
                if (sale === "secondary") expect(got.reasons, kase).toContain("secondarySale");
              }
            }
          }
        }
      }
    }
  });

  it("web2_guidance_names_the_network_gas_and_account", () => {
    expect(present(["secondarySale"], web2(), CHAIN.ETHEREUM).guidance).toEqual({
      chainId: CHAIN.ETHEREUM,
      nativeGasRequired: true,
      account: BOOK.buyer,
    });
    expect(present(["landOrRental"], SIGNED_OUT, CHAIN.POLYGON).guidance).toEqual({
      chainId: CHAIN.POLYGON,
      nativeGasRequired: false,
      account: null,
    });
  });
});

describe("presentPayment renders the server's answer (tests/shop/policy.rs)", () => {
  it("the_derived_table_matches_the_confirmed_rule: eleven derivable pairs, three sessions each", () => {
    let derived = 0;
    for (const kind of ASSET_KINDS) {
      for (const sale of SALE_KINDS) {
        for (const session of SESSIONS) {
          const offer = offerFor(kind as AssetKind, sale as SaleKind, session);
          if (!offer) continue;
          const kase = `${kind} ${sale} ${session.kind}`;
          const { view, issues } = show(offer, session);
          expect(issues, kase).toEqual([]);
          const eligible =
            ((kind === "wearable" || kind === "emote") && sale === "primaryMint") ||
            (kind === "name" && sale === "primaryNameRegistration");
          if (eligible) {
            expect(view.options.map((o) => o.method), kase).toEqual(hasWallet(session) ? ["credits", "mana"] : ["credits"]);
            expect(view.primary, kase).toBe("credits");
            expect(view.guidance, kase).toBeNull();
          } else {
            expect(view.options.map((o) => o.method), kase).toEqual(["mana"]);
            expect(view.primary, kase).toBe("mana");
            expect(view.manaOnly, kase).toBe(true);
            expect(view.guidance !== null, kase).toBe(!hasWallet(session));
            expect(view.reasons, kase).not.toContain("creditsFirst");
            expect(view.options[0]!.mixed, `${kase}: no credits+MANA option`).toBeNull();
          }
          derived += 1;
        }
      }
    }
    expect(derived).toBe(11 * 3);
  });

  it("a_creator_reselling_a_minted_token_is_a_secondary_sale", () => {
    const { view } = show(wearableResale(web3()), web3());
    expect(view.options.map((o) => o.method)).toEqual(["mana"]);
    expect(view.reasons).toEqual(["secondarySale"]);
    expect(view.options[0]!.quote).toEqual({ method: "mana", amount: manaFromWhole(2) });
  });

  it("LAND, estates and rentals are MANA only, with network, gas and account guidance for web2", () => {
    const { view } = show(parcelResale(web2()), web2());
    expect(view.guidance).toEqual({ chainId: CHAIN.ETHEREUM, nativeGasRequired: true, account: BOOK.buyer });
    expect(view.options).toEqual([
      { method: "mana", quote: { method: "mana", amount: manaFromWhole(4000) }, charge: null, state: { kind: "walletRequired" }, mixed: null },
    ]);
    expect(show(rental(SIGNED_OUT), SIGNED_OUT).view.guidance).toEqual({ chainId: CHAIN.ETHEREUM, nativeGasRequired: true, account: null });
    expect(show(rental(web3()), web3()).view.guidance).toBeNull();
  });

  it("NAME registration is credits first with the registrar second for web3; a NAME resale is MANA only", () => {
    expect(methods(nameRegistration(web3()), web3())).toEqual(["credits", "mana"]);
    expect(methods(nameRegistration(web2()), web2())).toEqual(["credits"]);
    const resale = show(nameResale(web2()), web2()).view;
    expect(resale.options.map((o) => o.method)).toEqual(["mana"]);
    expect(resale.guidance?.chainId).toBe(CHAIN.ETHEREUM);
  });

  it("bundles, foreign contracts and L1 mints are MANA only", () => {
    expect(show(bundleOrder(web3()), web3()).view.reasons).toEqual(["bundle"]);
    expect(show(foreignMint(web3()), web3()).view.reasons).toEqual(["notDecentralandItem"]);
    expect(show(l1Mint(web3()), web3()).view.reasons).toEqual(["creditsChainUnsupported"]);
  });

  it("signed out: credits first, sign in to pay", () => {
    const { view } = show(storeMint(SIGNED_OUT), SIGNED_OUT);
    expect(view.reasons).toEqual(["creditsFirst", "signInForCredits"]);
    expect(view.options[0]!.state).toEqual({ kind: "signInRequired" });
  });
});

describe("inconsistent server answers are rendered safely and reported", () => {
  it("a server quote offering credits on a resale is rendered MANA only and reported", () => {
    const honest = wearableResale(web3());
    const crafted: PurchaseOffer = {
      ...honest,
      eligibility: { allowed: ["credits", "mana"], default: "credits", reasons: ["creditsFirst", "manaSecond"], guidance: null },
      quotes: [{ method: "credits", quoted: usdCents(100) }, ...honest.quotes],
    };
    const { view, issues } = show(crafted, web3());
    expect(view.options.map((o) => o.method)).toEqual(["mana"]);
    expect(view.primary).toBe("mana");
    expect(view.reasons).toEqual(["secondarySale"]);
    expect(issues.map((i) => i.code)).toEqual(
      expect.arrayContaining(["creditsOnRestricted", "defaultMismatch", "creditsQuoteOnRestricted", "creditReasonOnRestricted"]),
    );
  });

  it("credits offered on LAND are dropped, and web2 still gets the MANA guidance", () => {
    const honest = parcelResale(web2());
    const crafted: PurchaseOffer = {
      ...honest,
      eligibility: { allowed: ["credits"], default: "credits", reasons: ["creditsFirst"], guidance: null },
      quotes: [{ method: "credits", quoted: usdCents(200_000) }],
    };
    const { view, issues } = show(crafted, web2());
    expect(view.options.map((o) => [o.method, o.state.kind])).toEqual([["mana", "unavailable"]]);
    expect(view.guidance).toEqual({ chainId: CHAIN.ETHEREUM, nativeGasRequired: true, account: BOOK.buyer });
    expect(issues.map((i) => i.code)).toEqual(expect.arrayContaining(["creditsOnRestricted", "serverNarrower", "guidanceMismatch"]));
  });

  it("credits the server withholds on an eligible primary line stay the unavailable main action, never a silent MANA", () => {
    for (const session of [web3(), web2()]) {
      const honest = storeMint(session);
      const narrowed: PurchaseOffer = { ...honest, eligibility: { ...honest.eligibility, allowed: honest.eligibility.allowed.filter((m) => m !== "credits") } };
      const { view, issues } = show(narrowed, session);
      expect(view.primary).toBe("credits");
      expect(view.options[0]).toMatchObject({ method: "credits", quote: null, state: { kind: "unavailable", withheld: true } });
      expect(optionStateText("credits", view.options[0]!.state)).toBe("Paying with Credits is unavailable right now. Try again in a moment.");
      expect(view.options.slice(1).map((o) => [o.method, o.state.kind])).toEqual(session.kind === "web3" ? [["mana", "ready"]] : []);
      expect(issues.map((i) => i.code)).toContain("serverNarrower");
    }
  });

  it("a primary sale claimed for an existing token is judged by its identity: a resale", () => {
    const honest = storeMint(web3());
    const crafted: PurchaseOffer = { ...wearableResale(web3()), sale: "primaryMint", eligibility: honest.eligibility, quotes: honest.quotes };
    const { view, issues } = show(crafted, web3());
    expect(view.manaOnly).toBe(true);
    expect(view.options.map((o) => o.method)).toEqual(["mana"]);
    expect(issues.map((i) => i.code)).toContain("saleIdentityMismatch");
  });

  it("an eligible line keeps credits as the main action whatever default the server names", () => {
    const honest = creatorOrder(web3());
    const { view, issues } = show({ ...honest, eligibility: { ...honest.eligibility, default: "mana" } }, web3());
    expect(view.primary).toBe("credits");
    expect(issues.map((i) => i.code)).toEqual(["defaultMismatch"]);
  });

  it("MANA offered to a web2 buyer on an eligible line is not shown", () => {
    const honest = storeMint(web3());
    const { view, issues } = show(honest, web2());
    expect(view.options.map((o) => o.method)).toEqual(["credits"]);
    expect(issues.map((i) => i.code)).toContain("manaWithoutWallet");
  });

  it("the default reporter is used when none is passed", async () => {
    const { setPaymentInconsistencyReporter } = await import("./present");
    const seen = vi.fn();
    const restore = setPaymentInconsistencyReporter(seen);
    const honest = creatorOrder(web3());
    presentPayment({ ...honest, eligibility: { ...honest.eligibility, default: "mana" } }, web3());
    restore();
    expect(seen).toHaveBeenCalledWith(expect.objectContaining({ code: "defaultMismatch" }));
  });
});

describe("balances decide an option's state, never its method", () => {
  it("insufficient credits is an explicit top-up; the main action stays credits and web2 sees no MANA", () => {
    const { view } = show(storeMint(web2()), web2(), { creditsBalance: usdCents(100) });
    expect(view.primary).toBe("credits");
    expect(view.options).toHaveLength(1);
    expect(view.options[0]!.charge).toBe(120);
    expect(view.options[0]!.state).toEqual({ kind: "topUpRequired", balance: 100, required: 120, short: 20 });
    expect(optionStateText("credits", view.options[0]!.state)).toMatch(/Top up/);
  });

  it("a web3 buyer short of credits still defaults to credits; MANA stays second", () => {
    const { view } = show(storeMint(web3()), web3(), { creditsBalance: usdCents(0), manaBalance: manaFromWhole(50) });
    expect(view.primary).toBe("credits");
    expect(view.options.map((o) => [o.method, o.state.kind])).toEqual([
      ["credits", "topUpRequired"],
      ["mana", "ready"],
    ]);
  });

  it("credits+MANA lives inside the MANA option: 327c held on a 410c NAME is 32 credits and 9 MANA", () => {
    const offer = nameRegistration(web3());
    const priced: PurchaseOffer = {
      ...offer,
      quotes: [{ method: "credits", quoted: usdCents(410) }, { method: "mana", amount: manaWei(41n * 10n ** 18n) }],
    };
    const { view } = show(priced, web3(), { creditsBalance: usdCents(327), manaBalance: manaFromWhole(10) });
    const mana = view.options.find((o) => o.method === "mana")!;
    expect(mana.mixed).toEqual({ creditsCents: 320, credits: 32, manaWei: manaFromWhole(9) });
    expect(view.options[0]!.state.kind).toBe("topUpRequired");
    expect(show(priced, web3(), { creditsBalance: usdCents(327), manaBalance: manaFromWhole(8) }).view.options[1]!.mixed).toBeNull();
  });

  it("not enough MANA is stated on the MANA option", () => {
    const { view } = show(parcelResale(web3()), web3(), { manaBalance: manaFromWhole(3999) });
    expect(view.options[0]!.state).toEqual({
      kind: "insufficientMana",
      balance: manaFromWhole(3999),
      required: manaFromWhole(4000),
      short: manaFromWhole(1),
    });
  });
});

describe("outages keep the correct choices with an explanation", () => {
  const outage = (offer: PurchaseOffer): PurchaseOffer => ({
    ...offer,
    quotes: offer.quotes.filter((q) => q.method !== "credits"),
    unavailable: [{ method: "credits", missing: "manaUsdRate" }],
  });

  it("web3: credits stays the unavailable main action and MANA stays second", () => {
    const { view, issues } = show(outage(storeMint(web3())), web3());
    expect(issues).toEqual([]);
    expect(view.primary).toBe("credits");
    expect(view.options.map((o) => [o.method, o.state.kind])).toEqual([
      ["credits", "unavailable"],
      ["mana", "ready"],
    ]);
    expect(optionStateText("credits", view.options[0]!.state)).toBe(
      "The Credits price is unavailable right now: the MANA price feed is not answering. Try again in a moment.",
    );
  });

  it("web2: the credits option stays, unavailable, and MANA is not offered in its place", () => {
    const { view } = show(outage(storeMint(web2())), web2());
    expect(view.options.map((o) => [o.method, o.state.kind])).toEqual([["credits", "unavailable"]]);
    expect(view.guidance).toBeNull();
  });

  it("a missing quote with no stated outage is still unavailable, and reported", () => {
    const offer = storeMint(web2());
    const { view, issues } = show({ ...offer, quotes: [] }, web2());
    expect(view.options[0]!.state).toEqual({ kind: "unavailable", missing: null });
    expect(issues.map((i) => i.code)).toEqual(["missingQuote"]);
  });
});

describe("the words", () => {
  it("MANA-only explanations never mention credits", () => {
    for (const reason of ["secondarySale", "landOrRental", "notDecentralandItem", "bundle", "creditsChainUnsupported", "unsupportedCombination"] as const) {
      expect(reasonText(reason)).not.toMatch(/credit/i);
    }
    expect(optionStateText("mana", { kind: "walletRequired" })).not.toMatch(/credit/i);
  });
});

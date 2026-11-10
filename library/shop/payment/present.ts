// from: shop@94584a5c:app/src/lib/payment-options.ts
// How one purchase line's payment choices are PRESENTED. Upstream computed rails from balances alone
// and took no sale kind; here the server's PurchaseOffer decides which methods exist (SHOP.md payment
// table), balances only decide each option's state, and nothing ever moves a buyer to another method:
// short credits mean an explicit top-up, a credits outage keeps credits as the (unavailable) main
// action, and a restricted line never shows a credit price, a top-up or a credits+MANA option.

import { assetKey } from "../model/asset";
import {
  chargeForQuote,
  creditsFloor,
  manaForRemainder,
  subCents,
  wholeCreditsFloor,
  type ManaWei,
  type UsdCents,
} from "../model/money";
import { offerQuote, type PurchaseOffer } from "../model/purchase";
import {
  CREDIT_REASONS,
  hasWallet,
  type Eligibility,
  type MissingFact,
  type PaymentMethod,
  type Quote,
  type Reason,
  type Session,
  type WalletGuidance,
} from "../model/settlement";
import { identitySale, ruleFor } from "./rule";

export type OptionState =
  | { kind: "ready" }
  /** Signed out: sign in (and buy credits as needed) to pay with credits. */
  | { kind: "signInRequired" }
  /** The balance does not cover the whole-credit charge: offer an explicit top-up. */
  | { kind: "topUpRequired"; balance: UsdCents; required: UsdCents; short: UsdCents }
  | { kind: "insufficientMana"; balance: ManaWei; required: ManaWei; short: ManaWei }
  /** A MANA-only line for a session without a web3 wallet: the guidance says how to get one. */
  | { kind: "walletRequired" }
  /**
   * The rule offers the method but the server cannot price it now, or (`withheld`) does not offer it at
   * all, e.g. a credits kill switch: the option stays on screen, unavailable, with a retry; never a switch.
   */
  | { kind: "unavailable"; missing: MissingFact | null; withheld?: boolean };

/** Credits first, then MANA for the remainder: only ever inside the MANA option of an eligible line. */
export type MixedPayment = { creditsCents: UsdCents; credits: number; manaWei: ManaWei };

export type PaymentOptionView = {
  method: PaymentMethod;
  quote: Quote | null;
  /** The whole-credit amount a credits quote is debited at. */
  charge: UsdCents | null;
  state: OptionState;
  mixed: MixedPayment | null;
};

export type InconsistencyCode =
  | "creditsOnRestricted"
  | "creditsQuoteOnRestricted"
  | "manaWithoutWallet"
  | "serverNarrower"
  | "defaultMismatch"
  | "saleIdentityMismatch"
  | "creditReasonOnRestricted"
  | "reasonsMismatch"
  | "guidanceMismatch"
  | "missingQuote"
  | "nonPositiveQuote"
  | "nothingOffered";

export type Inconsistency = { code: InconsistencyCode; asset: string; detail: string };

export type PaymentPresentation = {
  /** The main action: credits for an eligible primary line, MANA for a restricted one. */
  primary: PaymentMethod;
  /** In presentation order (credits first); empty when the server offered nothing presentable. */
  options: PaymentOptionView[];
  /** Wallet/network/funding guidance for a MANA-only line without a web3 wallet. */
  guidance: WalletGuidance | null;
  reasons: Reason[];
  manaOnly: boolean;
  inconsistencies: Inconsistency[];
};

export type PaymentContext = {
  /** The buyer's spendable credit balance, when known. */
  creditsBalance?: UsdCents | null;
  /** The connected wallet's MANA on the settlement chain, when known. */
  manaBalance?: ManaWei | null;
  report?: (issue: Inconsistency) => void;
};

let reporter: (issue: Inconsistency) => void = (issue) => {
  console.warn(`[shop/payment] ${issue.code} on ${issue.asset}: ${issue.detail}`);
};

/** Where inconsistent server answers are reported (monitoring); defaults to console.warn. */
export function setPaymentInconsistencyReporter(next: (issue: Inconsistency) => void): () => void {
  const previous = reporter;
  reporter = next;
  return () => {
    reporter = previous;
  };
}

function sameSet<T>(a: readonly T[], b: readonly T[]): boolean {
  return a.length === b.length && a.every((x) => b.includes(x));
}

export function presentPayment(offer: PurchaseOffer, session: Session, ctx: PaymentContext = {}): PaymentPresentation {
  const asset = assetKey(offer.asset);
  const inconsistencies: Inconsistency[] = [];
  const flag = (code: InconsistencyCode, detail: string) => {
    const issue = { code, asset, detail };
    inconsistencies.push(issue);
    (ctx.report ?? reporter)(issue);
  };

  if (identitySale(offer) !== offer.sale) {
    flag("saleIdentityMismatch", `a ${offer.sale} ${offer.kind} on a ${offer.asset.token.kind} id is judged as a resale`);
  }
  const rule = ruleFor(offer, session);
  const server = offer.eligibility;
  const manaOnly = !rule.allowed.includes("credits");
  const primary = rule.default ?? "mana";

  if (manaOnly && server.allowed.includes("credits")) flag("creditsOnRestricted", "the server offered credits on a MANA-only line");
  if (!rule.allowed.includes("mana") && server.allowed.includes("mana")) {
    flag("manaWithoutWallet", "the server offered MANA on an eligible line to a session without a web3 wallet");
  }
  for (const method of rule.allowed) {
    if (!server.allowed.includes(method)) flag("serverNarrower", `the server does not offer ${method}, which the rule offers`);
  }
  if (server.default !== rule.default) flag("defaultMismatch", `the server's default is ${server.default}, the rule's ${rule.default}`);
  if (manaOnly && offer.quotes.some((q) => q.method === "credits")) {
    flag("creditsQuoteOnRestricted", "a credit price was quoted for a MANA-only line; it is not shown");
  }

  const creditReasons = server.reasons.filter((r) => CREDIT_REASONS.includes(r));
  if (manaOnly && creditReasons.length > 0) flag("creditReasonOnRestricted", `dropped ${creditReasons.join(", ")}`);
  const serverReasons = manaOnly ? server.reasons.filter((r) => !CREDIT_REASONS.includes(r)) : server.reasons;
  if (!sameSet(serverReasons, rule.reasons)) {
    flag("reasonsMismatch", `server [${server.reasons.join(", ")}], rule [${rule.reasons.join(", ")}]`);
  }
  const reasons = serverReasons.length > 0 ? serverReasons : rule.reasons;

  let guidance: WalletGuidance | null = null;
  if (rule.guidance) {
    const served = server.guidance;
    const agrees =
      served !== null &&
      served.chainId === rule.guidance.chainId &&
      served.nativeGasRequired === rule.guidance.nativeGasRequired &&
      served.account === rule.guidance.account;
    if (!agrees) flag("guidanceMismatch", "the wallet guidance is missing or names another network or account");
    guidance = agrees ? served : rule.guidance;
  } else if (server.guidance) {
    flag("guidanceMismatch", "wallet guidance on a line that needs none");
  }

  const options: PaymentOptionView[] = [];
  for (const method of rule.allowed) {
    const outage = offer.unavailable.find((u) => u.method === method);
    if (!server.allowed.includes(method)) {
      options.push({ method, quote: null, charge: null, state: { kind: "unavailable", missing: outage?.missing ?? null, withheld: true }, mixed: null });
      continue;
    }
    const quote = offerQuote(offer, method);
    if (!quote) {
      if (!outage) flag("missingQuote", `no ${method} quote and no stated outage`);
      options.push({ method, quote: null, charge: null, state: { kind: "unavailable", missing: outage?.missing ?? null }, mixed: null });
      continue;
    }
    const positive = quote.method === "credits" ? quote.quoted > 0 : quote.amount > 0n;
    if (!positive) {
      flag("nonPositiveQuote", `the ${method} quote is not positive`);
      options.push({ method, quote: null, charge: null, state: { kind: "unavailable", missing: null }, mixed: null });
      continue;
    }
    options.push(optionView(quote, session, ctx));
  }
  if (options.length === 0) flag("nothingOffered", "no payment method is presentable for this line");

  const manaOption = options.find((o) => o.method === "mana");
  const creditsQuote = options.find((o) => o.method === "credits")?.quote;
  if (!manaOnly && manaOption?.quote?.method === "mana" && creditsQuote?.method === "credits") {
    manaOption.mixed = mixedPayment(creditsQuote.quoted, manaOption.quote.amount, ctx);
  }

  return { primary, options, guidance, reasons, manaOnly, inconsistencies };
}

/**
 * The presentation of a line the server has not priced (the quote route is missing or failing): the
 * rule mirror's choices for what the line identifies, each unavailable, so an outage keeps credits
 * first on an eligible line and MANA only (with its reasons and guidance) on a restricted one.
 */
export function presentUnpriced(rule: Eligibility): PaymentPresentation {
  const manaOnly = !rule.allowed.includes("credits");
  return {
    primary: rule.default ?? "mana",
    options: rule.allowed.map((method) => ({ method, quote: null, charge: null, state: { kind: "unavailable", missing: null }, mixed: null })),
    guidance: rule.guidance,
    reasons: manaOnly ? rule.reasons.filter((r) => !CREDIT_REASONS.includes(r)) : rule.reasons,
    manaOnly,
    inconsistencies: [],
  };
}

function optionView(quote: Quote, session: Session, ctx: PaymentContext): PaymentOptionView {
  if (quote.method === "credits") {
    const charge = chargeForQuote(quote.quoted);
    let state: OptionState = { kind: "ready" };
    if (session.kind === "signedOut") state = { kind: "signInRequired" };
    else if (charge !== null && ctx.creditsBalance != null && ctx.creditsBalance < charge) {
      state = { kind: "topUpRequired", balance: ctx.creditsBalance, required: charge, short: subCents(charge, ctx.creditsBalance)! };
    }
    return { method: "credits", quote, charge, state, mixed: null };
  }
  let state: OptionState = { kind: "ready" };
  if (!hasWallet(session)) state = { kind: "walletRequired" };
  else if (ctx.manaBalance != null && ctx.manaBalance < quote.amount) {
    state = {
      kind: "insufficientMana",
      balance: ctx.manaBalance,
      required: quote.amount,
      short: (quote.amount - ctx.manaBalance) as ManaWei,
    };
  }
  return { method: "mana", quote, charge: null, state, mixed: null };
}

/**
 * Upstream's 'combined' rail, kept inside the MANA option: the whole credits the balance holds, then
 * MANA for the remainder, rounded up. Null when credits alone or MANA alone is the whole story.
 */
function mixedPayment(priceCents: UsdCents, priceWei: ManaWei, ctx: PaymentContext): MixedPayment | null {
  if (ctx.creditsBalance == null || ctx.creditsBalance <= 0) return null;
  const leg = wholeCreditsFloor(ctx.creditsBalance);
  if (leg <= 0 || leg >= priceCents) return null;
  const remainder = subCents(priceCents, leg);
  const manaWei = remainder === null ? null : manaForRemainder(remainder, priceCents, priceWei);
  if (manaWei === null) return null;
  if (ctx.manaBalance != null && ctx.manaBalance < manaWei) return null;
  return { creditsCents: leg, credits: creditsFloor(leg), manaWei };
}

// from: shop@94584a5c:app/src/lib/buy.ts (groupPurchases, purchaseGroupKey), app/src/lib/cart-checkout.ts
//       (groupUnitsForAuthorization)
// Upstream grouped a cart by chain + marketplace + coupon with no sale kind, so a resale could join a
// credits group. This mirrors src/shop/policy.rs `partition` instead: one group per payment method,
// chain, settlement contract and call, each line in the group its SERVER offer names for the method
// the buyer chose. A line whose chosen method is not offered is refused and named, never moved to
// another method; the server repeats the partition at checkout and is the authority.

import { assetKey } from "../model/asset";
import { isPolygon } from "../model/chain";
import {
  addCents,
  addMana,
  chargeForQuote,
  mulCents,
  mulMana,
  subCents,
  ZERO_CENTS,
  ZERO_MANA,
  type ManaWei,
  type UsdCents,
} from "../model/money";
import { offerQuote, offerSettlementKey, type CartLine, type CartTotals, type PurchaseOffer } from "../model/purchase";
import {
  MAX_CREDIT_LINES_PER_GROUP,
  settlementKeyId,
  type GroupTotal,
  type MissingFact,
  type PaymentMethod,
  type Quote,
  type Session,
  type SettlementGroupKey,
} from "../model/settlement";
import { presentPayment, type Inconsistency } from "../payment/present";

export type PricedLine = { line: CartLine; offer: PurchaseOffer | null };

export type CartGroup = {
  keyId: string;
  key: SettlementGroupKey;
  /** Cart line ids, in cart order. */
  lineIds: string[];
  /** Each line's unit quote, aligned with lineIds. */
  quotes: Quote[];
  /** Units the group settles (a mint line of quantity 3 is three). */
  units: number;
  total: GroupTotal;
  /** A credits group whose every line also offers MANA (a web3 wallet) may top up with MANA. */
  manaRemainderAllowed: boolean;
};

export type LineRefusalCode =
  | "offerUnavailable"
  | "methodNotAllowed"
  | "quoteUnavailable"
  | "nonPositiveQuote"
  | "noSettlement"
  | "insufficientStock"
  | "groupTooLarge"
  | "amountOverflow";

export type LineRefusal = {
  lineId: string;
  index: number;
  code: LineRefusalCode;
  method: PaymentMethod | null;
  /** The methods the line does offer, so the UI can ask the buyer to choose. */
  allowed: PaymentMethod[];
  missing?: MissingFact | null;
};

export type CartPartition = {
  groups: CartGroup[];
  refused: LineRefusal[];
  /** Separate totals: never a sum across currencies. */
  totals: CartTotals;
  /** How much the credits groups' charge exceeds `creditsBalance`: an explicit top-up, never a switch. */
  creditsShort: UsdCents | null;
  ok: boolean;
};

export type PartitionOptions = {
  creditsBalance?: UsdCents | null;
  manaBalance?: ManaWei | null;
  report?: (issue: Inconsistency) => void;
};

export function lineUnits(line: CartLine): number {
  return line.asset.token.kind === "item" ? Math.max(1, Math.floor(line.quantity)) : 1;
}

function emptyTotal(method: PaymentMethod): GroupTotal {
  return method === "credits" ? { method, quoted: ZERO_CENTS, charged: ZERO_CENTS } : { method, amount: ZERO_MANA };
}

function addLine(total: GroupTotal, quote: Quote, units: number): GroupTotal | null {
  if (total.method === "credits" && quote.method === "credits") {
    const charge = chargeForQuote(quote.quoted);
    const quoted = mulCents(quote.quoted, units);
    const charged = charge === null ? null : mulCents(charge, units);
    if (quoted === null || charged === null) return null;
    const q = addCents(total.quoted, quoted);
    const c = addCents(total.charged, charged);
    return q === null || c === null ? null : { method: "credits", quoted: q, charged: c };
  }
  if (total.method === "mana" && quote.method === "mana") {
    const amount = mulMana(quote.amount, units);
    const sum = amount === null ? null : addMana(total.amount, amount);
    return sum === null ? null : { method: "mana", amount: sum };
  }
  return null;
}

export function partitionCart(priced: PricedLine[], session: Session, opts: PartitionOptions = {}): CartPartition {
  const groups: CartGroup[] = [];
  const byKey = new Map<string, CartGroup>();
  const refused: LineRefusal[] = [];

  priced.forEach(({ line, offer }, index) => {
    const refuse = (code: LineRefusalCode, method: PaymentMethod | null, allowed: PaymentMethod[], missing?: MissingFact | null) =>
      refused.push({ lineId: line.id, index, code, method, allowed, missing });
    if (!offer || assetKey(offer.asset) !== assetKey(line.asset)) {
      refuse("offerUnavailable", line.method, []);
      return;
    }
    const view = presentPayment(offer, session, {
      creditsBalance: opts.creditsBalance,
      manaBalance: opts.manaBalance,
      report: opts.report,
    });
    const allowed = view.options.map((o) => o.method);
    const method = line.method ?? view.primary;
    const option = view.options.find((o) => o.method === method);
    if (!option) {
      refuse("methodNotAllowed", method, allowed);
      return;
    }
    const raw = offerQuote(offer, method);
    if (raw && (raw.method === "credits" ? raw.quoted <= 0 : raw.amount <= 0n)) {
      refuse("nonPositiveQuote", method, allowed);
      return;
    }
    if (!option.quote) {
      refuse("quoteUnavailable", method, allowed, option.state.kind === "unavailable" ? option.state.missing : null);
      return;
    }
    const key = offerSettlementKey(offer, method);
    if (!key || key.method !== method || (method === "credits" && !isPolygon(key.chainId))) {
      refuse("noSettlement", method, allowed);
      return;
    }
    const units = lineUnits(line);
    if (offer.available !== null && units > offer.available) {
      refuse("insufficientStock", method, allowed);
      return;
    }
    const keyId = settlementKeyId(key);
    let group = byKey.get(keyId);
    const fresh = !group;
    if (!group) {
      group = {
        keyId,
        key,
        lineIds: [],
        quotes: [],
        units: 0,
        total: emptyTotal(method),
        manaRemainderAllowed: method === "credits",
      };
    }
    const total = addLine(group.total, option.quote, units);
    if (total === null) {
      refuse("amountOverflow", method, allowed);
      return;
    }
    if (fresh) {
      groups.push(group);
      byKey.set(keyId, group);
    }
    group.total = total;
    group.lineIds.push(line.id);
    group.quotes.push(option.quote);
    group.units += units;
    if (method === "credits" && !allowed.includes("mana")) group.manaRemainderAllowed = false;
  });

  for (const group of groups) {
    if (group.key.method === "credits" && group.units > MAX_CREDIT_LINES_PER_GROUP) {
      for (const lineId of group.lineIds) {
        const index = priced.findIndex((p) => p.line.id === lineId);
        refused.push({ lineId, index, code: "groupTooLarge", method: "credits", allowed: [] });
      }
    }
  }
  refused.sort((a, b) => a.index - b.index);

  const refusedIds = new Set(refused.map((r) => r.lineId));
  const kept = groups.filter((g) => !g.lineIds.some((id) => refusedIds.has(id)));
  const totals = cartTotals(kept);
  const balance = opts.creditsBalance;
  const creditsShort =
    balance != null && totals.credits.charged > balance ? (subCents(totals.credits.charged, balance) ?? null) : null;
  return { groups, refused, totals, creditsShort, ok: refused.length === 0 };
}

export function cartTotals(groups: CartGroup[]): CartTotals {
  let quoted = ZERO_CENTS;
  let charged = ZERO_CENTS;
  let mana = ZERO_MANA;
  for (const { total } of groups) {
    if (total.method === "credits") {
      quoted = addCents(quoted, total.quoted) ?? quoted;
      charged = addCents(charged, total.charged) ?? charged;
    } else {
      mana = addMana(mana, total.amount) ?? mana;
    }
  }
  return { credits: { quoted, charged }, mana };
}

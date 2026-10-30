// from: shop@94584a5c:app/src/lib/payment-options.ts
// Upstream computed the rails of one purchase from its price and the balances alone ('credits' when the
// balance covered it, 'combined', 'mana' whenever MANA covered it), with no sale kind: a resale could be
// offered credits, and a short credit balance quietly became a MANA default. Here the rails come from
// the server's offer under the payment rule, through the foundation's `presentPayment`:
//   - an eligible primary line: 'credits' is ALWAYS the first rail and the preferred one, even when the
//     balance is short (its state is then `topUpRequired`: an explicit top-up, never a switch); a web3
//     wallet also gets 'mana', with 'combined' (credits first, MANA for the remainder) inside it;
//   - a MANA-only line (resale, LAND, rental, foreign, bundle): 'mana' alone, with wallet guidance for a
//     session without a web3 wallet. No credit price, no top-up, no combined rail.
// `distributeCreditsAcrossUnits` stays a pure helper for the combined rail of a basket.

import type { ManaWei, UsdCents } from "../../model/money";
import { creditsFloor } from "../../model/money";
import type { PaymentMethod as RuleMethod, Reason, WalletGuidance } from "../../model/settlement";
import type { OptionState, PaymentPresentation } from "../../payment/present";

export type PaymentMethod = "credits" | "mana" | "combined";

export type PaymentOption =
  | { method: "credits"; creditsCents: UsdCents; credits: number; manaWei: 0n; state: OptionState }
  | { method: "mana"; creditsCents: 0; credits: 0; manaWei: ManaWei; state: OptionState }
  | { method: "combined"; creditsCents: UsdCents; credits: number; manaWei: ManaWei; state: OptionState };

/** Held MANA that cannot pay here: said on screen instead of silently dropping the MANA button. */
export type ManaShortfall = { manaWei: ManaWei; required: ManaWei; short: ManaWei };

export type PaymentOptions = {
  /** In presentation order: credits, combined, mana. A rail whose price is unavailable is still listed. */
  options: PaymentOption[];
  /** The main action: credits for an eligible primary line, MANA for a MANA-only one. Never re-routed by balances. */
  preferred: RuleMethod;
  manaShortfall: ManaShortfall | null;
  /** How much the credits rail is short, for the explicit top-up. */
  topUp: { short: UsdCents; required: UsdCents; balance: UsdCents } | null;
  manaOnly: boolean;
  guidance: WalletGuidance | null;
  reasons: Reason[];
};

const ZERO = 0 as UsdCents;

export function computePaymentOptions(view: PaymentPresentation): PaymentOptions {
  const options: PaymentOption[] = [];
  let manaShortfall: ManaShortfall | null = null;
  let topUp: PaymentOptions["topUp"] = null;
  for (const option of view.options) {
    if (option.method === "credits") {
      const charge = option.charge ?? ZERO;
      options.push({ method: "credits", creditsCents: charge, credits: creditsFloor(charge), manaWei: 0n, state: option.state });
      if (option.state.kind === "topUpRequired") {
        topUp = { short: option.state.short, required: option.state.required, balance: option.state.balance };
      }
      continue;
    }
    const amount = option.quote?.method === "mana" ? option.quote.amount : (0n as ManaWei);
    if (option.mixed && !view.manaOnly) {
      options.push({
        method: "combined",
        creditsCents: option.mixed.creditsCents,
        credits: option.mixed.credits,
        manaWei: option.mixed.manaWei,
        state: { kind: "ready" },
      });
    }
    options.push({ method: "mana", creditsCents: 0, credits: 0, manaWei: amount, state: option.state });
    if (option.state.kind === "insufficientMana") {
      manaShortfall = { manaWei: option.state.balance, required: option.state.required, short: option.state.short };
    }
  }
  options.sort((a, b) => ORDER[a.method] - ORDER[b.method]);
  return { options, preferred: view.primary, manaShortfall, topUp, manaOnly: view.manaOnly, guidance: view.guidance, reasons: view.reasons };
}

const ORDER: Record<PaymentMethod, number> = { credits: 0, combined: 1, mana: 2 };

/**
 * Spread a credit balance across a basket's units in order, for a combined payment: each unit takes
 * what it can, the unit that exhausts the balance takes a partial amount, later units take none.
 */
export function distributeCreditsAcrossUnits(unitCents: number[], balanceCents: number): number[] {
  let remaining = Number.isFinite(balanceCents) ? Math.max(0, Math.trunc(balanceCents)) : 0;
  return unitCents.map((raw) => {
    const price = Number.isFinite(raw) ? Math.max(0, Math.trunc(raw)) : 0;
    const take = Math.min(price, remaining);
    remaining -= take;
    return take;
  });
}

/** Can any rail pay right now (ready, not merely offered)? */
export function hasPayableOption(o: PaymentOptions): boolean {
  return o.options.some((opt) => opt.state.kind === "ready");
}

/** One rail by method, or null when it is not offered. */
export function findOption(o: PaymentOptions, method: PaymentMethod): PaymentOption | null {
  return o.options.find((opt) => opt.method === method) ?? null;
}

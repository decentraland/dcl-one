// from: shop@3a322cf9:app/src/pages/Cart.tsx (summaryRails: one CTA per rail the buyer's balances
// support, priced off the cart's own lines) and app/src/lib/payment-options.ts (credits alone, MANA alone,
// the MANA shortfall). The rails of the whole cart, read from each line's presentation (the server's offer
// under the payment rule), so the buttons can never offer a rail a line does not have. They depend on the
// offers and the balances only, never on the method each line happens to be set to now.
import { lineUnits } from "../../cart/partition";
import { addCents, mulCents, mulMana, type ManaWei, type UsdCents } from "../../model/money";
import type { CartLine } from "../../model/purchase";
import type { PaymentPresentation } from "../../payment/present";

export type CartRail = { method: "credits" } | { method: "mana"; manaWei: ManaWei; chainId: number };

export type CartRails = {
  /** Payable rails, credits first. Empty when nothing covers the cart: the credits fallback then opens a top-up. */
  rails: CartRail[];
  /** MANA the buyer holds that cannot pay for the cart: the disabled MANA button. */
  manaShortfall: { manaWei: ManaWei; chainId: number } | null;
};

const NONE: CartRails = { rails: [], manaShortfall: null };

type Priced = { line: CartLine; view: PaymentPresentation };

/** The cart's credits charge (each line's whole-credit charge times its units), or null when a line cannot take credits. */
function creditsCharge(priced: Priced[]): UsdCents | null {
  let sum = 0 as UsdCents;
  for (const { line, view } of priced) {
    const credits = view.options.find((o) => o.method === "credits");
    if (!credits || credits.charge === null || (credits.state.kind !== "ready" && credits.state.kind !== "topUpRequired")) return null;
    const lineCharge = mulCents(credits.charge, lineUnits(line));
    const next = lineCharge === null ? null : addCents(sum, lineCharge);
    if (next === null) return null;
    sum = next;
  }
  return sum;
}

/**
 * Credits pay the cart when every line can be paid with credits and the balance covers the sum of their
 * whole-credit charges. MANA pays it when every line has a MANA quote on the chain the wallet's MANA was read
 * on, and that MANA covers their sum. A line the rule restricts to MANA takes the credits rail away; a line
 * without a MANA quote, or one paid on another chain, takes the MANA rail away.
 */
export function cartRails(input: {
  lines: readonly CartLine[];
  views: Readonly<Record<string, PaymentPresentation | null>>;
  /** The spendable credits balance in cents; null while unknown (no credits rail then). */
  creditsBalance: UsdCents | null;
  /** The wallet's MANA on `manaChain`; null while unknown or without a wallet. */
  manaBalance: ManaWei | null;
  /** The chain `manaBalance` was read on (the settlement chain). */
  manaChain: number;
  /** The chain a line's MANA is paid on (its offer's, else its asset's). */
  manaChainOf(line: CartLine): number;
}): CartRails {
  const { lines, views } = input;
  if (lines.length === 0) return NONE;
  const presented = lines.map((line) => ({ line, view: views[line.id] ?? null }));
  if (presented.some((p) => p.view === null)) return NONE;
  const priced = presented as Priced[];

  const rails: CartRail[] = [];
  const charge = creditsCharge(priced);
  if (charge !== null && input.creditsBalance !== null && charge <= input.creditsBalance) rails.push({ method: "credits" });

  let manaWei: ManaWei | null = 0n as ManaWei;
  for (const { line, view } of priced) {
    const mana = view.options.find((o) => o.method === "mana");
    const quote = mana?.quote?.method === "mana" ? mana.quote : null;
    const ok = !!quote && (mana!.state.kind === "ready" || mana!.state.kind === "insufficientMana");
    const total = ok ? mulMana(quote!.amount, lineUnits(line)) : null;
    if (total === null || input.manaChainOf(line) !== input.manaChain) {
      manaWei = null;
      break;
    }
    manaWei = (manaWei + total) as ManaWei;
  }
  const balance = input.manaBalance;
  if (manaWei !== null && manaWei > 0n && balance !== null) {
    if (balance >= manaWei) rails.push({ method: "mana", manaWei, chainId: input.manaChain });
    else if (balance > 0n) return { rails, manaShortfall: { manaWei: balance, chainId: input.manaChain } };
  }
  return { rails, manaShortfall: null };
}

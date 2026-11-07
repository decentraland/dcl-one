// from: shop@3a322cf9:app/src/pages/Cart.tsx (summaryRails: one CTA per rail the buyer's balances
// support, priced off the cart's own lines) and app/src/lib/payment-options.ts (credits alone, MANA alone,
// the MANA shortfall). The rails of the whole cart, read from each line's presentation (the server's offer
// under the payment rule), so the buttons can never offer a rail a line does not have.
import { lineUnits } from "../../cart/partition";
import { mulMana, type ManaWei } from "../../model/money";
import type { CartLine } from "../../model/purchase";
import type { PaymentPresentation } from "../../payment/present";

export type CartRail = { method: "credits" } | { method: "mana"; manaWei: ManaWei; chainId: number };

export type CartRails = {
  /** Payable rails, credits first. Empty when nothing covers the cart: the credits fallback then opens a top-up. */
  rails: CartRail[];
  /** MANA the buyer holds that cannot pay for the cart: the disabled MANA button. */
  manaShortfall: { manaWei: ManaWei } | null;
};

const NONE: CartRails = { rails: [], manaShortfall: null };

/**
 * Credits pay the cart when every line can be paid with credits and the balance covers the whole-credit
 * charge (the partition's `creditsShort` is null). MANA pays it when every line has a MANA quote on one
 * chain and the wallet's MANA covers their sum. A line the rule restricts to MANA takes the credits rail
 * away; a line without a MANA quote takes the MANA rail away.
 */
export function cartRails(input: {
  lines: readonly CartLine[];
  views: Readonly<Record<string, PaymentPresentation | null>>;
  creditsShort: number | null;
  manaBalance: ManaWei | null;
  /** The chain a line's MANA is paid on (its offer's, else its asset's). */
  manaChainOf(line: CartLine): number;
}): CartRails {
  const { lines, views } = input;
  if (lines.length === 0) return NONE;
  const presented = lines.map((line) => ({ line, view: views[line.id] ?? null }));
  if (presented.some((p) => p.view === null)) return NONE;

  const rails: CartRail[] = [];
  const creditsEverywhere = presented.every(({ view }) =>
    view!.options.some((o) => o.method === "credits" && (o.state.kind === "ready" || o.state.kind === "topUpRequired")),
  );
  if (creditsEverywhere && input.creditsShort === null) rails.push({ method: "credits" });

  let manaWei: ManaWei | null = 0n as ManaWei;
  let chainId: number | null = null;
  for (const { line, view } of presented) {
    const mana = view!.options.find((o) => o.method === "mana");
    const ok = mana && mana.quote?.method === "mana" && (mana.state.kind === "ready" || mana.state.kind === "insufficientMana");
    const lineChain = input.manaChainOf(line);
    if (!ok || mana.quote?.method !== "mana" || (chainId !== null && chainId !== lineChain)) {
      manaWei = null;
      break;
    }
    chainId = lineChain;
    const total = mulMana(mana.quote.amount, lineUnits(line));
    if (total === null) {
      manaWei = null;
      break;
    }
    manaWei = (manaWei + total) as ManaWei;
  }
  const balance = input.manaBalance;
  if (manaWei !== null && manaWei > 0n && chainId !== null && balance !== null) {
    if (balance >= manaWei) rails.push({ method: "mana", manaWei, chainId });
    else if (balance > 0n) return { rails, manaShortfall: { manaWei: balance } };
  }
  return { rails, manaShortfall: null };
}

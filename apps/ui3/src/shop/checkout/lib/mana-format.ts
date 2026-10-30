// from: shop@94584a5c:app/src/lib/mana-format.ts
// MANA display. Upstream rounded to two decimals; SHOP.md keeps MANA in exact token units, so the
// formatter is model/money's exact one (every significant digit, grouped). `manaWeiToNumber` stays for
// charts and sorting only: it loses precision past 2^53 wei and must never size a charge.

import { formatMana as exact, type ManaWei } from "../../model/money";

/** MANA wei to a JS number, for DISPLAY arithmetic only. */
export function manaWeiToNumber(wei: bigint): number {
  return Number(wei) / 1e18;
}

/** Exact MANA, grouped: 1500e18 -> "1,500", 23e17 -> "2.3". */
export function formatMana(wei: bigint): string {
  return exact(wei as ManaWei, { group: true });
}

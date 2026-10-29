// from: shop@94584a5c:app/src/pages/GetCredits.tsx (clearResumeHandoffs), app/src/lib/resume-buy.ts,
//       app/src/lib/resume-name.ts, app/src/lib/cart-checkout.ts (RESUME_CART_KEY)
// The hand-off a purchase leaves when it sends the buyer to buy credits: where to come back to once the
// credits land. ONE entry, not one per surface: upstream's three independent keys could outlive an
// abandoned top-up and be claimed by an unrelated one, charging a basket the buyer had walked away from.
// Anything that ends a top-up without resuming (a cancel, an unpaid order, a failure, a grant still in
// flight) drops it.

import { RESUME_CART_KEY } from "../checkout/purchase/cart";

const KEY = "dcl-shop-topup-return:v1";
const MAX_AGE_MS = 2 * 60 * 60 * 1000;

export type TopUpReturn = { path: string; at: number };

function store(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

/** Remember where to resume after the credits land (a same-origin /shop path only). */
export function stashTopUpReturn(path: string, now: number = Date.now()): void {
  if (!path.startsWith("/shop") || path.startsWith("//")) return;
  try {
    store()?.setItem(KEY, JSON.stringify({ path, at: now }));
  } catch {
    // The purchase is still in the cart or on its page; the buyer returns by hand.
  }
}

/** Read and clear the hand-off. */
export function takeTopUpReturn(now: number = Date.now()): TopUpReturn | null {
  const s = store();
  try {
    const raw = s?.getItem(KEY) ?? null;
    s?.removeItem(KEY);
    const parsed = raw ? (JSON.parse(raw) as TopUpReturn) : null;
    if (!parsed || typeof parsed.path !== "string" || !parsed.path.startsWith("/shop") || now - parsed.at > MAX_AGE_MS) return null;
    return parsed;
  } catch {
    return null;
  }
}

/** Forget every pending resume: the top-up ended without one. */
export function clearTopUpHandoffs(): void {
  const s = store();
  try {
    s?.removeItem(KEY);
    s?.removeItem(RESUME_CART_KEY);
  } catch {
    // Nothing was stored.
  }
}

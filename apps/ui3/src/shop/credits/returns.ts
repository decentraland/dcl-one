// from: shop@94584a5c:app/src/pages/GetCredits.tsx (clearResumeHandoffs), app/src/lib/resume-buy.ts,
//       app/src/lib/resume-name.ts, app/src/lib/cart-checkout.ts (RESUME_CART_KEY)
// The hand-off a purchase leaves when it sends the buyer to buy credits: where to come back to once the
// credits land. ONE entry, not one per surface: upstream's three independent keys could outlive an
// abandoned top-up and be claimed by an unrelated one, charging a basket the buyer had walked away from.
// Anything that ends a top-up without resuming (a cancel, an unpaid order, a failure, a grant still in
// flight) drops it.
//
// Buy now's top-up (upstream's RESUME_BUY_KEY) adds the record of what the buyer confirmed. The page it returns
// to, `<item>?buy=resume`, is a link anyone can make; the dialog finishes the purchase on its own only with this
// tab's record of the same account, item page and cart line, at that Credits price or less, AND for the pack order
// that top-up created: the credits page hands the order it credited to the page it returns to, so an abandoned
// Buy now top-up is never finished by another one. Any other top-up drops the record. Both records carry the attempt
// that wrote them, so a late failure of an older attempt never drops a newer one.

import { RESUME_CART_KEY } from "../checkout/purchase/cart";

const KEY = "dcl-shop-topup-return:v1";
const BUY_NOW_KEY = "dcl-shop-buynow-return:v1";
const MAX_AGE_MS = 2 * 60 * 60 * 1000;

/** `orderId`: the pack order the top-up created, once known; a return bound to one resumes only after that order. */
export type TopUpReturn = { path: string; at: number; attempt?: string; orderId?: string };

/** What Buy now's buyer confirmed before leaving for the pack checkout. */
export type BuyNowReturn = {
  /** The detail page, lowercased. */
  path: string;
  /** The signed-in account, lowercased: only it may finish the purchase. */
  account: string;
  /** The cart line the dialog priced (cart/store.ts cartLineId). */
  lineId: string;
  /** The Credits price the buyer confirmed: the purchase finishes on its own only at this price or less. */
  credits: number;
  attempt: string;
  /** The pack order the top-up created; without it the record never resumes. */
  orderId?: string;
  at: number;
};

function store(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

function read<T>(key: string): T | null {
  try {
    const raw = store()?.getItem(key) ?? null;
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

/**
 * Remember where to resume after the credits land (a same-origin /shop path only). A top-up that is not Buy now's
 * (no `attempt`) replaces Buy now's record too: it is the one hand-off, and its page has nothing to finish.
 */
export function stashTopUpReturn(path: string, now: number = Date.now(), attempt?: string): void {
  if (!path.startsWith("/shop") || path.startsWith("//")) return;
  try {
    const s = store();
    if (!attempt) s?.removeItem(BUY_NOW_KEY);
    s?.setItem(KEY, JSON.stringify({ path, at: now, ...(attempt ? { attempt } : {}) }));
  } catch {
    // The purchase is still in the cart or on its page; the buyer returns by hand.
  }
}

/**
 * Read and clear the hand-off, for the order the credits page just credited: a return bound to another order belongs to
 * a top-up the buyer walked away from, and is dropped.
 */
export function takeTopUpReturn(now: number = Date.now(), orderId?: string): TopUpReturn | null {
  const s = store();
  try {
    const raw = s?.getItem(KEY) ?? null;
    s?.removeItem(KEY);
    const parsed = raw ? (JSON.parse(raw) as TopUpReturn) : null;
    if (!parsed || typeof parsed.path !== "string" || !parsed.path.startsWith("/shop") || now - parsed.at > MAX_AGE_MS) return null;
    if (parsed.orderId && parsed.orderId !== orderId) {
      s?.removeItem(BUY_NOW_KEY);
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

/**
 * Continuing an unpaid pack order: keep the hand-off that order's own top-up left (Buy now's, bound to it), drop any
 * other, so crediting this order finishes only what it was bought for.
 */
export function keepHandoffsOf(orderId: string): void {
  const s = store();
  try {
    const back = read<TopUpReturn>(KEY);
    if (!back || back.orderId !== orderId) {
      s?.removeItem(KEY);
      s?.removeItem(RESUME_CART_KEY);
    }
    if (read<BuyNowReturn>(BUY_NOW_KEY)?.orderId !== orderId) s?.removeItem(BUY_NOW_KEY);
  } catch {
    // Nothing was stored.
  }
}

/** Forget every pending resume: the top-up ended without one. */
export function clearTopUpHandoffs(): void {
  const s = store();
  try {
    s?.removeItem(KEY);
    s?.removeItem(RESUME_CART_KEY);
    s?.removeItem(BUY_NOW_KEY);
  } catch {
    // Nothing was stored.
  }
}

/**
 * Buy now leaves for the pack checkout: come back to `<path>?buy=resume`, with the record of what the buyer confirmed.
 * Returns the attempt, which `dropBuyNowTopUp` needs to drop only these records.
 */
export function stashBuyNowTopUp(r: { path: string; account: string; lineId: string; credits: number }, now: number = Date.now()): string {
  const attempt = `${now.toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  stashTopUpReturn(`${r.path}?buy=resume`, now, attempt);
  try {
    // The one hand-off, in this direction too: a cart top-up left behind must not pay the basket later.
    store()?.removeItem(RESUME_CART_KEY);
    const record: BuyNowReturn = { path: r.path.toLowerCase(), account: r.account.toLowerCase(), lineId: r.lineId, credits: r.credits, attempt, at: now };
    store()?.setItem(BUY_NOW_KEY, JSON.stringify(record));
  } catch {
    // The buyer comes back to the item and confirms again by hand.
  }
  return attempt;
}

/**
 * The pack order the cart's top-up created (a return with no attempt): continuing that order keeps the way back to the
 * cart, and another order's grant never takes it.
 */
export function bindTopUpReturn(orderId: string): void {
  try {
    const back = read<TopUpReturn>(KEY);
    if (back && !back.attempt && !back.orderId) store()?.setItem(KEY, JSON.stringify({ ...back, orderId }));
  } catch {
    // Unbound, the return still resumes for the next grant, as before.
  }
}

/** The pack order this attempt's top-up created: the only one whose grant may finish the purchase. */
export function bindBuyNowTopUp(attempt: string, orderId: string): void {
  try {
    const s = store();
    const back = read<TopUpReturn>(KEY);
    if (back?.attempt === attempt) s?.setItem(KEY, JSON.stringify({ ...back, orderId }));
    const record = read<BuyNowReturn>(BUY_NOW_KEY);
    if (record?.attempt === attempt) s?.setItem(BUY_NOW_KEY, JSON.stringify({ ...record, orderId }));
  } catch {
    // Unbound, the record never resumes: the buyer confirms again by hand.
  }
}

/** Drop the hand-off this attempt left, and nothing a newer attempt wrote since. */
export function dropBuyNowTopUp(attempt: string): void {
  const s = store();
  try {
    if (read<TopUpReturn>(KEY)?.attempt === attempt) s?.removeItem(KEY);
    if (read<BuyNowReturn>(BUY_NOW_KEY)?.attempt === attempt) s?.removeItem(BUY_NOW_KEY);
  } catch {
    // Nothing was stored.
  }
}

/**
 * Read and clear Buy now's record, if it is this page's and this account's, recent, and for the pack order the credits
 * page credited and handed over (`orderId`).
 */
export function takeBuyNowReturn(path: string, account: string | null, orderId: string | null, now: number = Date.now()): BuyNowReturn | null {
  const r = read<Partial<BuyNowReturn>>(BUY_NOW_KEY);
  try {
    store()?.removeItem(BUY_NOW_KEY);
  } catch {
    // Nothing to clear.
  }
  if (!r || !account || typeof r.path !== "string" || typeof r.account !== "string" || typeof r.lineId !== "string" || typeof r.credits !== "number") return null;
  if (typeof r.at !== "number" || typeof r.attempt !== "string" || !r.orderId || r.orderId !== orderId) return null;
  if (r.path !== path.toLowerCase() || r.account !== account.toLowerCase() || now - r.at > MAX_AGE_MS || r.at > now) return null;
  return r as BuyNowReturn;
}

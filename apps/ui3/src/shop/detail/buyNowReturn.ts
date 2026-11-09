// from: shop@3a322cf9:app/src/lib/resume-buy.ts (RESUME_BUY_KEY: the item a top-up started from)
// Buy now's own half of the top-up hand-off. The credits page sends the buyer back to `<item>?buy=resume`
// (credits/returns.ts); that query alone proves nothing, since anyone can link to it. The dialog finishes the purchase
// on its own only when this tab holds a record of the purchase the buyer confirmed: the same item page, the same cart
// line, a price no higher than the one they agreed to, and recent. It is taken once, and dropped when the buyer closes
// the dialog or the pack checkout fails to start.

const KEY = "dcl-shop-buynow-return:v1";
const MAX_AGE_MS = 30 * 60 * 1000;

export type BuyNowReturn = {
  /** The detail page, lowercased. */
  path: string;
  /** The cart line the dialog priced (cart/store.ts cartLineId). */
  lineId: string;
  /** The Credits price the buyer confirmed: a resume pays on its own only at this price or less. */
  credits: number;
  at: number;
};

function store(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

export function stashBuyNowReturn(r: Omit<BuyNowReturn, "at">, now: number = Date.now()): void {
  try {
    store()?.setItem(KEY, JSON.stringify({ ...r, path: r.path.toLowerCase(), at: now }));
  } catch {
    // The buyer comes back to the item and confirms again by hand.
  }
}

/** Read and clear the record, if it is this page's and recent. */
export function takeBuyNowReturn(path: string, now: number = Date.now()): BuyNowReturn | null {
  const s = store();
  try {
    const raw = s?.getItem(KEY) ?? null;
    s?.removeItem(KEY);
    const r = raw ? (JSON.parse(raw) as Partial<BuyNowReturn>) : null;
    if (!r || typeof r.path !== "string" || typeof r.lineId !== "string" || typeof r.credits !== "number" || typeof r.at !== "number") return null;
    if (r.path !== path.toLowerCase() || now - r.at > MAX_AGE_MS || r.at > now) return null;
    return r as BuyNowReturn;
  } catch {
    return null;
  }
}

export function clearBuyNowReturn(): void {
  try {
    store()?.removeItem(KEY);
  } catch {
    // Nothing was stored.
  }
}

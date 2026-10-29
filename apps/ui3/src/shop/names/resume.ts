// from: shop@94584a5c:app/src/lib/resume-name.ts
// The "resume this NAME purchase after topping up" hand-off. The buy-NAME modal stashes the NAME
// before sending the buyer to buy credits; the credits page reads it on return and re-opens the NAMEs
// page on it. The NAME alone, never its price: the page re-quotes and re-checks availability.

export const RESUME_NAME_KEY = "dcl_shop_resume_name";

/** The sibling hand-offs a NAME top-up supersedes (upstream RESUME_BUY_KEY, RESUME_CART_KEY). */
export const RESUME_BUY_KEY = "dcl_shop_resume_buy";
export const RESUME_CART_KEY = "dcl_shop_resume_cart";

function store(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

/** Claims the hand-off for this NAME outright: an older item or cart hand-off would otherwise win. */
export function stashNameResume(name: string): void {
  try {
    const s = store();
    s?.removeItem(RESUME_CART_KEY);
    s?.removeItem(RESUME_BUY_KEY);
    s?.setItem(RESUME_NAME_KEY, name);
  } catch {
    // Private mode: the resume will not auto-trigger; the credits still land.
  }
}

export function clearNameResume(): void {
  try {
    store()?.removeItem(RESUME_NAME_KEY);
  } catch {
    // Nothing to clear.
  }
}

/** Takes (and clears) a stashed NAME, when it is still a registrable label. */
export function takeNameResume(): string | null {
  try {
    const s = store();
    const name = s?.getItem(RESUME_NAME_KEY) ?? null;
    s?.removeItem(RESUME_NAME_KEY);
    return name && /^[A-Za-z0-9]{2,15}$/.test(name) ? name : null;
  } catch {
    return null;
  }
}

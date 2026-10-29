// Keeps the item and the return destination across a wallet connection or an account switch, which
// may leave the page (a wallet app, a sign-in redirect). After the switch the caller comes back here,
// and the purchase is revalidated for the new session before anything is bought. A cart line the
// guidance was shown for is `carry`: the cart is scoped to an account, so the Shop's wallet context
// adds it to the new account's cart once the switch lands (applyWalletReturn).

import type { CartAddInput, CartStore } from "../cart/store";

const KEY = "dcl-shop-wallet-return:v1";
const MAX_AGE_MS = 30 * 60 * 1000;

export type WalletReturn = {
  itemName: string;
  itemHref: string;
  returnTo: string;
  /** The account the buyer was signed in as when the switch started. */
  fromAccount: string | null;
  /** Cart lines to bring to the account the buyer switches to (never a price: each is re-quoted). */
  carry?: CartAddInput[];
  at: number;
};

function storage(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

function sameOriginPath(path: string): boolean {
  return path.startsWith("/") && !path.startsWith("//");
}

export function stashWalletReturn(entry: Omit<WalletReturn, "at">, now: number = Date.now()): void {
  if (!sameOriginPath(entry.itemHref) || !sameOriginPath(entry.returnTo)) return;
  try {
    storage()?.setItem(KEY, JSON.stringify({ ...entry, at: now }));
  } catch {
    // Without storage the buyer still has the item link on screen.
  }
}

/** Returns and clears the stashed destination; stale or foreign entries are dropped. */
export function takeWalletReturn(now: number = Date.now()): WalletReturn | null {
  const s = storage();
  if (!s) return null;
  try {
    const raw = s.getItem(KEY);
    s.removeItem(KEY);
    const entry = raw ? (JSON.parse(raw) as WalletReturn) : null;
    if (!entry || now - entry.at > MAX_AGE_MS || !sameOriginPath(entry.itemHref) || !sameOriginPath(entry.returnTo)) return null;
    return entry;
  } catch {
    return null;
  }
}

/**
 * After a sign-in lands on `account`: a switch away from the stashed account brings the carried lines
 * into this account's cart (the review re-quotes them for the new session); returning to the same
 * account only clears the stash. Returns what was applied, or null.
 */
export function applyWalletReturn(account: string | null, cart: Pick<CartStore, "add" | "getSnapshot">, now: number = Date.now()): WalletReturn | null {
  if (!account) return null;
  const entry = takeWalletReturn(now);
  if (!entry) return null;
  const switched = (entry.fromAccount ?? "").toLowerCase() !== account.toLowerCase();
  if (switched && cart.getSnapshot().account === account.toLowerCase()) {
    for (const line of entry.carry ?? []) cart.add({ ...line, seen: null });
  }
  return entry;
}

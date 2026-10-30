// from: shop@94584a5c:app/src/store/cart.ts
// One cart, scoped to the active account and persisted per account, plus that account's checkout
// progress. Upstream kept one global zustand cart tagged with its owner and emptied it for a
// different buyer; ui3 has no zustand, so this is a plain external store read through
// useSyncExternalStore, and each account keeps its own cart instead of losing it:
//   - signed out: the anonymous cart;
//   - signing in: the anonymous lines are adopted into the account's cart (so the sign-in that
//     checkout asks for keeps them), with their payment choice cleared;
//   - switching accounts: the other account's cart and checkout load; nothing crosses over, except a
//     line the MANA guidance's explicit switch carried (wallet/returnTo applyWalletReturn).
// A line stores what identifies the listing and a rendering snapshot, never a price authority.

import { useSyncExternalStore } from "react";

import { assetKey, parseAssetId, type AssetId } from "../model/asset";
import {
  type CartLine,
  type CartLineDisplay,
  type CartListingRef,
  type CartProvenance,
  type PurchaseOffer,
  type PurchaseState,
} from "../model/purchase";
import { decodePaymentMethod, decodePurchaseOffer, encodePurchaseOffer } from "../model/wire";
import type { PaymentMethod } from "../model/settlement";
import { applyEvent, newCheckout, type GroupEvent } from "./machine";
import type { CartGroup } from "./partition";

export type CartStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export type CartSnapshot = {
  /** Lowercase active account, or null when signed out. */
  account: string | null;
  lines: CartLine[];
  checkout: PurchaseState | null;
};

export type CartAddInput = {
  asset: AssetId;
  listing?: CartListingRef;
  display: CartLineDisplay;
  provenance?: CartProvenance;
  quantity?: number;
  /** Remaining supply of a mint, when known: the most units a line may hold. */
  available?: number | null;
  /** The server offer the buyer was shown when adding, for the review's price comparison. */
  seen?: PurchaseOffer | null;
};

export type CartStore = {
  getSnapshot(): CartSnapshot;
  getServerSnapshot(): CartSnapshot;
  subscribe(listener: () => void): () => void;
  scopeTo(account: string | null): void;
  add(input: CartAddInput): CartLine;
  remove(id: string): void;
  removeLines(ids: readonly string[]): void;
  setQuantity(id: string, quantity: number, available?: number | null): void;
  increment(id: string, available?: number | null): void;
  decrement(id: string): void;
  setMethod(id: string, method: PaymentMethod | null): void;
  markSeen(id: string, offer: PurchaseOffer | null): void;
  clear(): void;
  restore(lines: CartLine[]): void;
  startCheckout(groups: CartGroup[]): PurchaseState;
  dispatch(keyId: string, event: GroupEvent): PurchaseState;
  replaceCheckout(state: PurchaseState | null): void;
};

export const CART_KEY_PREFIX = "dcl-shop-cart:v1:";
export const CHECKOUT_KEY_PREFIX = "dcl-shop-checkout:v1:";
const ANONYMOUS = "anonymous";

const EMPTY: CartSnapshot = Object.freeze({ account: null, lines: [], checkout: null }) as CartSnapshot;

function defaultStorage(): CartStorage | null {
  try {
    return typeof localStorage !== "undefined" ? localStorage : null;
  } catch {
    return null;
  }
}

/** The stable key of a line: the asset plus the listing it buys. */
export function cartLineId(asset: AssetId, listing: CartListingRef = {}): string {
  const via = listing.tradeId ?? listing.orderId ?? listing.rentalId ?? listing.name ?? listing.acquisition ?? "trade";
  return `${assetKey(asset)}|${via}`;
}

function isMint(line: Pick<CartLine, "asset">): boolean {
  return line.asset.token.kind === "item";
}

function cap(quantity: number, available: number | null | undefined): number {
  const max = typeof available === "number" && available >= 1 ? available : Infinity;
  const n = Math.max(1, Math.min(Math.floor(quantity), max));
  return Number.isFinite(n) ? n : 1;
}

function encodeLine(line: CartLine): unknown {
  return { ...line, seen: line.seen ? encodePurchaseOffer(line.seen) : null };
}

function decodeLine(value: unknown): CartLine | null {
  if (!value || typeof value !== "object") return null;
  const v = value as Record<string, unknown>;
  const asset = parseAssetId(v.asset);
  const display = v.display as CartLineDisplay | undefined;
  if (!asset || typeof v.id !== "string" || !display || typeof display.name !== "string") return null;
  const method = v.method === null || v.method === undefined ? null : decodePaymentMethod(v.method);
  if (v.method != null && method === null) return null;
  const quantity = typeof v.quantity === "number" && Number.isFinite(v.quantity) ? v.quantity : 1;
  return {
    id: v.id,
    asset,
    quantity: isMint({ asset }) ? Math.max(1, Math.floor(quantity)) : 1,
    method,
    listing: (v.listing as CartListingRef | undefined) ?? {},
    display,
    provenance: (v.provenance as CartProvenance | undefined) ?? {},
    seen: v.seen ? decodePurchaseOffer(v.seen) : null,
    addedAt: typeof v.addedAt === "number" ? v.addedAt : 0,
  };
}

function readLines(storage: CartStorage | null, key: string): CartLine[] {
  if (!storage) return [];
  try {
    const raw = storage.getItem(key);
    const parsed = raw ? (JSON.parse(raw) as { version?: number; lines?: unknown[] }) : null;
    if (!parsed || parsed.version !== 1 || !Array.isArray(parsed.lines)) return [];
    const lines: CartLine[] = [];
    const seen = new Set<string>();
    for (const raw of parsed.lines) {
      const line = decodeLine(raw);
      if (line && !seen.has(line.id)) {
        seen.add(line.id);
        lines.push(line);
      }
    }
    return lines;
  } catch {
    return [];
  }
}

function readCheckout(storage: CartStorage | null, account: string | null): PurchaseState | null {
  if (!storage || !account) return null;
  try {
    const raw = storage.getItem(CHECKOUT_KEY_PREFIX + account);
    const parsed = raw ? (JSON.parse(raw) as PurchaseState) : null;
    return parsed && parsed.account === account && Array.isArray(parsed.groups) ? parsed : null;
  } catch {
    return null;
  }
}

export function createCartStore(opts: { storage?: () => CartStorage | null; now?: () => number } = {}): CartStore {
  const storage = opts.storage ?? defaultStorage;
  const now = opts.now ?? Date.now;
  const listeners = new Set<() => void>();
  let snapshot: CartSnapshot | null = null;

  const cartKey = (account: string | null) => CART_KEY_PREFIX + (account ?? ANONYMOUS);

  function current(): CartSnapshot {
    if (!snapshot) snapshot = { account: null, lines: readLines(storage(), cartKey(null)), checkout: null };
    return snapshot;
  }

  function persist(next: CartSnapshot) {
    const s = storage();
    if (!s) return;
    try {
      if (next.lines.length > 0) s.setItem(cartKey(next.account), JSON.stringify({ version: 1, lines: next.lines.map(encodeLine) }));
      else s.removeItem(cartKey(next.account));
      if (next.account) {
        if (next.checkout) s.setItem(CHECKOUT_KEY_PREFIX + next.account, JSON.stringify(next.checkout));
        else s.removeItem(CHECKOUT_KEY_PREFIX + next.account);
      }
    } catch {
      // Storage full or blocked: the cart still works for this page.
    }
  }

  function commit(next: CartSnapshot, save = true) {
    snapshot = next;
    if (save) persist(next);
    for (const listener of listeners) listener();
  }

  function update(fn: (lines: CartLine[]) => CartLine[]) {
    const s = current();
    const lines = fn(s.lines);
    const same = lines.length === s.lines.length && lines.every((l, i) => l === s.lines[i]);
    if (!same) commit({ ...s, lines });
  }

  const store: CartStore = {
    getSnapshot: current,
    getServerSnapshot: () => EMPTY,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    scopeTo(account) {
      const next = account ? account.toLowerCase() : null;
      const s = current();
      if (next === s.account) return;
      const st = storage();
      let lines = readLines(st, cartKey(next));
      if (s.account === null && next !== null && s.lines.length > 0) {
        const have = new Set(lines.map((l) => l.id));
        lines = [...lines, ...s.lines.filter((l) => !have.has(l.id)).map((l) => ({ ...l, method: null, seen: null }))];
        try {
          st?.removeItem(cartKey(null));
        } catch {
          // ignore
        }
      }
      commit({ account: next, lines, checkout: readCheckout(st, next) }, next !== null);
    },
    add(input) {
      const listing = input.listing ?? {};
      const id = cartLineId(input.asset, listing);
      const existing = current().lines.find((l) => l.id === id);
      if (existing) {
        if (isMint(existing)) store.setQuantity(id, existing.quantity + (input.quantity ?? 1), input.available);
        return current().lines.find((l) => l.id === id)!;
      }
      const line: CartLine = {
        id,
        asset: input.asset,
        quantity: input.asset.token.kind === "item" ? cap(input.quantity ?? 1, input.available) : 1,
        method: null,
        listing,
        display: input.display,
        provenance: input.provenance ?? {},
        seen: input.seen ?? null,
        addedAt: now(),
      };
      update((lines) => [...lines, line]);
      return line;
    },
    remove(id) {
      update((lines) => (lines.some((l) => l.id === id) ? lines.filter((l) => l.id !== id) : lines));
    },
    removeLines(ids) {
      const drop = new Set(ids);
      update((lines) => (lines.some((l) => drop.has(l.id)) ? lines.filter((l) => !drop.has(l.id)) : lines));
    },
    setQuantity(id, quantity, available) {
      update((lines) =>
        lines.map((l) => (l.id === id && isMint(l) && cap(quantity, available) !== l.quantity ? { ...l, quantity: cap(quantity, available) } : l)),
      );
    },
    increment(id, available) {
      const line = current().lines.find((l) => l.id === id);
      if (line) store.setQuantity(id, line.quantity + 1, available);
    },
    decrement(id) {
      const line = current().lines.find((l) => l.id === id);
      if (line && line.quantity > 1) store.setQuantity(id, line.quantity - 1);
    },
    setMethod(id, method) {
      update((lines) => lines.map((l) => (l.id === id && l.method !== method ? { ...l, method } : l)));
    },
    markSeen(id, offer) {
      update((lines) => lines.map((l) => (l.id === id ? { ...l, seen: offer } : l)));
    },
    clear() {
      const s = current();
      commit({ ...s, lines: [] });
    },
    restore(lines) {
      if (current().lines.length === 0 && lines.length > 0) update(() => lines.map((l) => ({ ...l })));
    },
    startCheckout(groups) {
      const s = current();
      if (!s.account) throw new Error("a checkout needs a signed-in account");
      const checkout = newCheckout(s.account, groups, now());
      commit({ ...s, checkout });
      return checkout;
    },
    dispatch(keyId, event) {
      const s = current();
      if (!s.checkout) throw new Error("no checkout in progress");
      const checkout = applyEvent(s.checkout, keyId, event, now());
      commit({ ...s, checkout });
      return checkout;
    },
    replaceCheckout(state) {
      const s = current();
      if (state && state.account !== s.account) throw new Error("a checkout belongs to its own account");
      commit({ ...s, checkout: state });
    },
  };
  return store;
}

/** The Shop's cart: one per document, shared by every Shop surface. */
export const shopCart = createCartStore();

export function useCart<T>(select: (snapshot: CartSnapshot) => T, store: CartStore = shopCart): T {
  return select(useSyncExternalStore(store.subscribe, store.getSnapshot, store.getServerSnapshot));
}

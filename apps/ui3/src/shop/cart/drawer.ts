// from: shop@3a322cf9:app/src/store/cart.ts (open, justAddedCount, setOpen; an add opens the drawer and
// counts the units it added). The drawer's state is the page's, never the cart's: it is not persisted, so
// a reload never reopens it, and it is not scoped to an account.
import { useSyncExternalStore } from "react";

export type CartDrawerState = {
  open: boolean;
  /** Units added since the drawer opened on an add: the "N Item(s) successfully added" banner. */
  justAdded: number;
};

export type CartDrawerStore = {
  getSnapshot(): CartDrawerState;
  getServerSnapshot(): CartDrawerState;
  subscribe(listener: () => void): () => void;
  /** An add opens the drawer; while it stays open, further adds add to the banner's count. */
  added(units?: number): void;
  /** The cart icon opens it with no banner: the banner belongs to an add-triggered open only. */
  open(): void;
  close(): void;
  /** A removed line takes the banner that announced it with it. */
  clearBanner(): void;
};

const CLOSED: CartDrawerState = Object.freeze({ open: false, justAdded: 0 });

export function createCartDrawer(): CartDrawerStore {
  let state: CartDrawerState = CLOSED;
  const listeners = new Set<() => void>();
  const set = (next: CartDrawerState) => {
    if (next.open === state.open && next.justAdded === state.justAdded) return;
    state = next;
    for (const listener of listeners) listener();
  };
  return {
    getSnapshot: () => state,
    getServerSnapshot: () => CLOSED,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    added(units = 1) {
      set({ open: true, justAdded: (state.open ? state.justAdded : 0) + Math.max(1, units) });
    },
    open() {
      set({ open: true, justAdded: state.open ? state.justAdded : 0 });
    },
    close() {
      set(CLOSED);
    },
    clearBanner() {
      if (state.justAdded) set({ ...state, justAdded: 0 });
    },
  };
}

/** The browser's drawer: browser-only state that no server render writes (getServerSnapshot is closed). */
export const shopCartDrawer = createCartDrawer();

export function useCartDrawer(store: CartDrawerStore = shopCartDrawer): CartDrawerState {
  return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getServerSnapshot);
}

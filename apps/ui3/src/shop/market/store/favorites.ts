// from: shop@94584a5c:app/src/store/favorites.ts
// Favorites have two homes. Signed in: the favorites service is the source of truth (synced across
// devices and with the old Marketplace); a toggle is optimistic and rolls back on failure. Signed out:
// localStorage, so a heart works before signing in. `reloadFor` swaps the mode at every session
// boundary, and a hydrate re-applies the toggles made while it was in flight so it never rewinds them.
// zustand is replaced by useSyncExternalStore, as the foundation's cart store did.

import { useSyncExternalStore } from "react";

import { shopClient, type ShopClient } from "../../api/shop";
import { fetchCatalogByIds, type FavoriteItem } from "../lib/catalog";
import { favoriteKey, fetchFavoriteIds, setFavorite } from "../lib/favorites";

const BASE_KEY = "shop-favorites";

export type FavoritesState = { items: Record<string, FavoriteItem>; status: "ready" | "loading" | "error"; account: string | null; error: unknown };

type Items = FavoritesState["items"];

function loadLocal(): Items {
  try {
    const raw = localStorage.getItem(BASE_KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    if (!parsed || typeof parsed !== "object") return {};
    const envelope = parsed as { state?: { items?: Items } };
    const items = (envelope.state?.items ?? parsed) as Items;
    const out: Items = {};
    for (const item of Object.values(items)) {
      const key = favoriteKey(item);
      if (key) out[key] = { ...item, key };
    }
    return out;
  } catch {
    return {};
  }
}

function saveLocal(items: Items): void {
  try {
    localStorage.setItem(BASE_KEY, JSON.stringify(items));
  } catch {
    // Best effort: favorites still work for this session.
  }
}

export function createFavoritesStore(getClient: () => ShopClient = shopClient) {
  let state: FavoritesState = { items: typeof localStorage !== "undefined" ? loadLocal() : {}, status: "ready", account: null, error: null };
  const listeners = new Set<() => void>();
  let epoch = 0;
  let toggleSeq = 0;
  const recent = new Map<string, { item: FavoriteItem; faved: boolean; seq: number }>();
  const generation = new Map<string, number>();

  const set = (next: Partial<FavoritesState>) => {
    state = { ...state, ...next };
    for (const l of listeners) l();
  };

  async function hydrate(): Promise<void> {
    const started = ++epoch;
    const seqAtStart = toggleSeq;
    set({ items: {}, status: "loading", error: null });
    try {
      const ids = await fetchFavoriteIds(getClient());
      const catalog = await fetchCatalogByIds(ids, getClient());
      if (epoch !== started) return;
      const items: Items = {};
      for (const item of catalog) items[item.key] = item;
      for (const [key, toggled] of recent) {
        if (toggled.seq <= seqAtStart) {
          recent.delete(key);
          continue;
        }
        if (toggled.faved) items[key] = toggled.item;
        else delete items[key];
      }
      set({ items, status: "ready" });
    } catch (error) {
      if (epoch !== started) return;
      set({ status: "error", error });
    }
  }

  return {
    getState: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    /** Swap modes on a session boundary: server-backed for an account, the local bucket otherwise. */
    reloadFor(account: string | null) {
      const next = account?.toLowerCase() ?? null;
      if (next === state.account && state.status !== "error") return;
      epoch++;
      recent.clear();
      if (!next) {
        set({ account: null, items: loadLocal(), status: "ready", error: null });
        return;
      }
      set({ account: next });
      void hydrate();
    },
    retry() {
      if (state.account) void hydrate();
    },
    isFavorite(key: string | null) {
      return !!key && !!state.items[key];
    },
    toggle(item: FavoriteItem) {
      const key = favoriteKey(item);
      if (!key) return;
      const was = !!state.items[key];
      const items = { ...state.items };
      if (was) delete items[key];
      else items[key] = item;
      set({ items, error: null });
      if (!state.account) {
        saveLocal(items);
        return;
      }
      const gen = (generation.get(key) ?? 0) + 1;
      generation.set(key, gen);
      const seq = ++toggleSeq;
      recent.set(key, { item, faved: !was, seq });
      const at = epoch;
      return setFavorite(key, !was, getClient()).catch(error => {
        if (at !== epoch || generation.get(key) !== gen) return;
        recent.delete(key);
        const rolled = { ...state.items };
        if (was) rolled[key] = item;
        else delete rolled[key];
        set({ items: rolled, error });
      });
    },
  };
}

export type FavoritesStore = ReturnType<typeof createFavoritesStore>;

export const shopFavorites = createFavoritesStore();

export function useFavorites(store: FavoritesStore = shopFavorites): FavoritesState {
  return useSyncExternalStore(store.subscribe, store.getState, store.getState);
}

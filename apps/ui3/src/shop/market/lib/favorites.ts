// from: shop@94584a5c:app/src/lib/favorites.ts; marketplace@f3956826:webapp/src/modules/vendor/decentraland/favorites/api.ts,
//       webapp/src/modules/favorites/sagas.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
//
// Favorites ("picks") and the lists they live in, over /shop-api/v1/{lists,picks}. The default list is
// one well-known row shared by every account, with picks scoped per user server-side; the Marketplace's
// own lists are private or shared. Privacy is enforced twice: the server answers a private list only to
// its owner or a grantee, and the UI renders a list's name, description and picks only when
// `canViewList` says so, whatever the server returned (a private list is never rendered to another
// account). Picks are keyed by the marketplace item id `<contract>-<itemId>`, never a trade id.

import type { FavoriteList } from "@catalyst-api/market/FavoriteList";
import type { ListPickItem } from "@catalyst-api/market/ListPickItem";
import { isShopApiError } from "../../api/client";
import { shopClient, type ShopClient } from "../../api/shop";
import { sameAddress } from "../../model/address";

/** The global default "Favorites" list (seeded in the marketplace-server DB; same id everywhere). */
export const DEFAULT_LIST_ID = "70ab6873-4a03-4eb2-b331-4b8be0e0b8af";

export type { FavoriteList, ListPickItem };

type Envelope<T> = { ok: boolean; message?: string; data: T };
type Page<T> = { results: T[]; total: number; page?: number; pages?: number; limit?: number };

function unwrap<T>(body: unknown, what: string): T {
  const b = body as Envelope<T> | null;
  if (!b || typeof b !== "object" || !("ok" in b)) return body as T;
  if (!b.ok) throw new Error(`${what}: ${b.message ?? "not ok"}`);
  return b.data;
}

const client = (c?: ShopClient) => c ?? shopClient();

/** Upstream `favoriteKey`: the favorites key of an item, or null when it has none (a token-only row). */
export function favoriteKey(item: { contractAddress?: string | null; itemId?: string | null }): string | null {
  if (!item.contractAddress || !item.itemId) return null;
  return `${item.contractAddress.toLowerCase()}-${item.itemId}`;
}

const PAGE_SIZE = 100;

/** Upstream `fetchFavoriteIds`: every favorited item id of the signed-in account's default list, newest first. */
export async function fetchFavoriteIds(c?: ShopClient, listId = DEFAULT_LIST_ID): Promise<string[]> {
  const ids: string[] = [];
  for (let offset = 0; ; offset += PAGE_SIZE) {
    const page = unwrap<Page<ListPickItem>>(await client(c).lists.picks(listId, { limit: PAGE_SIZE, offset }), "fetchFavoriteIds");
    ids.push(...page.results.map((r) => r.itemId));
    if (ids.length >= page.total || page.results.length === 0) return ids;
  }
}

/** Upstream `setFavorite`: pick or unpick an item on the default list. */
export async function setFavorite(itemKey: string, faved: boolean, c?: ShopClient): Promise<void> {
  await bulkPick(itemKey, faved ? [DEFAULT_LIST_ID] : [], faved ? [] : [DEFAULT_LIST_ID], c);
}

/** marketplace BULK_PICK: pick into some lists and unpick from others in one request. */
export async function bulkPick(itemKey: string, pickedFor: string[], unpickedFrom: string[], c?: ShopClient): Promise<{ pickedByUser: boolean }> {
  const body = {
    pickedFor: pickedFor.length > 0 ? pickedFor : undefined,
    unpickedFrom: unpickedFrom.length > 0 ? unpickedFrom : undefined,
  };
  return unwrap(await client(c).lists.pickUnpick(itemKey, body), "bulkPick");
}

export type FavoriteStats = { count: number; pickedByUser: boolean };

const STATS_BATCH_MS = 10;
const STATS_BATCH_SIZE = 50;
type StatsWaiter = { resolve: (stats: FavoriteStats) => void; reject: (error: unknown) => void };
const statsPending = new Map<string, Map<string, StatsWaiter[]>>();
let statsFlush: ReturnType<typeof setTimeout> | null = null;
let statsClient: ShopClient | undefined;

/**
 * Upstream `fetchFavoriteStats`: how many accounts saved an item and whether `address` did. Asks made in
 * one short window leave as one `GET /v1/picks/stats` per 50 keys; a missing row means zero saves.
 */
export function fetchFavoriteStats(itemKey: string, address?: string | null, c?: ShopClient): Promise<FavoriteStats> {
  statsClient = c ?? statsClient;
  const group = address?.toLowerCase() ?? "";
  return new Promise<FavoriteStats>((resolve, reject) => {
    let keys = statsPending.get(group);
    if (!keys) {
      keys = new Map();
      statsPending.set(group, keys);
    }
    const waiters = keys.get(itemKey) ?? [];
    waiters.push({ resolve, reject });
    keys.set(itemKey, waiters);
    statsFlush ??= setTimeout(() => void flushFavoriteStats(), STATS_BATCH_MS);
  });
}

async function flushFavoriteStats(): Promise<void> {
  statsFlush = null;
  const groups = [...statsPending];
  statsPending.clear();
  const c = statsClient;
  statsClient = undefined;
  await Promise.all(
    groups.flatMap(([address, keys]) => {
      const all = [...keys.keys()];
      const chunks: string[][] = [];
      for (let i = 0; i < all.length; i += STATS_BATCH_SIZE) chunks.push(all.slice(i, i + STATS_BATCH_SIZE));
      return chunks.map(async (chunk) => {
        try {
          const query: Record<string, string | string[]> = { itemId: chunk };
          if (address) query.checkingUserAddress = address;
          const rows = unwrap<{ itemId: string; count: number; pickedByUser?: boolean }[]>(await client(c).lists.picksStats(query), "fetchFavoriteStats");
          const byId = new Map(rows.map((r) => [r.itemId, { count: r.count, pickedByUser: !!r.pickedByUser }]));
          for (const key of chunk) keys.get(key)?.forEach((w) => w.resolve(byId.get(key) ?? { count: 0, pickedByUser: false }));
        } catch (error) {
          for (const key of chunk) keys.get(key)?.forEach((w) => w.reject(error));
        }
      });
    }),
  );
}

export type ListsSortBy = "name" | "createdAt" | "updatedAt";

/** marketplace FETCH_LISTS: the signer's lists (the default list first), optionally marking `itemId`'s membership. */
export async function fetchLists(
  opts: { first?: number; skip?: number; sortBy?: ListsSortBy; sortDirection?: "asc" | "desc"; itemId?: string } = {},
  c?: ShopClient,
): Promise<{ results: FavoriteList[]; total: number }> {
  const page = unwrap<Page<FavoriteList>>(
    await client(c).lists.lists({ limit: opts.first, offset: opts.skip, sortBy: opts.sortBy, sortDirection: opts.sortDirection, itemId: opts.itemId }),
    "fetchLists",
  );
  return { results: page.results, total: page.total };
}

export class ListNotAvailableError extends Error {
  override name = "ListNotAvailableError";
  constructor(readonly listId: string) {
    super("This list is private or does not exist.");
  }
}

/**
 * Whether `viewer` may see `list`: a public list is anyone's to read; a private one only its owner's or a
 * grantee's (the server marks the grant with `permission`). The default list is only ever the viewer's own.
 */
export function canViewList(list: Pick<FavoriteList, "isPrivate" | "userAddress" | "permission" | "isDefaultList">, viewer: string | null): boolean {
  if (list.isDefaultList) return !!viewer;
  if (!list.isPrivate) return true;
  if (!viewer) return false;
  return sameAddress(list.userAddress, viewer) || list.permission === "view" || list.permission === "edit";
}

export function canEditList(list: Pick<FavoriteList, "userAddress" | "permission" | "isDefaultList">, viewer: string | null): boolean {
  if (!viewer || list.isDefaultList) return false;
  return sameAddress(list.userAddress, viewer) || list.permission === "edit";
}

/**
 * marketplace GET_LIST: one list, for `viewer`. A private list the viewer cannot see, a 401/403 or a 404 all
 * surface as ListNotAvailableError, the same answer, so a probe cannot tell a private list from none.
 */
export async function getList(listId: string, viewer: string | null, c?: ShopClient): Promise<FavoriteList> {
  let list: FavoriteList;
  try {
    list = unwrap<FavoriteList>(await client(c).lists.list(listId), "getList");
  } catch (e) {
    if (isShopApiError(e) && (e.status === 404 || e.status === 405 || e.status === 501) && viewer) {
      const own = await ownList(listId, viewer, c);
      if (own) return own;
    }
    if (isShopApiError(e) && (e.status === 401 || e.status === 403 || e.status === 404)) throw new ListNotAvailableError(listId);
    throw e;
  }
  if (!list || !canViewList(list, viewer)) throw new ListNotAvailableError(listId);
  return list;
}

/**
 * The signer's own list by id, from the signed `GET /v1/lists` (scoped to its signer). A node without the
 * single-list route (v2 today) still shows its owner their list this way; nobody else's list can surface.
 */
async function ownList(listId: string, viewer: string, c?: ShopClient): Promise<FavoriteList | null> {
  try {
    const { results } = await fetchLists({ first: 100 }, c);
    return results.find((l) => l.id === listId && !l.isDefaultList && sameAddress(l.userAddress, viewer)) ?? null;
  } catch {
    return null;
  }
}

/** A list's picks, fetched only after the list itself was authorized for the viewer. */
export async function fetchListPicks(
  list: FavoriteList,
  viewer: string | null,
  opts: { first?: number; skip?: number } = {},
  c?: ShopClient,
): Promise<{ results: ListPickItem[]; total: number }> {
  if (!canViewList(list, viewer)) throw new ListNotAvailableError(list.id);
  const page = unwrap<Page<ListPickItem>>(await client(c).lists.picks(list.id, { limit: opts.first, offset: opts.skip }), "fetchListPicks");
  return { results: page.results, total: page.total };
}

export type ListDraft = { name: string; description?: string; isPrivate: boolean };

export const LIST_NAME_MAX = 32;
export const LIST_DESCRIPTION_MAX = 100;

/** The form's refusals before anything is sent (the marketplace's CreateOrEditListModal limits). */
export function validateListDraft(draft: ListDraft): "nameRequired" | "nameTooLong" | "descriptionTooLong" | null {
  const name = draft.name.trim();
  if (!name) return "nameRequired";
  if (name.length > LIST_NAME_MAX) return "nameTooLong";
  if ((draft.description ?? "").length > LIST_DESCRIPTION_MAX) return "descriptionTooLong";
  return null;
}

/** marketplace CREATE_LIST: the wire field is `private`. */
export async function createList(draft: ListDraft, c?: ShopClient): Promise<FavoriteList> {
  return unwrap(await client(c).lists.create({ name: draft.name.trim(), private: draft.isPrivate, description: draft.description }), "createList");
}

/** marketplace UPDATE_LIST: name, description, private/shared. */
export async function updateList(listId: string, draft: Partial<ListDraft>, c?: ShopClient): Promise<FavoriteList> {
  return unwrap(
    await client(c).lists.update(listId, { name: draft.name?.trim(), private: draft.isPrivate, description: draft.description }),
    "updateList",
  );
}

/** marketplace DELETE_LIST (after DELETE_LIST_START's confirmation). The default list cannot be deleted. */
export async function deleteList(listId: string, c?: ShopClient): Promise<void> {
  if (listId === DEFAULT_LIST_ID) throw new Error("The default list cannot be deleted.");
  unwrap(await client(c).lists.remove(listId), "deleteList");
}

/** Who saved an item (public: the service lists the addresses). */
export async function fetchWhoPicked(itemKey: string, opts: { first?: number; skip?: number } = {}, c?: ShopClient): Promise<{ addresses: string[]; total: number }> {
  const page = unwrap<Page<{ userAddress: string }>>(await client(c).lists.itemPicks(itemKey, { limit: opts.first, offset: opts.skip }), "fetchWhoPicked");
  return { addresses: page.results.map((p) => p.userAddress), total: page.total };
}

/** The shareable link of a list (only meaningful for a public one). */
export function listPath(listId: string): string {
  return `/shop/my-favorites/${encodeURIComponent(listId)}`;
}

// The /shop SSR loaders' server-only reads. Every read goes through ui3's ONE Shop client over the
// canonical /shop-api paths (SHOP_API_MODE=legacy sends them to the legacy aliases while a node has
// not mounted /shop-api), unsigned: the server never reads anything on a visitor's behalf, so only
// public catalogue answers are fetched and primed into the page's query cache. Each answer is primed
// under the exact key the ui3 page asks for, so the first paint and hydration show it and the page's
// own hooks keep it current (a money read such as a listing is refetched on mount, never trusted).

import { createShopClient, hostApiOptions, type ShopClient } from "@ui/shop/api";
import { fetchCollection } from "@shop/discovery/api";
import { fetchItemMeta, fetchPrimaryListingForItem, fetchTokenById, itemListingKey } from "@shop/detail/api";
import { LAND_NETWORKS } from "@shop/land/contracts";
import { itemIdFromTokenId } from "@ui/shop/detail/lib/token-id";
import { routeSegment } from "@ui/shop/discovery/lib/routes";
import { dealsRequestedIn, itemsFilterDefaults, itemsGrid, itemsQuery } from "@ui/shop/discovery/lib/itemsGrid";
import { readUrlFilters } from "@ui/shop/discovery/hooks";
import { shopFlags } from "@ui/shop/discovery/flags";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";
import { RAIL_KEYS, readDeals, readListings, readTrending } from "@ui/shop/discovery/pages/overviewRails";
import type { ShopPageId } from "@ui/shop/routes";

import { catalystUrl, processEnv, publicPageUrl, type BaseEnv } from "../catalyst/bases";
import { dappsFlagsAnswer, type DappsFlags, type DappsFlagsAnswer, type DappsFlagVariants } from "../feature-flags.server";
import { serverShopApiMode } from "./mode";
import { defaultShopSeo, type PrimedQuery, type ShopPageData, type ShopSeo } from "./pages";

/** The creator-sales switch the reads map with: the page's, primed (primeRemoteFlags) before any answer is mapped. */
const creatorSales = (): boolean => shopFlags().creatorSales;

/** How long one server read may hold the document before the page falls back to its own fetch. */
export const SHOP_SSR_READ_MS = 1_500;

/** The server's unsigned client for CATALYST_URL. */
export function serverShopClient(env: BaseEnv = processEnv(), fetchImpl?: typeof fetch): ShopClient {
  return createShopClient(hostApiOptions({ base: catalystUrl(undefined, env), mode: serverShopApiMode(env), signer: null, fetchImpl }));
}

export type ShopLoadInput = {
  request: Request;
  params: Record<string, string | undefined>;
  client?: ShopClient;
  now?: () => number;
  env?: BaseEnv;
  /** decentraland.org's rollout flags; read from the service when absent, none for an injected (test) client. */
  flags?: DappsFlags;
  /** Their variants' payloads, with `flags`; none when absent. */
  flagVariants?: DappsFlagVariants;
};

export type ShopPageLoad = { data: ShopPageData; status: number };

function deadline(request: Request): AbortSignal {
  return AbortSignal.any([request.signal, AbortSignal.timeout(SHOP_SSR_READ_MS)]);
}

/**
 * How long one public catalogue answer (a grid's first page, the overview's rails) serves every document
 * that asks for it. Each keeps the time it was read, so the page's own staleness rule (refetch after 30s)
 * still runs from the read, not from the document; ~200ms of a grid's first byte is that read.
 */
export const SHOP_SSR_SHARE_MS = 10_000;
/**
 * How long after SHOP_SSR_SHARE_MS an item or collection document still takes the last answer at once while one
 * reload runs. The listing is still refetched on mount (staleMs 0), so an older price shows only until then.
 */
export const SHOP_SSR_STALE_MS = 60_000;
const SHARED_MAX = 256;
/** `missed`: a read whose null answer means "not found" (`missIsFinal` off) answered null; it is not served past its share. */
type Shared = { at: number; read: Promise<{ value: unknown; at: number }>; reloading?: boolean; missed?: boolean };
const sharedReads = new Map<string, Shared>();

type ReadCtx = { request: Request; now: () => number; share: boolean };

/** Newest last, so the oldest entry goes first when the map is full. */
function remember(key: string, entry: Shared, trackMiss: boolean): void {
  sharedReads.delete(key);
  sharedReads.set(key, entry);
  if (sharedReads.size > SHARED_MAX) sharedReads.delete(sharedReads.keys().next().value!);
  if (trackMiss)
    entry.read.then(
      ({ value }) => {
        entry.missed = value === null || value === undefined;
      },
      () => {},
    );
}

/**
 * A public read, shared across documents for SHOP_SSR_SHARE_MS on the server's own client. With `staleMs`, an
 * answer past its share serves for that much longer at once, and the first document then starts one reload. With
 * `missIsFinal` off, a null answer (the asset is not known) is not served past its share: an item or collection
 * indexed since then renders on the next document instead of a 404.
 */
function publicRead<T>(ctx: ReadCtx, key: string, read: (signal: AbortSignal) => Promise<T>, staleMs = 0, missIsFinal = true): Promise<{ value: T; at: number }> {
  if (!ctx.share) return read(deadline(ctx.request)).then((value) => ({ value, at: ctx.now() }));
  const at = ctx.now();
  const hit = sharedReads.get(key);
  if (hit && at - hit.at < SHOP_SSR_SHARE_MS) return hit.read as Promise<{ value: T; at: number }>;
  // Not tied to this visitor's request: a later document may be waiting on the same read.
  const load = () => read(AbortSignal.timeout(SHOP_SSR_READ_MS)).then((value) => ({ value, at }));
  if (hit && !hit.missed && at - hit.at < SHOP_SSR_SHARE_MS + staleMs) {
    if (!hit.reloading) {
      hit.reloading = true;
      const next = load();
      next.then(
        () => {
          if (sharedReads.get(key) === hit) remember(key, { at, read: next }, !missIsFinal);
        },
        () => {
          hit.reloading = false;
        },
      );
    }
    return hit.read as Promise<{ value: T; at: number }>;
  }
  const pending = load();
  remember(key, { at, read: pending }, !missIsFinal);
  pending.catch(() => {
    if (sharedReads.get(key)?.read === pending) sharedReads.delete(key);
  });
  return pending;
}

async function settle<T>(read: Promise<T>): Promise<{ ok: true; value: T } | { ok: false; error: unknown }> {
  try {
    return { ok: true, value: await read };
  } catch (error) {
    return { ok: false, error };
  }
}

const ADDRESS = /^0x[0-9a-f]{40}$/;
const ITEM_ID = /^\d{1,78}$/;

/** Shares only reads whose key parts are well formed, so made-up URLs cannot push real pages out of the share. */
const sharedIf = (ctx: ReadCtx, wellFormed: boolean): ReadCtx => (wellFormed ? ctx : { ...ctx, share: false });

type Collected = { queries: PrimedQuery[]; seo: ShopSeo; status: number };

async function itemPage(ctx: ReadCtx, client: ShopClient, params: ShopLoadInput["params"], env: BaseEnv, token: boolean): Promise<Collected> {
  const { request, now } = ctx;
  const contract = (routeSegment(params.contractAddress) ?? "").toLowerCase();
  const tokenId = token ? (routeSegment(params.tokenId) ?? null) : null;
  const itemId = token ? itemIdFromTokenId(tokenId) : (routeSegment(params.itemId) ?? null);
  const path = token ? `/shop/token/${contract}/${tokenId ?? ""}` : `/shop/item/${contract}/${itemId ?? ""}`;
  const seo = defaultShopSeo(token ? "tokenDetail" : "itemDetail", publicPageUrl(request, path, env));
  if (!ADDRESS.test(contract) || (token ? !tokenId : !itemId)) return { queries: [], seo, status: 404 };

  const opts = { client, creatorSales };
  const itemCtx = sharedIf(ctx, ITEM_ID.test(itemId ?? ""));
  /**
   * One of the item's public facts, shared across documents under its page key; it keeps the time it was read. A
   * fact whose null decides a 404 (`missIsFinal` off) is read again after its share instead of served stale.
   */
  const shared = async <T>(key: readonly unknown[], read: (signal: AbortSignal) => Promise<T>, missIsFinal = true) => {
    const answer = await settle(publicRead(itemCtx, JSON.stringify(key), read, SHOP_SSR_STALE_MS, missIsFinal));
    return answer.ok ? { ok: true as const, key, ...answer.value } : answer;
  };
  const [listing, meta, collection, owned] = await Promise.all([
    // The creator's primary sale only: the page's first render reads it while the Marketplace (resales, for a web3
    // session with the rollout on) is still pending, so a resale is never server-rendered as the item's price.
    token ? Promise.resolve(null) : shared(itemListingKey(contract, itemId!, "primary"), (signal) => fetchPrimaryListingForItem(contract, itemId!, { ...opts, signal })),
    itemId ? shared(["item-meta", contract, itemId], (signal) => fetchItemMeta(contract, itemId, { ...opts, signal }), false) : Promise.resolve(null),
    // Shared with the collection page under the same key, so with the same rule for a null.
    shared(["collection-meta", contract], (signal) => fetchCollection(contract, { ...opts, signal }), false),
    // The owner changes with every transfer: read for this document only.
    token ? settle(fetchTokenById(contract, tokenId!, { ...opts, signal: deadline(request) })) : Promise.resolve(null),
  ]);

  const queries: PrimedQuery[] = [];
  for (const read of [listing, meta, collection]) if (read?.ok) queries.push([read.key, read.value, read.at]);
  if (owned?.ok) queries.push([["token", contract, tokenId], owned.value, now()]);

  const name = (owned?.ok ? owned.value?.name : undefined) || (meta?.ok ? meta.value?.name : undefined) || (listing?.ok ? listing.value?.name : undefined);
  const image = (owned?.ok ? owned.value?.image : undefined) || (meta?.ok ? meta.value?.thumbnail : undefined) || (listing?.ok ? listing.value?.thumbnail : undefined);
  const collectionName = collection.ok ? collection.value?.name : undefined;
  const fallback = name ? `${name}${collectionName ? ` from ${collectionName}` : ""} in the Decentraland Shop.` : seo.description;
  const description = (meta?.ok ? meta.value?.description : undefined) || fallback;

  // Missing only when every read answered and none knows the asset; an index outage is not a 404.
  const answered = [token ? owned : meta, token ? null : listing].filter((r) => r !== null);
  const known = !!name;
  const missing = !known && answered.length > 0 && answered.every((r) => r!.ok);
  // A LAND token's page is the Marketplace's (LAND, its owner's actions, its sales): drawn as a stripped page for
  // anyone without it, so never indexed, as /shop/lands is not.
  const land = token && Object.values(LAND_NETWORKS).some((n) => n.land.toLowerCase() === contract || n.estate.toLowerCase() === contract);
  return {
    queries,
    seo: { ...seo, title: name || seo.title, description, image: image || undefined, noindex: missing || land, canonical: land ? undefined : seo.canonical },
    status: missing ? 404 : 200,
  };
}

async function collectionPage(ctx: ReadCtx, client: ShopClient, params: ShopLoadInput["params"], env: BaseEnv): Promise<Collected> {
  const contract = routeSegment(params.contractAddress);
  const seo = defaultShopSeo("collection", publicPageUrl(ctx.request, `/shop/collection/${contract ?? ""}`, env));
  if (!contract) return { queries: [], seo, status: 404 };
  const key = ["collection-meta", contract];
  const read = await settle(publicRead(sharedIf(ctx, ADDRESS.test(contract.toLowerCase())), JSON.stringify(key), (signal) => fetchCollection(contract, { client, creatorSales, signal }), SHOP_SSR_STALE_MS, false));
  if (!read.ok) return { queries: [], seo, status: 200 };
  const found = read.value.value;
  const queries: PrimedQuery[] = [[key, found, read.value.at]];
  if (!found) return { queries, seo: { ...seo, noindex: true }, status: 404 };
  return {
    queries,
    seo: { ...seo, title: found.name, description: `The ${found.name} collection in the Decentraland Shop.` },
    status: 200,
  };
}

async function overviewPage(ctx: ReadCtx, client: ShopClient, env: BaseEnv): Promise<Collected> {
  const seo = defaultShopSeo("overview", publicPageUrl(ctx.request, "/shop/overview", env));
  const flags = shopFlags();
  // The server reads for no one, and the Marketplace is a web3 session's (market/access.tsx): primary sales only.
  const [listings, trending, deals] = await Promise.all([
    settle(publicRead(ctx, "overview-listings", (signal) => readListings({ client, creatorSales, signal }))),
    settle(publicRead(ctx, "overview-trending|primary", (signal) => readTrending("primary", { client, creatorSales, signal }))),
    flags.creatorSales ? settle(publicRead(ctx, "overview-deals", (signal) => readDeals({ client, creatorSales, signal }))) : Promise.resolve(null),
  ]);
  // Under the page's own keys (overviewRails.ts), so the page and a link's prefetch find them.
  const queries: PrimedQuery[] = [];
  if (listings.ok) queries.push([RAIL_KEYS.listings, listings.value.value, listings.value.at]);
  if (trending.ok) queries.push([RAIL_KEYS.trending("primary"), trending.value.value, trending.value.at]);
  if (deals?.ok) queries.push([RAIL_KEYS.deals, deals.value.value, deals.value.at]);
  return { queries, seo, status: 200 };
}

/** The grid's first page under the URL's filters, so the cards (the page's largest paint) arrive with the document. */
async function itemsPage(ctx: ReadCtx, client: ShopClient, env: BaseEnv): Promise<Collected> {
  const url = new URL(ctx.request.url);
  const seo = defaultShopSeo("items", publicPageUrl(ctx.request, url.pathname, env));
  const { q } = itemsQuery(url.searchParams);
  const filters = readUrlFilters(url.searchParams, itemsFilterDefaults(q, dealsRequestedIn(url.searchParams)));
  const grid = itemsGrid(filters, q);
  if (!grid.key) return { queries: [], seo, status: 200 };
  const page = await settle(publicRead(ctx, JSON.stringify(grid.key), (signal) => grid.fetchPage(0, { client, creatorSales, signal })));
  return { queries: page.ok ? [[grid.key, page.value.value, page.value.at]] : [], seo, status: 200 };
}

/** Statuses whose response has no body (a Response built with one throws). */
const NULL_BODY: ReadonlySet<number> = new Set([101, 204, 205, 304]);

/**
 * A fetch whose answers wait for `ready` (the page's flags) before they are handed on, with each body already read: the
 * body is read under the request's own deadline, which waiting first would leave running against a slow `ready`.
 */
export function readBodyThenWait(fetchImpl: typeof fetch, ready: Promise<unknown>): typeof fetch {
  return async (...args) => {
    const res = await fetchImpl(...args);
    const body = NULL_BODY.has(res.status) ? null : await res.arrayBuffer();
    await ready;
    return new Response(body, { status: res.status, statusText: res.statusText, headers: res.headers });
  };
}

/** The SSR data for one /shop page: its head, its primed public reads and the HTTP status. */
export async function loadShopPage(page: ShopPageId, input: ShopLoadInput): Promise<ShopPageLoad> {
  const env = input.env ?? processEnv();
  // The page's flags decide what the overview and the grid read (the Best Deals rail), and every answer is mapped with
  // them (creator sales). They go with the page, so the browser primes the same answer before it hydrates; their
  // variants (My Store's address list) go with them. The other reads start together with the flags read: each answer
  // waits until the flags are primed.
  const flagsRead: Promise<DappsFlagsAnswer> = input.flags
    ? Promise.resolve({ flags: input.flags, variants: input.flagVariants ?? {} })
    : input.client
      ? Promise.resolve({ flags: {}, variants: {} })
      : dappsFlagsAnswer();
  const primed = flagsRead.then((answer) => {
    primeRemoteFlags(answer.flags, answer.variants);
    return answer;
  });
  const client = input.client ?? serverShopClient(env, readBodyThenWait((...args) => fetch(...args), primed));
  const now = input.now ?? Date.now;
  const { request, params } = input;
  const pathname = new URL(request.url).pathname;
  // Only the server's own client shares reads: an injected one (a test's node) always asks.
  const ctx: ReadCtx = { request, now, share: !input.client };
  let collected: Collected;
  switch (page) {
    case "itemDetail":
    case "tokenDetail":
      collected = await itemPage(ctx, client, params, env, page === "tokenDetail");
      break;
    case "collection":
      collected = await collectionPage(ctx, client, params, env);
      break;
    case "overview":
      await primed;
      collected = await overviewPage(ctx, client, env);
      break;
    case "items":
      await primed;
      collected = await itemsPage(ctx, client, env);
      break;
    case "notFound":
      collected = { queries: [], seo: defaultShopSeo("notFound"), status: 404 };
      break;
    default:
      collected = { queries: [], seo: defaultShopSeo(page, publicPageUrl(request, pathname, env)), status: 200 };
  }
  const answer = await primed;
  const featureFlags = answer.flags;
  const featureFlagVariants = Object.keys(answer.variants).length > 0 ? answer.variants : undefined;
  return {
    data: { page, seo: collected.seo, queries: collected.queries, featureFlags, ...(featureFlagVariants ? { featureFlagVariants } : {}) },
    status: collected.status,
  };
}

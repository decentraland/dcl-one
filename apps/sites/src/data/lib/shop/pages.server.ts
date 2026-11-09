// The /shop SSR loaders' server-only reads. Every read goes through ui3's ONE Shop client over the
// canonical /shop-api paths (SHOP_API_MODE=legacy sends them to the legacy aliases while a node has
// not mounted /shop-api), unsigned: the server never reads anything on a visitor's behalf, so only
// public catalogue answers are fetched and primed into the page's query cache. Each answer is primed
// under the exact key the ui3 page asks for, so the first paint and hydration show it and the page's
// own hooks keep it current (a money read such as a listing is refetched on mount, never trusted).

import { createShopClient, hostApiOptions, type ShopClient } from "@ui/shop/api";
import { fetchCollection, fetchShopItems, fetchTrendingItems } from "@ui/shop/discovery/api";
import { fetchItemDescription, fetchItemMeta, fetchPrimaryListingForItem, fetchTokenById, itemListingKey } from "@ui/shop/detail/api";
import { LAND_NETWORKS } from "@ui/shop/land/contracts";
import { itemIdFromTokenId } from "@ui/shop/detail/lib/token-id";
import { routeSegment } from "@ui/shop/discovery/lib/routes";
import { dealsRequestedIn, itemsFilterDefaults, itemsGrid, itemsQuery } from "@ui/shop/discovery/lib/itemsGrid";
import { readUrlFilters } from "@ui/shop/discovery/hooks";
import { shopFlags } from "@ui/shop/discovery/flags";
import { primeRemoteFlags } from "@ui/shop/discovery/lib/remoteFlags";
import type { ShopPageId } from "@ui/shop/routes";

import { catalystUrl, processEnv, publicCatalystUrl, type BaseEnv } from "../catalyst/bases";
import { dappsFlagsAnswer, type DappsFlags, type DappsFlagsAnswer, type DappsFlagVariants } from "../feature-flags.server";
import { serverShopApiMode } from "./mode";
import { defaultShopSeo, type PrimedQuery, type ShopPageData, type ShopSeo } from "./pages";

/** The creator-sales switch the reads map with: the page's, primed (primeRemoteFlags) before any read. */
const creatorSales = (): boolean => shopFlags().creatorSales;

/** How long one server read may hold the document before the page falls back to its own fetch. */
export const SHOP_SSR_READ_MS = 1_500;

let shared: { key: string; client: ShopClient } | null = null;

/** The server's unsigned client for CATALYST_URL, rebuilt when the base or the mode changes. */
export function serverShopClient(env: BaseEnv = processEnv(), fetchImpl?: typeof fetch): ShopClient {
  const base = catalystUrl(undefined, env);
  const mode = serverShopApiMode(env);
  const key = `${base}|${mode}`;
  if (fetchImpl) return createShopClient(hostApiOptions({ base, mode, signer: null, fetchImpl }));
  if (!shared || shared.key !== key) shared = { key, client: createShopClient(hostApiOptions({ base, mode, signer: null })) };
  return shared.client;
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
const SHARED_MAX = 256;
const sharedReads = new Map<string, { at: number; read: Promise<{ value: unknown; at: number }> }>();

type ReadCtx = { request: Request; now: () => number; share: boolean };

/** A public read, shared across documents for SHOP_SSR_SHARE_MS on the server's own client. */
function publicRead<T>(ctx: ReadCtx, key: string, read: (signal: AbortSignal) => Promise<T>): Promise<{ value: T; at: number }> {
  if (!ctx.share) return read(deadline(ctx.request)).then((value) => ({ value, at: ctx.now() }));
  const at = ctx.now();
  const hit = sharedReads.get(key);
  if (hit && at - hit.at < SHOP_SSR_SHARE_MS) return hit.read as Promise<{ value: T; at: number }>;
  // Not tied to this visitor's request: a later document may be waiting on the same read.
  const pending = read(AbortSignal.timeout(SHOP_SSR_READ_MS)).then((value) => ({ value, at }));
  sharedReads.set(key, { at, read: pending });
  pending.catch(() => {
    if (sharedReads.get(key)?.read === pending) sharedReads.delete(key);
  });
  if (sharedReads.size > SHARED_MAX) sharedReads.delete(sharedReads.keys().next().value!);
  return pending;
}

async function settle<T>(read: Promise<T>): Promise<{ ok: true; value: T } | { ok: false; error: unknown }> {
  try {
    return { ok: true, value: await read };
  } catch (error) {
    return { ok: false, error };
  }
}

/** The public origin a canonical URL names: PUBLIC_CATALYST_URL when set, else the request's. */
function canonicalFor(request: Request, pathname: string, env: BaseEnv): string {
  const configured = env.PUBLIC_CATALYST_URL?.trim() ? publicCatalystUrl(env) : new URL(request.url).origin;
  return `${configured.replace(/\/$/, "")}${pathname}`;
}

const ADDRESS = /^0x[0-9a-f]{40}$/;

type Collected = { queries: PrimedQuery[]; seo: ShopSeo; status: number };

async function itemPage(input: Required<Pick<ShopLoadInput, "request" | "params" | "client">> & { now: () => number; env: BaseEnv; token: boolean }): Promise<Collected> {
  const { request, params, client, now, env, token } = input;
  const contract = (routeSegment(params.contractAddress) ?? "").toLowerCase();
  const tokenId = token ? (routeSegment(params.tokenId) ?? null) : null;
  const itemId = token ? itemIdFromTokenId(tokenId) : (routeSegment(params.itemId) ?? null);
  const path = token ? `/shop/token/${contract}/${tokenId ?? ""}` : `/shop/item/${contract}/${itemId ?? ""}`;
  const seo = defaultShopSeo(token ? "tokenDetail" : "itemDetail", canonicalFor(request, path, env));
  if (!ADDRESS.test(contract) || (token ? !tokenId : !itemId)) return { queries: [], seo, status: 404 };

  const signal = deadline(request);
  const opts = { client, creatorSales, signal };
  const [listing, meta, desc, collection, owned] = await Promise.all([
    // The creator's primary sale only: the page's first render reads it while the Marketplace (resales, for a web3
    // session with the rollout on) is still pending, so a resale is never server-rendered as the item's price.
    token ? Promise.resolve(null) : settle(fetchPrimaryListingForItem(contract, itemId!, opts)),
    itemId ? settle(fetchItemMeta(contract, itemId, opts)) : Promise.resolve(null),
    itemId ? settle(fetchItemDescription(contract, itemId, opts)) : Promise.resolve(null),
    settle(fetchCollection(contract, opts)),
    token ? settle(fetchTokenById(contract, tokenId!, opts)) : Promise.resolve(null),
  ]);

  const at = now();
  const queries: PrimedQuery[] = [];
  if (listing?.ok) queries.push([[...itemListingKey(contract, itemId!, "primary")], listing.value, at]);
  if (meta?.ok) queries.push([["item-meta", contract, itemId], meta.value, at]);
  if (desc?.ok) queries.push([["item-desc", contract, itemId], desc.value, at]);
  if (collection.ok) queries.push([["collection-meta", contract], collection.value, at]);
  if (owned?.ok) queries.push([["token", contract, tokenId], owned.value, at]);

  const name = (owned?.ok ? owned.value?.name : undefined) || (meta?.ok ? meta.value?.name : undefined) || (listing?.ok ? listing.value?.name : undefined);
  const image = (owned?.ok ? owned.value?.image : undefined) || (meta?.ok ? meta.value?.thumbnail : undefined) || (listing?.ok ? listing.value?.thumbnail : undefined);
  const collectionName = collection.ok ? collection.value?.name : undefined;
  const description = desc?.ok && desc.value ? desc.value : name ? `${name}${collectionName ? ` from ${collectionName}` : ""} in the Decentraland Shop.` : seo.description;

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

async function collectionPage(request: Request, params: ShopLoadInput["params"], client: ShopClient, now: () => number, env: BaseEnv): Promise<Collected> {
  const contract = routeSegment(params.contractAddress);
  const seo = defaultShopSeo("collection", canonicalFor(request, `/shop/collection/${contract ?? ""}`, env));
  if (!contract) return { queries: [], seo, status: 404 };
  const found = await settle(fetchCollection(contract, { client, creatorSales, signal: deadline(request) }));
  if (!found.ok) return { queries: [], seo, status: 200 };
  const queries: PrimedQuery[] = [[["collection-meta", contract], found.value, now()]];
  if (!found.value) return { queries, seo: { ...seo, noindex: true }, status: 404 };
  return {
    queries,
    seo: { ...seo, title: found.value.name, description: `The ${found.value.name} collection in the Decentraland Shop.` },
    status: 200,
  };
}

async function overviewPage(ctx: ReadCtx, client: ShopClient, env: BaseEnv): Promise<Collected> {
  const seo = defaultShopSeo("overview", canonicalFor(ctx.request, "/shop/overview", env));
  const flags = shopFlags();
  // The server reads for no one, and the Marketplace is a web3 session's (market/access.tsx): primary sales only.
  const [listings, trending, deals] = await Promise.all([
    settle(publicRead(ctx, "overview-listings", (signal) => fetchShopItems({ first: 12, sortBy: "newest", listingType: "primary" }, { client, creatorSales, signal }))),
    settle(publicRead(ctx, "overview-trending|primary", (signal) => fetchTrendingItems({ first: 12, listingType: "primary" }, { client, creatorSales, signal }))),
    flags.creatorSales
      ? settle(publicRead(ctx, "overview-deals", (signal) => fetchShopItems({ first: 12, discounted: true, sortBy: "discount", listingType: "primary" }, { client, creatorSales, signal })))
      : Promise.resolve(null),
  ]);
  const queries: PrimedQuery[] = [];
  if (listings.ok) queries.push([["overview-listings"], listings.value.value, listings.value.at]);
  if (trending.ok) queries.push([["overview-trending", "primary"], trending.value.value, trending.value.at]);
  if (deals?.ok) queries.push([["overview-deals"], deals.value.value, deals.value.at]);
  return { queries, seo, status: 200 };
}

/** The grid's first page under the URL's filters, so the cards (the page's largest paint) arrive with the document. */
async function itemsPage(ctx: ReadCtx, client: ShopClient, env: BaseEnv): Promise<Collected> {
  const url = new URL(ctx.request.url);
  const seo = defaultShopSeo("items", canonicalFor(ctx.request, url.pathname, env));
  const { q } = itemsQuery(url.searchParams);
  const filters = readUrlFilters(url.searchParams, itemsFilterDefaults(q, dealsRequestedIn(url.searchParams)));
  const grid = itemsGrid(filters, q);
  if (!grid.key) return { queries: [], seo, status: 200 };
  const page = await settle(publicRead(ctx, JSON.stringify(grid.key), (signal) => grid.fetchPage(0, { client, creatorSales, signal })));
  return { queries: page.ok ? [[grid.key, page.value.value, page.value.at]] : [], seo, status: 200 };
}

/** The SSR data for one /shop page: its head, its primed public reads and the HTTP status. */
export async function loadShopPage(page: ShopPageId, input: ShopLoadInput): Promise<ShopPageLoad> {
  const env = input.env ?? processEnv();
  const client = input.client ?? serverShopClient(env);
  // The page's flags decide what it reads (the Best Deals rail): primed here before any read, and sent with the page
  // so the browser primes the same answer before it hydrates. Their variants (My Store's address list) go with them.
  const answer: DappsFlagsAnswer = input.flags
    ? { flags: input.flags, variants: input.flagVariants ?? {} }
    : input.client
      ? { flags: {}, variants: {} }
      : await dappsFlagsAnswer();
  const featureFlags = answer.flags;
  const featureFlagVariants = Object.keys(answer.variants).length > 0 ? answer.variants : undefined;
  primeRemoteFlags(featureFlags, answer.variants);
  const now = input.now ?? Date.now;
  const { request, params } = input;
  const pathname = new URL(request.url).pathname;
  // Only the server's own client shares reads: an injected one (a test's node) always asks.
  const ctx: ReadCtx = { request, now, share: !input.client };
  let collected: Collected;
  switch (page) {
    case "itemDetail":
    case "tokenDetail":
      collected = await itemPage({ request, params, client, now, env, token: page === "tokenDetail" });
      break;
    case "collection":
      collected = await collectionPage(request, params, client, now, env);
      break;
    case "overview":
      collected = await overviewPage(ctx, client, env);
      break;
    case "items":
      collected = await itemsPage(ctx, client, env);
      break;
    case "notFound":
      collected = { queries: [], seo: defaultShopSeo("notFound"), status: 404 };
      break;
    default:
      collected = { queries: [], seo: defaultShopSeo(page, canonicalFor(request, pathname, env)), status: 200 };
  }
  return {
    data: { page, seo: collected.seo, queries: collected.queries, featureFlags, ...(featureFlagVariants ? { featureFlagVariants } : {}) },
    status: collected.status,
  };
}

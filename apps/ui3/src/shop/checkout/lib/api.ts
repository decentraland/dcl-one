// from: shop@94584a5c:app/src/lib/api.ts (usdWeiToCents, TradeNotFoundError, fetchTrade, resolveLiveTrade,
//       resolveLiveCoupon, fetchTradeDisplay, fetchTradeForItem, fetchAssetDisplay, fetchStoreMintState,
//       pickItemListing, SaleRecord/fetchUserSales, fetchOwnsItem);
//       marketplace@f3956826:webapp/src/modules/activity/actions.ts (FETCH_USER_ACTIVITY_REQUEST),
//       webapp/src/modules/sagas.ts (the Transak and Wert gateway calls). Marketplace UI: Apache-2.0,
//       Copyright 2020 Metaverse Holdings Ltd.
// The Shopping area's slice of upstream's `lib/api.ts`, on the ONE client (api/shop.ts): what the
// purchase rails, the receipts and the activity feed read. The shared `ui3/src/shop/lib/api.ts`
// (Discovery's catalogue mappers) can re-export these under the same names.

import { isShopApiError } from "../../api/client";
import { shopClient, type ShopClient } from "../../api/shop";
import type { UnifiedListing } from "@catalyst-api/market/UnifiedListing";
import { centsFromUsdWeiCeil } from "../../model/money";
import type { SaleRecord } from "../../history/activity";
import { ASSET_TYPE, type ListingCoupon, type SignedTrade } from "../purchase/encoding";

/** Upstream `usdWeiToCents`: a USD-pegged amount (1e18 = $1) in cents, rounded UP; 0 when unparseable. */
export function usdWeiToCents(amount?: string | null): number {
  if (!amount) return 0;
  try {
    return centsFromUsdWeiCeil(BigInt(amount)) ?? 0;
  } catch {
    return 0;
  }
}

/** A trade id that no longer resolves: it was re-signed (availability, window) or it sold. */
export class TradeNotFoundError extends Error {
  override name = "TradeNotFoundError";
  constructor(readonly tradeId: string) {
    super("fetchTrade 404");
  }
}

export function isTradeNotFound(e: unknown): e is TradeNotFoundError {
  return e instanceof TradeNotFoundError || (e as { name?: string } | null)?.name === "TradeNotFoundError";
}

function unwrap<T>(body: unknown): T {
  const b = body as { data?: T } | T;
  return ((b as { data?: T })?.data ?? b) as T;
}

/** GET /shop-api/v1/trades/{id}: the signed trade needed to execute a purchase. */
export async function fetchTrade(tradeId: string, client: ShopClient = shopClient()): Promise<SignedTrade> {
  try {
    return unwrap<SignedTrade>(await client.market.trade<unknown>(tradeId));
  } catch (e) {
    if (isShopApiError(e) && e.status === 404) throw new TradeNotFoundError(tradeId);
    throw e;
  }
}

/** Upstream `pickItemListing`: the mint over a resale, then USD-pegged over legacy. */
export function pickItemListing(items: UnifiedListing[]): UnifiedListing | null {
  const rank = (l: UnifiedListing) => (l.tokenId ? 2 : 0) + (l.source === "native" ? 0 : 1);
  return [...items].sort((a, b) => rank(a) - rank(b))[0] ?? null;
}

/** The unified-feed listing an item page and the cart price from (`/v3/catalog/unified`). */
export async function fetchUnifiedListingForItem(contractAddress: string, itemId: string, client: ShopClient = shopClient()): Promise<UnifiedListing | null> {
  const body = await client.catalog.unified<{ data?: UnifiedListing[] }>({ contractAddress, itemId, first: 5 });
  return pickItemListing(body?.data ?? []);
}

/** The live trade of an item (primary), or null when it is not on sale. */
export async function fetchTradeForItem(contractAddress: string, itemId: string, client: ShopClient = shopClient()): Promise<SignedTrade | null> {
  const listing = await fetchUnifiedListingForItem(contractAddress, itemId, client);
  return listing?.tradeId ? fetchTrade(listing.tradeId, client) : null;
}

/**
 * An item's CURRENT trade, tolerant of a stale id: the known id first, and on a 404 the item's live
 * listing (only ever by the same item, so a caller cannot end up buying an unrelated trade). Other
 * failures propagate: a transient error must never swap trades.
 */
export async function resolveLiveTrade(
  item: { tradeId?: string; contractAddress: string; itemId?: string | null },
  client: ShopClient = shopClient(),
): Promise<SignedTrade | null> {
  if (item.tradeId) {
    try {
      return await fetchTrade(item.tradeId, client);
    } catch (e) {
      if (!isTradeNotFound(e) || !item.itemId) throw e;
    }
  }
  return item.itemId ? fetchTradeForItem(item.contractAddress, item.itemId, client) : null;
}

/** The creator discount on an item's live listing, re-read at checkout; undefined when none (or unreadable). */
export async function resolveLiveCoupon(
  item: { contractAddress: string; itemId?: string | null },
  client: ShopClient = shopClient(),
): Promise<ListingCoupon | undefined> {
  if (!item.itemId) return undefined;
  try {
    return (await fetchUnifiedListingForItem(item.contractAddress, item.itemId, client))?.coupon ?? undefined;
  } catch {
    return undefined;
  }
}

/** A mint's LIVE price and remaining supply, or null when it is no longer a store mint. */
export async function fetchStoreMintState(contractAddress: string, itemId: string, client: ShopClient = shopClient()): Promise<{ priceWei: string; available: number } | null> {
  const body = await client.catalog.unified<{ data?: UnifiedListing[] }>({ contractAddress, itemId, first: 1 });
  const row = body?.data?.[0];
  if (!row || row.acquisition !== "store" || !row.manaWei) return null;
  return { priceWei: row.manaWei, available: typeof row.available === "number" ? row.available : 0 };
}

/** What a receipt or an activity line shows for a bought asset. */
export type PurchaseDisplay = { name: string; thumbnail: string; contractAddress: string; tokenId?: string; itemId?: string };

type ItemRow = { name?: string; thumbnail?: string };
type NftRow = { nft?: { name?: string; image?: string } };

async function itemMeta(contractAddress: string, itemId: string, client: ShopClient): Promise<ItemRow | null> {
  const body = await client.market.items<{ data?: ItemRow[] }>({ contractAddress, itemId, first: 1 }).catch(() => null);
  return body?.data?.[0] ?? null;
}

async function nftMeta(contractAddress: string, tokenId: string, client: ShopClient): Promise<{ name?: string; image?: string } | null> {
  const body = await client.market.nfts<{ data?: NftRow[] }>({ contractAddress, tokenId, first: 1 }).catch(() => null);
  return body?.data?.[0]?.nft ?? null;
}

/** Name and thumbnail for a sold or bought asset: a token (resale) or an item (mint). Never throws. */
export async function fetchAssetDisplay(
  contractAddress: string,
  { tokenId, itemId }: { tokenId?: string | null; itemId?: string | null },
  client: ShopClient = shopClient(),
): Promise<PurchaseDisplay | null> {
  if (!contractAddress) return null;
  if (tokenId) {
    const meta = await nftMeta(contractAddress, tokenId, client);
    return { name: meta?.name ?? `#${tokenId}`, thumbnail: meta?.image ?? "", contractAddress, tokenId };
  }
  if (itemId) {
    const meta = await itemMeta(contractAddress, itemId, client);
    return { name: meta?.name ?? "Item", thumbnail: meta?.thumbnail ?? "", contractAddress, itemId };
  }
  return null;
}

/** A purchased trade resolved to something renderable (the sold asset's typed id, never the on-chain `value`). */
export async function fetchTradeDisplay(tradeId: string, client: ShopClient = shopClient()): Promise<PurchaseDisplay | null> {
  let trade: SignedTrade;
  try {
    trade = await fetchTrade(tradeId, client);
  } catch {
    return null;
  }
  const sent = trade.sent?.[0];
  const contractAddress = sent?.contractAddress ?? "";
  if (!contractAddress) return { name: "Item", thumbnail: "", contractAddress: "" };
  if (sent?.assetType === ASSET_TYPE.COLLECTION_ITEM) {
    const id = sent.itemId ?? sent.value ?? "";
    return id ? fetchAssetDisplay(contractAddress, { itemId: id }, client) : { name: "Item", thumbnail: "", contractAddress };
  }
  const id = sent?.tokenId ?? sent?.value ?? "";
  return id ? fetchAssetDisplay(contractAddress, { tokenId: id }, client) : { name: "Item", thumbnail: "", contractAddress };
}

type RawSale = {
  id: string;
  buyer: string;
  seller: string;
  contractAddress: string;
  tokenId: string | null;
  itemId: string | null;
  price: string;
  timestamp: number;
  txHash: string;
  category?: string;
  type?: string;
  chainId?: number;
};

function toSaleRecord(s: RawSale): SaleRecord {
  return {
    id: s.id,
    buyer: s.buyer,
    seller: s.seller,
    contractAddress: s.contractAddress,
    tokenId: s.tokenId ?? null,
    itemId: s.itemId ?? null,
    manaWei: s.price,
    createdAt: s.timestamp,
    txHash: s.txHash,
    category: s.category ?? s.type ?? "",
    chainId: typeof s.chainId === "number" ? s.chainId : null,
  };
}

/** The account's completed sales, by side (seller: what it sold; buyer: what it bought with MANA). MANA wei. */
export async function fetchUserSales(
  address: string,
  { role = "seller", first = 24, skip = 0 }: { role?: "seller" | "buyer"; first?: number; skip?: number } = {},
  client: ShopClient = shopClient(),
): Promise<{ items: SaleRecord[]; total: number }> {
  const body = await client.market.sales<{ data?: RawSale[]; total?: number }>({
    [role]: address.toLowerCase(),
    first,
    skip,
    sortBy: "recently_sold",
  });
  const items = (body?.data ?? []).map(toSaleRecord);
  return { items, total: body?.total ?? items.length };
}

/** Does `owner` hold a token of this item? (the success page's ownership check). */
export async function fetchOwnsItem(owner: string, contractAddress: string, itemId: string, client: ShopClient = shopClient()): Promise<boolean> {
  const body = await client.market.nfts<{ total?: number; data?: unknown[] }>({ owner: owner.toLowerCase(), contractAddress, itemId, first: 1 }).catch(() => null);
  return (body?.total ?? body?.data?.length ?? 0) > 0;
}

/** One entry of the signed wallet activity feed (`GET /shop-api/v1/activity`, marketplace FETCH_USER_ACTIVITY_REQUEST). */
export type WalletActivity = {
  id?: string;
  type?: string;
  txHash?: string;
  chainId?: number;
  status?: string;
  timestamp?: number;
  [key: string]: unknown;
};

/** The connected wallet's activity. Signed with the marketplace signer metadata. */
export async function fetchUserActivity(query: Record<string, string | number | undefined> = {}, client: ShopClient = shopClient()): Promise<{ items: WalletActivity[]; total: number }> {
  const body = await client.market.activity<{ data?: WalletActivity[]; total?: number } | WalletActivity[]>(query);
  const items = Array.isArray(body) ? body : (body?.data ?? []);
  return { items, total: Array.isArray(body) ? items.length : (body?.total ?? items.length) };
}

/** POST /shop-api/v1/transak/widget-url: the widget URL for buying MANA (or an asset) by card. */
export async function fetchTransakWidgetUrl(params: Record<string, unknown>, client: ShopClient = shopClient()): Promise<string> {
  const body = await client.onramp.transakWidgetUrl<{ data?: { widgetUrl?: string }; widgetUrl?: string; url?: string }>(params);
  const url = body?.data?.widgetUrl ?? body?.widgetUrl ?? body?.url;
  if (!url || !/^https:\/\//.test(url)) throw new Error("The card provider did not return a page to open");
  return url;
}

export type TransakOrderStatus = { status: string; isNFTOrder?: boolean; transactionHash?: string | null; errorMessage?: string | null };

/** GET /shop-api/v1/transak/orders/{id}: a card order's status. */
export async function fetchTransakOrder(orderId: string, client: ShopClient = shopClient()): Promise<TransakOrderStatus> {
  const body = await client.onramp.transakOrder<{ data?: TransakOrderStatus } | TransakOrderStatus>(orderId);
  return unwrap<TransakOrderStatus>(body);
}

/** POST /shop-api/v1/wert/sign: a signed Wert session for a card purchase. */
export async function signWertSession(message: Record<string, unknown>, client: ShopClient = shopClient()): Promise<{ signature: string; sessionId?: string }> {
  return unwrap<{ signature: string; sessionId?: string }>(await client.onramp.wertSign<unknown>(message));
}

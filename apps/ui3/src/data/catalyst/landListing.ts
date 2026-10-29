import { getJSON, type RequestOpts } from "./client";

export const LAND_CONTRACT = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
export const ESTATE_CONTRACT = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
export const LAND_LISTING_STALE_MS = 5 * 60 * 1000;

export type LandListing = {
  kind: "parcel" | "estate";
  name: string;
  priceMana: number;
  contractAddress: string;
  tokenId: string;
  marketplacePath: string;
};

type MarketOrder = { status?: string; price?: string; expiresAt?: number };
type MarketNft = {
  contractAddress?: string;
  tokenId?: string;
  name?: string;
  data?: { parcel?: { estate?: { tokenId?: string; name?: string } | null } };
};
type MarketNftsResponse = { data?: { nft?: MarketNft; order?: MarketOrder | null }[] };

export function landTokenId(x: number, y: number): string {
  return ((BigInt.asUintN(128, BigInt(x)) << 128n) | BigInt.asUintN(128, BigInt(y))).toString();
}

export function parseParcel(coords: string | null | undefined): [number, number] | null {
  const m = /^\s*(-?\d+)\s*,\s*(-?\d+)\s*$/.exec(coords ?? "");
  return m ? [Number(m[1]), Number(m[2])] : null;
}

function weiToMana(wei: string): number {
  try { return Number(BigInt(wei) / 10n ** 14n) / 1e4; } catch { return NaN; }
}

function orderIsLive(order: MarketOrder | null | undefined, nowMs: number): order is MarketOrder & { price: string } {
  if (!order || order.status !== "open" || typeof order.price !== "string") return false;
  if (order.expiresAt == null) return true;
  const expiresMs = order.expiresAt < 1e12 ? order.expiresAt * 1000 : order.expiresAt;
  return expiresMs > nowMs;
}

export function marketplacePath(contractAddress: string, tokenId: string): string {
  return `/shop/token/${contractAddress.toLowerCase()}/${tokenId}`;
}

async function fetchNft(contractAddress: string, tokenId: string, opts: RequestOpts) {
  const body = await getJSON<MarketNftsResponse>("/v1/nfts", { ...opts, service: "market", query: { contractAddress, tokenId, first: 1 } });
  return body.data?.[0] ?? null;
}

export async function fetchLandListing(coords: string, opts: RequestOpts & { now?: number } = {}): Promise<LandListing | null> {
  const parcel = parseParcel(coords);
  if (!parcel) return null;
  const now = opts.now ?? Date.now();
  const tokenId = landTokenId(parcel[0], parcel[1]);
  const land = await fetchNft(LAND_CONTRACT, tokenId, opts);
  if (land && orderIsLive(land.order, now)) {
    return { kind: "parcel", name: land.nft?.name || coords, priceMana: weiToMana(land.order.price), contractAddress: LAND_CONTRACT, tokenId, marketplacePath: marketplacePath(LAND_CONTRACT, tokenId) };
  }
  const estateId = land?.nft?.data?.parcel?.estate?.tokenId;
  if (!estateId) return null;
  const estate = await fetchNft(ESTATE_CONTRACT, estateId, opts);
  if (!estate || !orderIsLive(estate.order, now)) return null;
  return { kind: "estate", name: estate.nft?.name || land?.nft?.data?.parcel?.estate?.name || "Estate", priceMana: weiToMana(estate.order.price), contractAddress: ESTATE_CONTRACT, tokenId: estateId, marketplacePath: marketplacePath(ESTATE_CONTRACT, estateId) };
}

export function describeListing(listing: LandListing): string {
  const price = Number.isFinite(listing.priceMana) ? `${listing.priceMana.toLocaleString("en-US", { maximumFractionDigits: 4 })} MANA` : "an undisclosed price";
  return listing.kind === "estate"
    ? `Part of ${listing.name}, listed for ${price}. Open on the marketplace.`
    : `This parcel is listed for ${price}. Open on the marketplace.`;
}

export const landListingKeys = { listing: (coords: string | null) => ["land-listing", coords] as const };

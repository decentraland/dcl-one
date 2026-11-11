import { z } from "zod";

import { formatManaPrice, parseManaWei } from "@shop/model/money";

import { CatalystError, getJSON, type GetOptions } from "../client";
import { dataTotalOf } from "../envelope";
import { CatalogItemSchema, NftSchema, OrderSchema } from "../generated-schemas/market";
import { warnInvalid } from "../warn";

/** What a profile's Creations tab filters by: the catalogue's top-level category. */
export type CreationsCategory = "wearable" | "emote";

/** What a profile's Assets tab filters by; no category is every asset. */
export type AssetCategory = "wearable" | "emote" | "ens" | "parcel" | "estate";

export const ASSET_CATEGORIES: readonly AssetCategory[] = ["wearable", "emote", "ens", "parcel", "estate"];

export type Rarity = "common" | "uncommon" | "rare" | "epic" | "legendary" | "exotic" | "mythic" | "unique";

const RARITIES = new Set<string>(["common", "uncommon", "rare", "epic", "legendary", "exotic", "mythic", "unique"]);

/** The wearable or emote facts a card shows as badges; all absent for a NAME or LAND. */
export type CollectibleTraits = {
  /** The wearable slot (`upper_body`, `hat`...) or the emote kind. */
  category?: string;
  bodyShapes: string[];
  isSmart: boolean;
};

/** One item a wallet created, as the Creations tab shows it. */
export type ProfileCreation = {
  id: string;
  contractAddress: string;
  itemId: string;
  name: string;
  /** An http(s) thumbnail, or absent. */
  thumbnail?: string;
  /** Absent when the catalogue sends a rarity this list does not know. */
  rarity?: Rarity;
  network: "ethereum" | "polygon";
  /** The primary sale price, else the cheapest listing, formatted ("1,500"); absent when not for sale. */
  price?: string;
  traits: CollectibleTraits;
};

/** One token a wallet owns, as the Assets tab shows it. */
export type ProfileAsset = {
  id: string;
  contractAddress: string;
  tokenId: string;
  /** `wearable`, `emote`, `ens`, `parcel` or `estate`, as the catalogue names it. */
  category: string;
  /** For a NAME, its label without `.dcl.eth`; never blank (see `assetName`). */
  name: string;
  image?: string;
  rarity?: Rarity;
  network: "ethereum" | "polygon";
  /** The open listing's price, formatted; absent when the token is not listed. */
  price?: string;
  traits: CollectibleTraits;
};

/**
 * One read's rows and how many the catalogue holds in all. `received` counts the rows the catalogue
 * sent, before any that fail to parse are left out, so paging follows the catalogue's offsets.
 */
export type CollectiblePage<T> = { items: T[]; received: number; total: number };

/** Only the fields the cards show are required, so an unrelated catalogue change does not drop a row. */
const CreationRowSchema = CatalogItemSchema.pick({
  id: true,
  itemId: true,
  name: true,
  thumbnail: true,
  contractAddress: true,
  rarity: true,
  network: true,
  price: true,
  minListingPrice: true,
  isOnSale: true,
  available: true,
}).extend({ data: z.record(z.string(), z.unknown()).nullish() });

const AssetRowSchema = z.object({
  nft: NftSchema.pick({
    id: true,
    category: true,
    contractAddress: true,
    tokenId: true,
    name: true,
    image: true,
    network: true,
  }).extend({
    // Most LAND has no name; a missing one gets the catalogue's fallback, not a dropped row.
    name: z.string().nullish(),
    data: z.record(z.string(), z.unknown()).nullish(),
  }),
  order: OrderSchema.pick({ price: true }).nullish(),
});

const PageSchema = dataTotalOf(z.unknown());

function requirePage(raw: unknown, path: string): { data: unknown[]; total: number } {
  const page = PageSchema.safeParse(raw);
  if (!page.success) throw new CatalystError("the response did not have the paged shape", path);
  return page.data;
}

/** The rows that parse; one that does not is reported and left out, never shown half-read. */
function collect<T>(rows: unknown[], parse: (raw: unknown) => T | null): T[] {
  const out: T[] = [];
  for (const raw of rows) {
    const row = parse(raw);
    if (row) out.push(row);
  }
  return out;
}

/** Only https images: the site is served over https, so an http one would be mixed content. */
function httpsUrl(value: string | null | undefined): string | undefined {
  const url = value?.trim() ?? "";
  return /^https:\/\//i.test(url) ? url : undefined;
}

function rarityOf(value: unknown): Rarity | undefined {
  return typeof value === "string" && RARITIES.has(value.toLowerCase()) ? (value.toLowerCase() as Rarity) : undefined;
}

function networkOf(value: string): "ethereum" | "polygon" {
  return value === "ETHEREUM" ? "ethereum" : "polygon";
}

/** A positive wei amount as the Shop prints it; zero, garbage or nothing is no price. */
export function manaPrice(wei: string | null | undefined): string | undefined {
  const amount = parseManaWei(wei ?? undefined);
  return amount !== null && amount > 0n ? formatManaPrice(amount) : undefined;
}

/** The `data.wearable` or `data.emote` block, whichever the row carries. */
function traitsBlock(data: Record<string, unknown> | null | undefined): Record<string, unknown> | undefined {
  const block = data?.wearable ?? data?.emote;
  return block && typeof block === "object" ? (block as Record<string, unknown>) : undefined;
}

function traitsOf(block: Record<string, unknown> | undefined): CollectibleTraits {
  const shapes = Array.isArray(block?.bodyShapes) ? block.bodyShapes : [];
  return {
    category: typeof block?.category === "string" && block.category ? block.category : undefined,
    bodyShapes: shapes.filter((shape): shape is string => typeof shape === "string"),
    isSmart: block?.isSmart === true,
  };
}

function toCreation(raw: unknown): ProfileCreation | null {
  const row = CreationRowSchema.safeParse(raw);
  if (!row.success) {
    warnInvalid("ProfileCreation", row.error.issues);
    return null;
  }
  const item = row.data;
  return {
    id: item.id,
    contractAddress: item.contractAddress.toLowerCase(),
    itemId: item.itemId,
    // A blank name would leave the card link and its actions without an accessible name.
    name: item.name.trim() || "Untitled",
    thumbnail: httpsUrl(item.thumbnail),
    rarity: rarityOf(item.rarity),
    network: networkOf(item.network),
    // The catalogue keeps `price` after a delisting or a sell-out, so it counts only while the
    // creator still sells; otherwise the item trades at its cheapest listing, if any.
    price: (item.isOnSale && item.available > 0 ? manaPrice(item.price) : undefined) ?? manaPrice(item.minListingPrice),
    traits: traitsOf(traitsBlock(item.data)),
  };
}

/**
 * A token's title, as the marketplace names one without a name: a NAME by its label, a parcel by
 * its coordinates, an estate as "Estate", anything else "Untitled".
 */
function assetName(category: string, name: string | null | undefined, data: Record<string, unknown> | null | undefined): string {
  const given = (category === "ens" ? (name ?? "").replace(/\.dcl\.eth$/i, "") : (name ?? "")).trim();
  if (given) return given;
  const parcel = data?.parcel as { x?: unknown; y?: unknown } | undefined;
  if (category === "parcel" && parcel && parcel.x != null && parcel.y != null) return `Parcel ${String(parcel.x)},${String(parcel.y)}`;
  if (category === "estate") return "Estate";
  return "Untitled";
}

function toAsset(raw: unknown): ProfileAsset | null {
  const row = AssetRowSchema.safeParse(raw);
  if (!row.success) {
    warnInvalid("ProfileAsset", row.error.issues);
    return null;
  }
  const { nft, order } = row.data;
  const block = traitsBlock(nft.data);
  const category = nft.category.toLowerCase();
  return {
    id: nft.id,
    contractAddress: nft.contractAddress.toLowerCase(),
    tokenId: nft.tokenId,
    category,
    name: assetName(category, nft.name, nft.data),
    image: httpsUrl(nft.image),
    rarity: rarityOf(block?.rarity),
    network: networkOf(nft.network),
    price: manaPrice(order?.price),
    traits: traitsOf(block),
  };
}

/** A creator's published items of one category, newest first, from the catalogue. */
export async function fetchProfileCreations(
  creator: string,
  params: { category: CreationsCategory; first: number; skip?: number },
  opts: GetOptions = {},
): Promise<CollectiblePage<ProfileCreation>> {
  const path = "/marketplace-api/v2/catalog";
  const page = requirePage(
    await getJSON<unknown>(path, {
      ...opts,
      query: {
        creator: creator.trim().toLowerCase(),
        category: params.category,
        first: params.first,
        skip: params.skip ?? 0,
        sortBy: "newest",
      },
    }),
    path,
  );
  return { items: collect(page.data, toCreation), received: page.data.length, total: page.total };
}

/** The tokens a wallet owns, newest first, optionally of one category. */
export async function fetchProfileAssets(
  owner: string,
  params: { category?: AssetCategory; first: number; skip?: number },
  opts: GetOptions = {},
): Promise<CollectiblePage<ProfileAsset>> {
  const path = "/marketplace-api/v1/nfts";
  const page = requirePage(
    await getJSON<unknown>(path, {
      ...opts,
      query: {
        owner: owner.trim().toLowerCase(),
        category: params.category,
        first: params.first,
        skip: params.skip ?? 0,
        sortBy: "newest",
      },
    }),
    path,
  );
  return { items: collect(page.data, toAsset), received: page.data.length, total: page.total };
}

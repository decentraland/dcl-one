import { z } from "zod";

import { formatManaPrice, parseManaWei } from "@shop/model/money";

import { CatalystError, getJSON, type GetOptions } from "../client";
import { withDeadline } from "../../request-deadline";
import { tryQuoteCreditItems } from "./credit-quotes";
import { cardPayment, type CardPayment, type CardSaleFacts } from "./index";
import { dataTotalOf } from "../envelope";
import { CatalogItemSchema, NftSchema, OrderSchema } from "../generated-schemas/market";
import { warnInvalid } from "../warn";

/** What a profile's Creations tab filters by: the catalogue's top-level category. */
export type CreationsCategory = "wearable" | "emote";

/** What a profile's Assets tab filters by; no category is every asset. */
export type AssetCategory = "wearable" | "emote" | "ens" | "parcel" | "estate";

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
  /**
   * What the card may show, by the Shop's own card rule (`cardPayment`): a credit-eligible primary
   * sale is priced in credits (or says the quote is unavailable, never a MANA fallback); anything
   * else is MANA, or nothing when it is not for sale.
   */
  payment: CardPayment;
  traits: CollectibleTraits;
};

/** How long a page waits for its credit prices, as the Shop's cards do, before showing them unavailable. */
export const CREATIONS_QUOTES_TIMEOUT_MS = 1_000;

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
  chainId: true,
  category: true,
}).extend({
  data: z.record(z.string(), z.unknown()).nullish(),
  // Optional, like every field the card does not show: an unknown supply is not sold out.
  available: z.number().nullish().transform((v) => v ?? null),
  // Only the worn items' order reads it.
  urn: z.string().nullish(),
});

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

type ParsedCreation = { card: Omit<ProfileCreation, "payment">; sale: CardSaleFacts; urn?: string };

function toCreation(raw: unknown): ParsedCreation | null {
  const row = CreationRowSchema.safeParse(raw);
  if (!row.success) {
    warnInvalid("ProfileCreation", row.error.issues);
    return null;
  }
  const item = row.data;
  const card = {
    id: item.id,
    contractAddress: item.contractAddress.toLowerCase(),
    itemId: item.itemId,
    // A blank name would leave the card link and its actions without an accessible name.
    name: item.name.trim() || "Untitled",
    thumbnail: httpsUrl(item.thumbnail),
    rarity: rarityOf(item.rarity),
    network: networkOf(item.network),
    traits: traitsOf(traitsBlock(item.data)),
  };
  return { card, sale: item, urn: item.urn ?? undefined };
}

/** Each row's credit price, quoted as the Shop's cards are; a failed or slow quote reads as unavailable. */
async function creditPrices(rows: ParsedCreation[], signal?: AbortSignal): Promise<(string | null)[]> {
  const refs = rows.map(({ card }) => ({ itemId: card.itemId, collection: card.contractAddress }));
  try {
    return await withDeadline((deadline) => tryQuoteCreditItems(refs, { signal: deadline }), CREATIONS_QUOTES_TIMEOUT_MS, signal);
  } catch {
    // A visitor who left aborts the read; anything else (the deadline) only loses the quotes.
    signal?.throwIfAborted();
    return rows.map(() => null);
  }
}

/**
 * A token's title, as the marketplace names one without a name: a NAME by its label (its
 * subdomain when the name is empty), a parcel by its coordinates, an estate as "Estate", anything
 * else "Untitled". The catalogue names an unnamed parcel or estate by its capitalized category
 * ("Parcel", "Estate"), so for those two that name counts as none.
 */
function coordinate(value: unknown): string | null {
  const text = typeof value === "number" || typeof value === "string" ? String(value).trim() : "";
  return text ? text : null;
}

function assetName(category: string, name: string | null | undefined, data: Record<string, unknown> | null | undefined): string {
  if (category === "ens") {
    const subdomain = (data?.ens as { subdomain?: unknown } | undefined)?.subdomain;
    // The catalyst names an unnamed token by its capitalized category ("Ens"); a NAME's own name ends in .dcl.eth.
    const given = name === "Ens" ? "" : (name ?? "");
    const label = given.replace(/\.dcl\.eth$/i, "").trim() || (typeof subdomain === "string" ? subdomain.trim() : "");
    return label || "Untitled";
  }
  const given = (name ?? "").trim();
  const placeholder = (category === "parcel" || category === "estate") && given.toLowerCase() === category;
  if (given && !placeholder) return given;
  const parcel = data?.parcel as { x?: unknown; y?: unknown } | undefined;
  // The catalyst sends a missing coordinate as "", not null.
  const x = coordinate(parcel?.x);
  const y = coordinate(parcel?.y);
  if (category === "parcel" && x !== null && y !== null) return `Parcel ${x},${y}`;
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

/** The parsed rows as cards, each with the payment the Shop's card rule gives it. */
async function pricedCreations(rows: ParsedCreation[], signal?: AbortSignal): Promise<ProfileCreation[]> {
  const credits = rows.length > 0 ? await creditPrices(rows, signal) : [];
  return rows.map(({ card, sale }, index) => ({ ...card, payment: cardPayment(sale, credits[index] ?? null) }));
}

/**
 * An item URN in one spelling: lowercase, with Ethereum named `ethereum` (the catalogue also takes
 * `mainnet` and answers `ethereum`). The URN is the one key a worn item and its catalogue row share:
 * a `collections-v1` URN names its collection and item by name, where the row has a contract
 * address and a number.
 */
function urnKey(urn: string): string {
  return urn.toLowerCase().replace(":mainnet:", ":ethereum:");
}

/**
 * The catalogue's cards for the collectibles an avatar wears, in the order worn. One read per
 * network, each naming its item URNs; an item the catalogue does not know is left out. When one
 * network's read fails, the other network's cards are still returned; the read fails only when no
 * network it asked answered.
 */
export async function fetchWornCollectibles(urns: readonly string[], opts: GetOptions = {}): Promise<ProfileCreation[]> {
  const path = "/marketplace-api/v2/catalog";
  const byNetwork = (network: "matic" | "ethereum") => urns.filter((urn) => urnKey(urn).split(":")[2] === network);
  const read = async (network: "MATIC" | "ETHEREUM", wanted: string[]): Promise<ParsedCreation[]> => {
    const query = new URLSearchParams({ network, first: String(wanted.length) });
    for (const urn of wanted) query.append("urn", urn);
    return collect(requirePage(await getJSON<unknown>(`${path}?${query}`, opts), path).data, toCreation);
  };
  const reads: Promise<ParsedCreation[]>[] = [];
  const matic = byNetwork("matic");
  const ethereum = byNetwork("ethereum");
  if (matic.length > 0) reads.push(read("MATIC", matic));
  if (ethereum.length > 0) reads.push(read("ETHEREUM", ethereum));
  const results = await Promise.allSettled(reads);
  // A visitor who left aborts every read; that is no partial answer.
  opts.signal?.throwIfAborted();
  const failed = results.find((result) => result.status === "rejected");
  if (failed && results.every((result) => result.status === "rejected")) throw failed.reason;
  const order = new Map(urns.map((urn, index) => [urnKey(urn), index]));
  const position = (row: ParsedCreation) => (row.urn ? order.get(urnKey(row.urn)) : undefined) ?? urns.length;
  const rows = results
    .flatMap((result) => (result.status === "fulfilled" ? result.value : []))
    .sort((a, b) => position(a) - position(b));
  return pricedCreations(rows, opts.signal);
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
  return {
    items: await pricedCreations(collect(page.data, toCreation), opts.signal),
    received: page.data.length,
    total: page.total,
  };
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

// from: shop@2d3bf62:app/src/lib/outfits.ts, app/src/lib/ownership.ts (isOwnListing)

import { isShopApiError } from "../../api/client";
import { ENDPOINTS } from "../../api/endpoints";
import { identitySigner } from "../../api/signer";
import { shopClient } from "../../api/shop";
import type { AuthIdentity } from "../../../data/auth/identity";
import type { CatalogItem } from "../lib/catalogItem";
import { slotOf } from "../lib/outfit";

export type OutfitBodyShape = "male" | "female" | "unisex";
export type OutfitItemRef = { contractAddress: string; itemId: string };

export type Outfit = {
  id: string;
  name: string;
  thumbnailHash: string;
  items: OutfitItemRef[];
  bodyShape: OutfitBodyShape;
  gradientFrom: string;
  gradientTo: string;
  published: boolean;
  authorAddress?: string;
  createdAt?: number;
  updatedAt?: number;
};

export type OutfitDraft = Omit<Outfit, "authorAddress" | "createdAt" | "updatedAt">;

export const MIN_OUTFIT_ITEMS = 1;
export const MAX_OUTFIT_ITEMS = 20;
export const MAX_THUMBNAIL_BYTES = 1024 * 1024;
export const DEFAULT_OUTFIT_GRADIENT = { from: "#c640cd", to: "#691fa9" } as const;

export function outfitItemKey(ref: OutfitItemRef): string {
  return `${ref.contractAddress.toLowerCase()}-${ref.itemId}`;
}

export function parseOutfit(body: unknown): Outfit | null {
  const raw = (body && typeof body === "object" && "outfit" in body ? (body as { outfit: unknown }).outfit : body) as Partial<Outfit> & { author?: string } | null;
  if (!raw || typeof raw !== "object" || typeof raw.id !== "string" || typeof raw.name !== "string" || !Array.isArray(raw.items)) return null;
  return {
    id: raw.id,
    name: raw.name,
    thumbnailHash: typeof raw.thumbnailHash === "string" ? raw.thumbnailHash : "",
    items: raw.items.filter((i) => i && typeof i.contractAddress === "string" && typeof i.itemId === "string").map((i) => ({ contractAddress: i.contractAddress, itemId: i.itemId })),
    bodyShape: raw.bodyShape === "male" || raw.bodyShape === "female" ? raw.bodyShape : "unisex",
    gradientFrom: typeof raw.gradientFrom === "string" ? raw.gradientFrom : "",
    gradientTo: typeof raw.gradientTo === "string" ? raw.gradientTo : "",
    published: raw.published !== false,
    authorAddress: raw.authorAddress ?? raw.author,
    createdAt: raw.createdAt,
    updatedAt: raw.updatedAt,
  };
}

export async function fetchOutfit(id: string, signal?: AbortSignal): Promise<Outfit | null> {
  try {
    return parseOutfit(await shopClient().outfits.get(id, { signal }));
  } catch (e) {
    if (isShopApiError(e) && e.status === 404) return null;
    throw e;
  }
}

export async function fetchAllOutfits(signal?: AbortSignal): Promise<Outfit[]> {
  const body = await shopClient().outfits.all<{ outfits?: unknown[] }>({ signal });
  return (body?.outfits ?? []).map(parseOutfit).filter((o): o is Outfit => !!o);
}

export async function saveOutfit(draft: OutfitDraft, mode: "create" | "update"): Promise<Outfit> {
  const body = mode === "create" ? await shopClient().outfits.create(draft) : await shopClient().outfits.update(draft.id, draft);
  const saved = parseOutfit(body);
  if (!saved) throw new OutfitsError("generic");
  return saved;
}

export async function deleteOutfit(id: string): Promise<void> {
  await shopClient().outfits.remove(id);
}

export async function uploadThumbnail(image: Blob, identity: AuthIdentity | null): Promise<string> {
  const signer = identitySigner(identity);
  if (!signer) throw new OutfitsError("not_allowed");
  const api = shopClient().api;
  const url = api.url(ENDPOINTS.uploadOutfitThumbnail);
  const path = new URL(url, "http://shop.invalid").pathname;
  const headers = await signer.sign("POST", path, {});
  const res = await fetch(url, { method: "POST", headers: { ...headers, "content-type": image.type || "application/octet-stream" }, body: image });
  const body = (await res.json().catch(() => null)) as { hash?: string; error?: string } | null;
  if (!res.ok || !body?.hash) throw new OutfitsError(body?.error ?? `http_${res.status}`);
  return body.hash;
}

export class OutfitsError extends Error {
  code: string;
  constructor(code: string) {
    super(`outfits: ${code}`);
    this.code = code;
  }
}

export const OUTFIT_ERRORS: Record<string, string> = {
  generic: "Something went wrong. Please try again.",
  not_allowed: "Your account can't manage outfits.",
  not_publishable: "Add a name, a thumbnail and 1–20 items before publishing.",
  invalid_items: "One of the items is duplicated or invalid.",
  too_large: "That image is too big — keep it under 1 MB.",
  unsupported_type: "Use a PNG, JPEG or WebP image.",
  not_found: "This outfit no longer exists.",
};

export function outfitErrorMessage(e: unknown): string {
  const code = e instanceof OutfitsError ? e.code : isShopApiError(e) ? e.code : "";
  return (code && OUTFIT_ERRORS[code]) || OUTFIT_ERRORS.generic!;
}

const THUMBNAIL_HASH = /^[0-9a-f]{64}$/;

export function thumbnailUrl(hash: string): string | null {
  return THUMBNAIL_HASH.test(hash) ? shopClient().outfits.thumbnailUrl(hash) : null;
}

const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;

export function isHexColor(value: string): boolean {
  return HEX_COLOR.test(value);
}

function outfitStops(outfit: Pick<Outfit, "gradientFrom" | "gradientTo">): { from: string; to: string } {
  return {
    from: isHexColor(outfit.gradientFrom) ? outfit.gradientFrom : DEFAULT_OUTFIT_GRADIENT.from,
    to: isHexColor(outfit.gradientTo) ? outfit.gradientTo : DEFAULT_OUTFIT_GRADIENT.to,
  };
}

export function outfitGradient(outfit: Pick<Outfit, "gradientFrom" | "gradientTo">): string {
  const { from, to } = outfitStops(outfit);
  return `radial-gradient(65.83% 65.83% at 50% 50%, ${to} 0%, ${from} 100%)`;
}

export function outfitRadialGradient(outfit: Pick<Outfit, "gradientFrom" | "gradientTo">): string {
  const { from, to } = outfitStops(outfit);
  return `radial-gradient(circle, ${to} 0%, ${from} 100%)`;
}

export type OutfitItemState = "purchasable" | "unavailable" | "own_listing" | "in_cart";

export function isListingUnavailable(item: Pick<CatalogItem, "priceCredits" | "tokenId" | "available" | "manaWei" | "onSale">): boolean {
  if (item.onSale === false) return true;
  if (item.priceCredits <= 0 && !item.manaWei) return true;
  return !item.tokenId && typeof item.available === "number" && item.available <= 0;
}

export function isOwnListing(item: Pick<CatalogItem, "creator" | "tokenId">, address?: string | null): boolean {
  if (!address || !item.creator) return false;
  return !item.tokenId && item.creator.toLowerCase() === address.toLowerCase();
}

export function classifyOutfitItem(item: CatalogItem, options: { address?: string | null; inCart: (item: CatalogItem) => boolean }): OutfitItemState {
  if (isListingUnavailable(item)) return "unavailable";
  if (isOwnListing(item, options.address)) return "own_listing";
  if (options.inCart(item)) return "in_cart";
  return "purchasable";
}

export function toggleOutfitItem(refs: OutfitItemRef[], picked: CatalogItem, resolved: ReadonlyMap<string, CatalogItem>): OutfitItemRef[] {
  if (!picked.itemId) return refs;
  const pickedRef: OutfitItemRef = { contractAddress: picked.contractAddress.toLowerCase(), itemId: picked.itemId };
  const pickedKey = outfitItemKey(pickedRef);
  if (refs.some((ref) => outfitItemKey(ref) === pickedKey)) return refs.filter((ref) => outfitItemKey(ref) !== pickedKey);
  const slot = slotOf(picked);
  const kept =
    slot == null
      ? refs
      : refs.filter((ref) => {
          const item = resolved.get(outfitItemKey(ref));
          return !item || slotOf(item) !== slot;
        });
  return [...kept, pickedRef];
}

export type OutfitImport = {
  items: OutfitItemRef[];
  bodyShape?: Exclude<OutfitBodyShape, "unisex">;
  colors?: { skin?: string; hair?: string; eyes?: string };
};

const IMPORT_ITEM_URN = /^urn:decentraland:(?:matic|amoy):collections-v2:(0x[0-9a-f]{40}):(\d+)$/i;

export function parseOutfitImport(raw: string): OutfitImport | null {
  const query = raw.includes("?") ? raw.slice(raw.indexOf("?") + 1) : raw;
  const params = new URLSearchParams(query);
  const seen = new Set<string>();
  const items: OutfitItemRef[] = [];
  for (const urn of [...params.getAll("urn"), ...params.getAll("emote")]) {
    const match = IMPORT_ITEM_URN.exec(urn.trim());
    if (!match) continue;
    const ref = { contractAddress: match[1]!.toLowerCase(), itemId: match[2]! };
    const key = outfitItemKey(ref);
    if (seen.has(key)) continue;
    seen.add(key);
    items.push(ref);
  }
  if (items.length === 0) return null;
  const shapeUrn = params.get("bodyShape") ?? "";
  const bodyShape = /basefemale$/i.test(shapeUrn) ? "female" : /basemale$/i.test(shapeUrn) ? "male" : undefined;
  const color = (name: string): string | undefined => {
    const value = (params.get(name) ?? "").trim().replace(/^#/, "");
    return /^[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : undefined;
  };
  const colors = { skin: color("skinColor"), hair: color("hairColor"), eyes: color("eyeColor") };
  return { items: items.slice(0, MAX_OUTFIT_ITEMS), bodyShape, colors: colors.skin || colors.hair || colors.eyes ? colors : undefined };
}

// from: shop@94584a5c:app/src/lib/outfit.ts
// Fitting-room logic, pure: one wearable per slot (its category), emotes are never worn, and the
// preview's URN order puts the playing emote first (both renderers then pick the same one).

import { isCompatible, type BodyShapeUrn } from "../../detail/lib/bodyShape";
import { itemUrn } from "../../detail/lib/urn";
import type { CatalogItem } from "./catalogItem";

export function isWearable(item: Pick<CatalogItem, "category">): boolean {
  return item.category !== "emote";
}

export function slotOf(item: Pick<CatalogItem, "category" | "wearableCategory" | "id">): string | null {
  if (!isWearable(item)) return null;
  return item.wearableCategory ?? `unknown:${item.id}`;
}

export type SlotRegion = "head" | "upper" | "lower" | "feet" | "hands" | "item";

const HEAD_CATEGORIES = new Set(["hat", "helmet", "mask", "tiara", "top_head", "hair", "facial_hair", "eyewear", "earring", "eyes", "eyebrows", "mouth"]);

export function slotRegion(item: Pick<CatalogItem, "category" | "wearableCategory">): SlotRegion {
  if (!isWearable(item)) return "item";
  const c = item.wearableCategory;
  if (!c) return "item";
  if (HEAD_CATEGORIES.has(c)) return "head";
  if (c === "upper_body") return "upper";
  if (c === "lower_body") return "lower";
  if (c === "feet") return "feet";
  if (c === "hands" || c === "hands_wear") return "hands";
  return "item";
}

/** A fresh fitting room: one wearable per slot (first wins), none the target body cannot wear. */
export function defaultWorn(items: CatalogItem[], target: BodyShapeUrn | null = null): Set<string> {
  const worn = new Set<string>();
  const taken = new Set<string>();
  for (const item of items) {
    const slot = slotOf(item);
    if (slot == null || taken.has(slot) || !isCompatible(item, target)) continue;
    taken.add(slot);
    worn.add(item.id);
  }
  return worn;
}

/** Equipping swaps out whatever holds the same slot; emotes cannot be equipped. */
export function toggleWorn(worn: Set<string>, item: CatalogItem, all: CatalogItem[]): Set<string> {
  if (!isWearable(item)) return worn;
  const next = new Set(worn);
  if (next.has(item.id)) {
    next.delete(item.id);
    return next;
  }
  const slot = slotOf(item);
  if (slot != null) for (const other of all) if (other.id !== item.id && next.has(other.id) && slotOf(other) === slot) next.delete(other.id);
  next.add(item.id);
  return next;
}

export function conflictingIds(items: CatalogItem[]): Set<string> {
  const bySlot = new Map<string, string[]>();
  for (const item of items) {
    const slot = slotOf(item);
    if (slot == null) continue;
    bySlot.set(slot, [...(bySlot.get(slot) ?? []), item.id]);
  }
  const conflicts = new Set<string>();
  for (const ids of bySlot.values()) if (ids.length > 1) ids.forEach((id) => conflicts.add(id));
  return conflicts;
}

export function playingEmote(items: CatalogItem[]): CatalogItem | undefined {
  return items.find((item) => item.category === "emote");
}

export function outfitPreviewUrns(items: CatalogItem[]): string[] {
  const playing = playingEmote(items);
  const urns: string[] = [];
  const playingUrn = playing ? (playing.urn ?? itemUrn(playing)) : null;
  if (playingUrn) urns.push(playingUrn);
  for (const item of items) {
    if (!isWearable(item)) continue;
    const urn = item.urn ?? itemUrn(item);
    if (urn) urns.push(urn);
  }
  return urns;
}

export function wornUrns(items: CatalogItem[], worn: Set<string>, target: BodyShapeUrn | null = null): string[] {
  const urns: string[] = [];
  for (const item of items) {
    if (!worn.has(item.id) || !isWearable(item) || !isCompatible(item, target)) continue;
    const urn = item.urn ?? itemUrn(item);
    if (urn) urns.push(urn);
  }
  return urns;
}

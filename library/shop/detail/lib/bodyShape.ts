// from: shop@94584a5c:app/src/lib/bodyShape.ts
// Body-shape compatibility. A wearable that does not support the avatar's body renders invisible, so
// the preview uses a mannequin of a shape the item supports instead. Emotes and unisex items fit both.

import type { CatalogItem } from "../../discovery/lib/catalogItem";
import { t } from "../../discovery/intl";

/** @dcl/schemas BodyShape.MALE / FEMALE. */
export const BASE_MALE = "urn:decentraland:off-chain:base-avatars:BaseMale";
export const BASE_FEMALE = "urn:decentraland:off-chain:base-avatars:BaseFemale";
export type BodyShapeUrn = typeof BASE_MALE | typeof BASE_FEMALE;

type ItemLike = Pick<CatalogItem, "gender" | "category">;

export function itemShapes(item: ItemLike): BodyShapeUrn[] {
  if (item.category === "emote") return [BASE_MALE, BASE_FEMALE];
  switch (item.gender) {
    case "male":
      return [BASE_MALE];
    case "female":
      return [BASE_FEMALE];
    default:
      return [BASE_MALE, BASE_FEMALE];
  }
}

/** The avatar's body shape from its profile (`avatar.bodyShape`), or null when unknown. */
export function avatarShape(profile?: { avatar?: { bodyShape?: string } } | null): BodyShapeUrn | null {
  const urn = profile?.avatar?.bodyShape;
  if (!urn) return null;
  return urn.includes("BaseFemale") ? BASE_FEMALE : BASE_MALE;
}

export function isCompatible(item: ItemLike, shape: BodyShapeUrn | null): boolean {
  return shape == null || itemShapes(item).includes(shape);
}

export function shapeLabel(shape: BodyShapeUrn): string {
  return shape === BASE_FEMALE ? t("bodyShape.female") : t("bodyShape.male");
}

/** The one shape a whole set must be worn on, or null when the set leaves the choice open (or conflicts). */
export function requiredShape(items: ItemLike[]): BodyShapeUrn | null {
  const shapes = items.reduce<BodyShapeUrn[]>((acc, item) => acc.filter((shape) => itemShapes(item).includes(shape)), [BASE_MALE, BASE_FEMALE]);
  return shapes.length === 1 ? shapes[0]! : null;
}

/** The majority shape among the gendered items (ties male); null when none are gendered. */
export function dominantShape(items: ItemLike[]): BodyShapeUrn | null {
  let male = 0;
  let female = 0;
  for (const it of items) {
    if (it.category === "emote") continue;
    if (it.gender === "male") male++;
    else if (it.gender === "female") female++;
  }
  if (male === 0 && female === 0) return null;
  return female > male ? BASE_FEMALE : BASE_MALE;
}

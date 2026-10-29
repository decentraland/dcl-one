// from: shop@94584a5c:app/src/lib/itemIcons.ts
import type { IconName } from "../components/Icon";
import type { CatalogItem } from "./catalogItem";

const CATEGORY_ICON: Record<string, IconName> = {
  head: "cat-head",
  hat: "cat-head",
  hair: "cat-head",
  facial_hair: "cat-head",
  eyes: "cat-head",
  eyebrows: "cat-head",
  mouth: "cat-head",
  mask: "cat-head",
  helmet: "cat-head",
  tiara: "cat-head",
  top_head: "cat-head",
  upper_body: "cat-upper",
  hands_wear: "cat-handwear",
  lower_body: "cat-lower",
  feet: "cat-feet",
  earring: "cat-accessories",
  eyewear: "cat-accessories",
  skin: "cat-skins",
  dance: "emote-dance",
  stunt: "emote-stunt",
  greetings: "emote-greetings",
  fun: "emote-fun",
  poses: "emote-poses",
  reactions: "emote-reactions",
  horror: "emote-horror",
};

/** The card's category chip icon, or null when the category has none. */
export function categoryIcon(item: Pick<CatalogItem, "category" | "wearableCategory">): IconName | null {
  const key = item.wearableCategory?.toLowerCase();
  if (key && CATEGORY_ICON[key]) return CATEGORY_ICON[key]!;
  if (item.category === "emote") return "emote-misc";
  return null;
}

export function genderIcon(gender: CatalogItem["gender"]): IconName | null {
  if (gender === "male") return "gender-male";
  if (gender === "female") return "gender-female";
  if (gender === "unisex") return "gender-unisex";
  return null;
}

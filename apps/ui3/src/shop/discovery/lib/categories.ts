// from: shop@94584a5c:app/src/lib/categories.ts
// Sidebar sub-category keys -> the on-chain categories the `wearableCategory` server filter takes, and
// the category tree CategoryFilter renders. Keys are filter state (and URL values): never rename one.

import type { IconName } from "../components/Icon";

export const SUBCAT_MAP: Record<string, string[]> = {
  Head: ["head", "hair", "facial_hair", "eyes", "eyebrows", "mouth"],
  "Facial Hair": ["facial_hair"],
  Hair: ["hair"],
  Eyes: ["eyes"],
  Eyebrows: ["eyebrows"],
  Mouth: ["mouth"],
  "Upper Body": ["upper_body"],
  Handwear: ["hands_wear"],
  "Lower Body": ["lower_body"],
  Feet: ["feet"],
  Accessories: ["earring", "eyewear", "hat", "helmet", "mask", "tiara", "top_head"],
  Earring: ["earring"],
  Eyewear: ["eyewear"],
  Hat: ["hat"],
  Helmet: ["helmet"],
  Mask: ["mask"],
  Tiara: ["tiara"],
  "Top Head": ["top_head"],
  Skins: ["skin"],
  Dance: ["dance"],
  Stunt: ["stunt"],
  Greetings: ["greetings"],
  Fun: ["fun"],
  Poses: ["poses"],
  Reactions: ["reactions"],
  Horror: ["horror"],
  Miscellaneous: ["miscellaneous"],
};

export type SubSub = { key: string; labelKey: string; icon: IconName };
export type Sub = { key: string; labelKey: string; icon: IconName; expandable?: boolean; subs?: SubSub[] };
export type Top = { key: string; labelKey: string; expandable?: boolean; subs?: Sub[] };

export const CATEGORIES: Top[] = [
  { key: "all", labelKey: "categories.shopAll" },
  {
    key: "wearable",
    labelKey: "categories.wearables",
    expandable: true,
    subs: [
      {
        key: "Head",
        labelKey: "categories.head",
        icon: "cat-head",
        expandable: true,
        subs: [
          { key: "Facial Hair", labelKey: "categories.facialHair", icon: "cat-facial-hair" },
          { key: "Hair", labelKey: "categories.hair", icon: "cat-hair" },
          { key: "Eyes", labelKey: "categories.eyes", icon: "cat-eyes" },
          { key: "Eyebrows", labelKey: "categories.eyebrows", icon: "cat-eyebrows" },
          { key: "Mouth", labelKey: "categories.mouth", icon: "cat-mouth" },
        ],
      },
      { key: "Upper Body", labelKey: "categories.upperBody", icon: "cat-upper" },
      { key: "Handwear", labelKey: "categories.handwear", icon: "cat-handwear" },
      { key: "Lower Body", labelKey: "categories.lowerBody", icon: "cat-lower" },
      { key: "Feet", labelKey: "categories.feet", icon: "cat-feet" },
      {
        key: "Accessories",
        labelKey: "categories.accessories",
        icon: "cat-accessories",
        expandable: true,
        subs: [
          { key: "Earring", labelKey: "categories.earring", icon: "cat-earring" },
          { key: "Eyewear", labelKey: "categories.eyewear", icon: "cat-eyewear" },
          { key: "Hat", labelKey: "categories.hat", icon: "cat-hat" },
          { key: "Helmet", labelKey: "categories.helmet", icon: "cat-helmet" },
          { key: "Mask", labelKey: "categories.mask", icon: "cat-mask" },
          { key: "Tiara", labelKey: "categories.tiara", icon: "cat-tiara" },
          { key: "Top Head", labelKey: "categories.topHead", icon: "cat-top-head" },
        ],
      },
      { key: "Skins", labelKey: "categories.skins", icon: "cat-skins" },
    ],
  },
  {
    key: "emote",
    labelKey: "categories.emotes",
    expandable: true,
    subs: [
      { key: "Dance", labelKey: "categories.dance", icon: "emote-dance" },
      { key: "Stunt", labelKey: "categories.stunt", icon: "emote-stunt" },
      { key: "Greetings", labelKey: "categories.greetings", icon: "emote-greetings" },
      { key: "Fun", labelKey: "categories.fun", icon: "emote-fun" },
      { key: "Poses", labelKey: "categories.poses", icon: "emote-poses" },
      { key: "Reactions", labelKey: "categories.reactions", icon: "emote-reactions" },
      { key: "Horror", labelKey: "categories.horror", icon: "emote-horror" },
      { key: "Miscellaneous", labelKey: "categories.miscellaneous", icon: "emote-misc" },
    ],
  },
  { key: "names", labelKey: "categories.names" },
];

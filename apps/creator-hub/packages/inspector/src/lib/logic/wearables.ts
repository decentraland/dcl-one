/**
 * Wearable body slots and the scene wearables an AvatarShape can list.
 *
 * Besides wearable URNs, `AvatarShape.wearables` accepts the path of a GLB file in the scene
 * content named `<name>_<category>.glb`, for example `cap_hat.glb`. The category in the name is
 * the body slot it takes.
 */

export type WearableCategory =
  | 'helmet'
  | 'hat'
  | 'mask'
  | 'eyewear'
  | 'earring'
  | 'tiara'
  | 'top_head'
  | 'hair'
  | 'eyes'
  | 'eyebrows'
  | 'mouth'
  | 'facial_hair'
  | 'upper_body'
  | 'hands_wear'
  | 'lower_body'
  | 'feet'
  | 'skin';

export type WearableCategoryGroup = 'head' | 'face' | 'body';

export type WearableCategoryInfo = {
  id: WearableCategory;
  label: string;
  group: WearableCategoryGroup;
};

/** Every slot, in the order the avatar editor shows them. */
export const WEARABLE_CATEGORIES: readonly WearableCategoryInfo[] = [
  { id: 'helmet', label: 'Helmet', group: 'head' },
  { id: 'hat', label: 'Hat', group: 'head' },
  { id: 'mask', label: 'Mask', group: 'head' },
  { id: 'eyewear', label: 'Eyewear', group: 'head' },
  { id: 'earring', label: 'Earring', group: 'head' },
  { id: 'tiara', label: 'Tiara', group: 'head' },
  { id: 'top_head', label: 'Top head', group: 'head' },
  { id: 'hair', label: 'Hair', group: 'head' },
  { id: 'eyes', label: 'Eyes', group: 'face' },
  { id: 'eyebrows', label: 'Eyebrows', group: 'face' },
  { id: 'mouth', label: 'Mouth', group: 'face' },
  { id: 'facial_hair', label: 'Facial hair', group: 'face' },
  { id: 'upper_body', label: 'Upper body', group: 'body' },
  { id: 'hands_wear', label: 'Hands', group: 'body' },
  { id: 'lower_body', label: 'Lower body', group: 'body' },
  { id: 'feet', label: 'Feet', group: 'body' },
  { id: 'skin', label: 'Skin', group: 'body' },
];

export const WEARABLE_CATEGORY_GROUPS: readonly { id: WearableCategoryGroup; label: string }[] = [
  { id: 'head', label: 'Head' },
  { id: 'face', label: 'Face' },
  { id: 'body', label: 'Body' },
];

const CATEGORY_IDS = new Set<string>(WEARABLE_CATEGORIES.map($ => $.id));

export function isWearableCategory(value: unknown): value is WearableCategory {
  return typeof value === 'string' && CATEGORY_IDS.has(value);
}

export function getCategoryLabel(category: WearableCategory): string {
  return WEARABLE_CATEGORIES.find($ => $.id === category)?.label ?? category;
}

// The leftmost `_<category>.glb` wins, so `beard_facial_hair.glb` is facial hair, not hair.
const SCENE_WEARABLE_PATTERN = new RegExp(
  `_(${WEARABLE_CATEGORIES.map($ => $.id).join('|')})\\.glb$`,
  'i',
);

/** The body slot of a scene wearable file, or undefined when the path is not one. */
export function getSceneWearableCategory(path: string): WearableCategory | undefined {
  const fileName = path.split('/').pop() ?? '';
  const match = SCENE_WEARABLE_PATTERN.exec(fileName);
  if (!match || match.index === 0) return undefined;
  return match[1].toLowerCase() as WearableCategory;
}

export function isSceneWearablePath(path: string): boolean {
  return getSceneWearableCategory(path) !== undefined;
}

/** `cap_hat.glb` → `cap`; the name part a creator gave the file. */
export function getSceneWearableName(path: string): string {
  const fileName = path.split('/').pop() ?? path;
  const category = getSceneWearableCategory(fileName);
  if (!category) return fileName;
  return fileName.slice(0, fileName.length - `_${category}.glb`.length);
}

export function isWearableUrn(value: string): boolean {
  return value.startsWith('urn:');
}

export const BODY_SHAPE_A = 'urn:decentraland:off-chain:base-avatars:BaseMale';
export const BODY_SHAPE_B = 'urn:decentraland:off-chain:base-avatars:BaseFemale';
/** The body shape an NPC gets when AvatarShape leaves it unset. */
export const DEFAULT_BODY_SHAPE = BODY_SHAPE_B;

const BASE = 'urn:decentraland:off-chain:base-avatars:';

/**
 * The base outfit of each body shape, slot by slot: the one `avatar_shape.proto` documents for
 * Body B, and the Bevy explorer's Body A list. A new NPC is dressed in it, because explorers do not
 * all fill an empty slot: the Bevy explorer fills only the eyes, eyebrows and mouth.
 */
export const BASE_OUTFITS: Record<string, Partial<Record<WearableCategory, string>>> = {
  [BODY_SHAPE_B]: {
    eyes: `${BASE}f_eyes_00`,
    eyebrows: `${BASE}f_eyebrows_00`,
    mouth: `${BASE}f_mouth_00`,
    hair: `${BASE}standard_hair`,
    upper_body: `${BASE}f_simple_yellow_tshirt`,
    lower_body: `${BASE}f_brown_trousers`,
    feet: `${BASE}bun_shoes`,
  },
  [BODY_SHAPE_A]: {
    eyes: `${BASE}eyes_00`,
    eyebrows: `${BASE}eyebrows_00`,
    mouth: `${BASE}mouth_00`,
    hair: `${BASE}standard_hair`,
    upper_body: `${BASE}simple_blue_tshirt`,
    lower_body: `${BASE}distressed_black_Jeans`,
    feet: `${BASE}citycomfortableshoes`,
  },
};

/** The slots an explorer fills by itself when the list leaves them empty. */
export const FACE_SLOTS: readonly WearableCategory[] = ['eyes', 'eyebrows', 'mouth'];

export function getBaseOutfit(bodyShape: string): Partial<Record<WearableCategory, string>> {
  return BASE_OUTFITS[bodyShape] ?? BASE_OUTFITS[DEFAULT_BODY_SHAPE];
}

/** The wearables of a body shape's base outfit, as an AvatarShape lists them. */
export function getBaseOutfitUrns(bodyShape: string): string[] {
  return Object.values(getBaseOutfit(bodyShape)) as string[];
}

/** The base-outfit slot of `urn` on either body shape, when it is one. */
export function getBaseOutfitSlot(urn: string): WearableCategory | undefined {
  for (const outfit of Object.values(BASE_OUTFITS)) {
    for (const [slot, item] of Object.entries(outfit)) {
      if (item.toLowerCase() === urn.toLowerCase()) return slot as WearableCategory;
    }
  }
  return undefined;
}

/**
 * The list for another body shape: each item of the old body's base outfit becomes the new body's
 * item for that slot, and everything else stays.
 */
export function swapBaseOutfit(wearables: readonly string[], toBodyShape: string): string[] {
  const outfit = getBaseOutfit(toBodyShape);
  return wearables.map(item => {
    const slot = getBaseOutfitSlot(item);
    return (slot && outfit[slot]) || item;
  });
}

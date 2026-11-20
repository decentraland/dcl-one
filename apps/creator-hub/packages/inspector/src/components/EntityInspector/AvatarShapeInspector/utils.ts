import type { PBAvatarShape } from '@dcl/ecs';

import type { WearableCategory } from '../../../lib/logic/wearables';
import {
  FACE_SLOTS,
  getBaseOutfit,
  getBaseOutfitSlot,
  getCategoryLabel,
  getSceneWearableCategory,
  getSceneWearableName,
  isWearableUrn,
} from '../../../lib/logic/wearables';
import { toHex } from '../../ui/ColorField/utils';
import type { WearableItem } from './catalog';

export type Color3 = NonNullable<PBAvatarShape['skinColor']>;

/** The colours an NPC gets when AvatarShape leaves one unset (`avatar_shape.proto`). */
export const DEFAULT_SKIN_COLOR: Color3 = { r: 0.6, g: 0.462, b: 0.356 };
export const DEFAULT_HAIR_COLOR: Color3 = { r: 0.283, g: 0.142, b: 0 };
export const DEFAULT_EYE_COLOR: Color3 = { r: 0.6, g: 0.462, b: 0.356 };

/** Whether an explorer fills `category` itself when the list leaves it empty. */
export function hasDefaultWearable(category: WearableCategory): boolean {
  return FACE_SLOTS.includes(category);
}

/**
 * What the avatar preview shows: the worn URNs, and the face an explorer adds to a slot the list
 * leaves empty. Scene files cannot go in the hosted preview.
 */
export function getPreviewUrns(rows: readonly WearableRow[], bodyShape: string): string[] {
  const worn = rows.filter($ => !$.replacedBy && isWearableUrn($.value));
  const filled = new Set(rows.filter($ => !$.replacedBy).map($ => $.category));
  const outfit = getBaseOutfit(bodyShape);
  const face = FACE_SLOTS.filter(slot => !filled.has(slot)).map(slot => outfit[slot]!);
  return [...worn.map($ => $.value), ...face];
}

export type WearableRow = {
  /** The item as it is in `AvatarShape.wearables`. */
  value: string;
  index: number;
  name: string;
  category?: WearableCategory;
  categoryLabel: string;
  source?: WearableItem['source'];
  thumbnail?: string;
  /** Name of a later item of the same category, which the avatar wears instead. */
  replacedBy?: string;
  /** Neither a URN nor a scene `.glb`; explorers skip it. */
  invalid: boolean;
};

/**
 * One row per item of `wearables`, in order. `resolved` holds what the catalogs know about the
 * URNs. Of two items with the same category the later one is worn, so the earlier one is marked.
 */
export function toWearableRows(
  wearables: readonly string[],
  resolved: ReadonlyMap<string, WearableItem>,
): WearableRow[] {
  const rows = wearables.map((value, index): WearableRow => {
    if (isWearableUrn(value)) {
      const item = resolved.get(value);
      const baseSlot = item ? undefined : getBaseOutfitSlot(value);
      return {
        value,
        index,
        name: item?.name ?? value.split(':').pop() ?? value,
        category: item?.category ?? baseSlot,
        categoryLabel: item
          ? getCategoryLabel(item.category)
          : baseSlot
            ? getCategoryLabel(baseSlot)
            : 'Wearable',
        source: item?.source ?? (baseSlot ? 'base' : undefined),
        thumbnail: item?.thumbnail,
        invalid: false,
      };
    }
    const category = getSceneWearableCategory(value);
    return {
      value,
      index,
      name: category ? getSceneWearableName(value) : value,
      category,
      categoryLabel: category
        ? getCategoryLabel(category)
        : 'Not a wearable: use a URN or a name_category.glb file',
      source: category ? 'scene' : undefined,
      invalid: !category,
    };
  });

  const wornByCategory = getWornBySlot(rows);
  return rows.map(row => {
    const worn = row.category ? wornByCategory.get(row.category) : undefined;
    return worn && worn !== row ? { ...row, replacedBy: worn.name } : row;
  });
}

/** The wearable each slot shows, after later items replace earlier ones. */
export function getWornBySlot(rows: readonly WearableRow[]): Map<WearableCategory, WearableRow> {
  const worn = new Map<WearableCategory, WearableRow>();
  for (const row of rows) {
    if (row.category) worn.set(row.category, row);
  }
  return worn;
}

/**
 * Puts `item` on the avatar: it takes the place of whatever the list had in its slot, so the list
 * keeps one item per slot instead of growing with items the avatar no longer wears.
 */
export function addWearable(
  wearables: readonly string[],
  item: Pick<WearableItem, 'id' | 'category'>,
  categoryOf: (value: string) => WearableCategory | undefined,
): string[] {
  const kept = wearables.filter($ => $ !== item.id && categoryOf($) !== item.category);
  return [...kept, item.id];
}

/** `models/jetpack_suit.glb` → `jetpack_suit_upper_body.glb`: the name a slot gives a scene file. */
export function getSlotFileName(path: string, category: WearableCategory): string {
  const fileName = path.split('/').pop() ?? path;
  const stem = fileName.replace(/\.glb$/i, '');
  return `${stem}_${category}.glb`;
}

export function removeWearableAt(wearables: readonly string[], index: number): string[] {
  return wearables.filter((_, i) => i !== index);
}

/** `#AABBCC` → `AABBCC`, the form the avatar preview takes. */
export function toPreviewColor(color: Color3 | undefined, fallback: Color3): string {
  return toHex(color ?? fallback).replace('#', '');
}

/**
 * The explorers' built-in animations, by the names `expressionTriggerId` takes (the documented
 * default animations; the names are case sensitive).
 */
export const BUILT_IN_EMOTES: readonly { value: string; label: string }[] = [
  { value: 'wave', label: 'Wave' },
  { value: 'fistpump', label: 'Fist pump' },
  { value: 'fistpump_short', label: 'Fist pump (short)' },
  { value: 'robot', label: 'Robot' },
  { value: 'raiseHand', label: 'Raise hand' },
  { value: 'clap', label: 'Clap' },
  { value: 'money', label: 'Money' },
  { value: 'kiss', label: 'Kiss' },
  { value: 'tik', label: 'Tik' },
  { value: 'hammer', label: 'Hammer' },
  { value: 'tektonik', label: 'Tektonik' },
  { value: 'dontsee', label: "Don't see" },
  { value: 'handsair', label: 'Hands in the air' },
  { value: 'handsintheair', label: 'Hands in the air (long)' },
  { value: 'shrug', label: 'Shrug' },
  { value: 'disco', label: 'Disco' },
  { value: 'dab', label: 'Dab' },
  { value: 'headexplode', label: 'Head explode' },
  { value: 'crafting', label: 'Crafting' },
  { value: 'Victory', label: 'Victory' },
  { value: 'Waving', label: 'Waving' },
  { value: 'sittingChair1', label: 'Sitting on a chair 1' },
  { value: 'sittingChair2', label: 'Sitting on a chair 2' },
  { value: 'sittingGround1', label: 'Sitting on the ground 1' },
  { value: 'sittingGround2', label: 'Sitting on the ground 2' },
  { value: 'buttonDown', label: 'Press a button down' },
  { value: 'buttonFront', label: 'Press a button in front' },
  { value: 'getHit', label: 'Get hit' },
  { value: 'knockOut', label: 'Knock out' },
  { value: 'lever', label: 'Pull a lever' },
  { value: 'openChest', label: 'Open a chest' },
  { value: 'openDoor', label: 'Open a door' },
  { value: 'punch', label: 'Punch' },
  { value: 'push', label: 'Push' },
  { value: 'swingWeaponOneHand', label: 'Swing a weapon, one hand' },
  { value: 'swingWeaponTwoHands', label: 'Swing a weapon, two hands' },
  { value: 'throw', label: 'Throw' },
];

export const NO_EMOTE = '';

/**
 * `expressionTriggerTimestamp` is a lamport counter, not a time: the first play is 0 and each
 * replay adds 1, which is how an explorer tells a new play from one it already ran.
 */
export function nextExpressionTimestamp(current: number | undefined): number {
  return current === undefined ? 0 : current + 1;
}

/**
 * What explorers show after an avatar's name: `#` and the last four characters of its `id`, or
 * `#0000` when the id is shorter (the Bevy explorer's nametag).
 */
export function getNametagSuffix(id: string | undefined): string {
  const chars = Array.from(id ?? '');
  return chars.length >= 4 ? `#${chars.slice(-4).join('')}` : '#0000';
}

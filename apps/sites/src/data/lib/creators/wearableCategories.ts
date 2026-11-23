// Category choices for wearables, ported from the legacy builder's modules/item/utils: which slots a
// file can occupy and which body parts / slots it may hide.
import { BodyPartCategory, EmoteCategory, WearableCategory } from '@dcl/schemas'
import { isModelFile } from './itemFiles'

/** Texture-only slots: a PNG wearable can only be one of these. */
export const IMAGE_WEARABLE_CATEGORIES: string[] = [
  WearableCategory.EYEBROWS,
  WearableCategory.EYES,
  WearableCategory.MOUTH
]

/** Hides that still mean something for a wearable of this category: a skin covers everything, and no wearable hides itself. */
export function hidesForCategory(category: string | null | undefined, hides: string[]): string[] {
  if (category === (WearableCategory.SKIN as string)) return []
  return hides.filter(hide => hide !== category)
}

/** Everything a skin already covers, so it can't also be listed as hidden by one. */
export const SKIN_HIDDEN_CATEGORIES: string[] = [
  BodyPartCategory.HEAD,
  BodyPartCategory.HANDS,
  WearableCategory.HAIR,
  WearableCategory.FACIAL_HAIR,
  WearableCategory.MOUTH,
  WearableCategory.EYEBROWS,
  WearableCategory.EYES,
  WearableCategory.UPPER_BODY,
  WearableCategory.LOWER_BODY,
  WearableCategory.FEET
]

export function isImageWearableContents(contents: Record<string, unknown>): boolean {
  return !Object.keys(contents).some(isModelFile)
}

function getCategoriesForContents(contents: Record<string, unknown>): string[] {
  if (isImageWearableContents(contents)) return IMAGE_WEARABLE_CATEGORIES
  return (WearableCategory.schema.enum as string[]).filter(category => !IMAGE_WEARABLE_CATEGORIES.includes(category))
}

/** The slots a wearable with these files may be assigned to. */
export function getWearableCategoryOptions(contents: Record<string, unknown>): string[] {
  return getCategoriesForContents(contents).filter(category => category !== (WearableCategory.BODY_SHAPE as string))
}

export function getEmoteCategoryOptions(): string[] {
  return EmoteCategory.schema.enum as string[]
}

/** Base body parts a model wearable may hide; texture wearables hide none. */
export function getHideableBodyPartCategories(contents: Record<string, unknown>): string[] {
  if (isImageWearableContents(contents)) return []
  return BodyPartCategory.schema.enum as string[]
}

/**
 * Wearable slots this wearable may hide. `body_shape` and the wearable's own category are never offered
 * anew (legacy builder#2068; renderers ignore a self-hide) but stay selectable while already hidden; a skin
 * can't hide what it already covers.
 */
export function getHideableWearableCategories(
  contents: Record<string, unknown>,
  category: string | null | undefined,
  currentHides: string[] = []
): string[] {
  let hideable = getCategoriesForContents(contents)
  if (category === (WearableCategory.SKIN as string)) {
    hideable = hideable.filter(
      candidate => !SKIN_HIDDEN_CATEGORIES.includes(candidate) && candidate !== (WearableCategory.SKIN as string)
    )
  }
  return hideable.filter(
    candidate =>
      (candidate !== (WearableCategory.BODY_SHAPE as string) && candidate !== category) ||
      currentHides.includes(candidate)
  )
}

// The avatar look the Backpack edits, and the setAvatar deploy built from it. Shared with the bridge
// scene (like protocol.ts); pure, so react-web's vitest covers it.

import type { Color3, SetAvatarData } from './generated/index'

export interface AvatarLook {
  bodyShape: string
  eyes: Color3 | null
  hair: Color3 | null
  skin: Color3 | null
  wearables: string[]
  /** The emote wheel's slots as the profile has them ('' = empty). */
  emotes: string[]
  forceRender: string[]
}

const sameColor = (a: Color3 | null, b: Color3 | null): boolean =>
  a === b || (a != null && b != null && a.r === b.r && a.g === b.g && a.b === b.b)

const sameItems = (a: string[], b: string[]): boolean =>
  a.length === b.length && a.every((item, i) => item === b[i])

/** An outstanding Backpack close can still land or fail. Carry the player's changed fields relative
 *  to each possible prior look. Body and its fitting equipment deploy together. */
export function lookDeploy(look: AvatarLook, ...prior: AvatarLook[]): SetAvatarData {
  const bodyShapeUrn = prior.some((from) => look.bodyShape !== from.bodyShape) ? look.bodyShape : ''
  const eyesColor = prior.some((from) => !sameColor(look.eyes, from.eyes)) ? look.eyes : null
  const hairColor = prior.some((from) => !sameColor(look.hair, from.hair)) ? look.hair : null
  const skinColor = prior.some((from) => !sameColor(look.skin, from.skin)) ? look.skin : null
  const patch: SetAvatarData = {}
  if (bodyShapeUrn || eyesColor || hairColor || skinColor) {
    patch.base = { name: '', bodyShapeUrn, eyesColor, hairColor, skinColor, talking: null }
  }
  if (bodyShapeUrn || prior.some((from) => !sameItems(look.wearables, from.wearables) ||
    !sameItems(look.emotes, from.emotes) || !sameItems(look.forceRender, from.forceRender))) {
    patch.equip = { wearableUrns: look.wearables, emoteUrns: look.emotes, forceRender: look.forceRender }
  }
  return patch
}

export function sameLook(a: AvatarLook, b: AvatarLook): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

// Session-only state of the preview mannequin: who it is (body shape, colors, base outfit), what it
// wears from the collection, and which emote plays. The editor and the live preview share it.
import { BodyShape, PreviewEmote } from '@dcl/schemas'
import { create } from 'zustand'
import {
  getRandomBaseWearables,
  getRandomBodyShape,
  getRandomColors,
  toBaseWearableUrns,
  type AvatarAttributes,
  type AvatarColors,
  type BaseWearable,
  type BaseWearableCategory,
  type BaseWearableSelection
} from '~/lib/avatar'
import { ItemType } from '~/lib/items'

/** What the avatar needs to know about an item to dress it: one emote and one wearable per slot at a time. */
type Dressable = { id: string; type: ItemType; category?: string }

/** What a dressed id is, kept so a newly dressed item can push out the one it competes with. */
type DressedKind = { type: ItemType; category?: string }

type Dressing = Pick<AvatarPreviewState, 'dressedItemIds' | 'dressedKinds'>

export type AvatarPreviewState = AvatarColors & {
  bodyShape: BodyShape
  /** The base outfit per body shape; null until the catalog has been seeded. */
  baseWearables: Record<BodyShape, BaseWearableSelection> | null
  dressedItemIds: string[]
  /** Kept in the store, not module scope, so it is reset with the ids it describes. */
  dressedKinds: Record<string, DressedKind>
  emote: PreviewEmote
  isPlaying: boolean
  /** The scene boundaries (the cylinder an item must fit in) drawn around the avatar. */
  showSceneBoundaries: boolean
  setBodyShape: (bodyShape: BodyShape) => void
  setColor: (slot: keyof AvatarColors, hex: string) => void
  setBaseWearable: (bodyShape: BodyShape, category: BaseWearableCategory, urn: string | null) => void
  /** First random outfit once the catalog loads; a no-op when one is already set. */
  seedBaseWearables: (catalog: BaseWearable[]) => void
  /** Re-rolls body shape, colors and outfit. */
  randomize: (catalog: BaseWearable[]) => void
  /** Adds an item to the avatar; it replaces any dressed emote (for an emote) or wearable of the same category. */
  dress: (item: Dressable) => void
  undress: (itemId: string) => void
  toggleDressed: (item: Dressable) => void
  setDressed: (items: Dressable[]) => void
  clearDressed: () => void
  setEmote: (emote: PreviewEmote) => void
  endEmote: () => void
  setPlaying: (isPlaying: boolean) => void
  toggleSceneBoundaries: () => void
}

const initialColors = getRandomColors()

export const useAvatarPreview = create<AvatarPreviewState>()((set, get) => ({
  bodyShape: getRandomBodyShape(),
  ...initialColors,
  baseWearables: null,
  dressedItemIds: [],
  dressedKinds: {},
  emote: PreviewEmote.IDLE,
  isPlaying: false,
  showSceneBoundaries: false,
  setBodyShape: bodyShape => set({ bodyShape }),
  setColor: (slot, hex) => set({ [slot]: hex }),
  setBaseWearable: (bodyShape, category, urn) =>
    set(state => {
      const current = state.baseWearables ?? emptySelections()
      return { baseWearables: { ...current, [bodyShape]: { ...current[bodyShape], [category]: urn } } }
    }),
  seedBaseWearables: catalog => {
    if (get().baseWearables) return
    set({
      baseWearables: {
        [BodyShape.MALE]: getRandomBaseWearables(catalog, BodyShape.MALE),
        [BodyShape.FEMALE]: getRandomBaseWearables(catalog, BodyShape.FEMALE)
      }
    })
  },
  randomize: catalog => {
    const bodyShape = getRandomBodyShape()
    set(state => ({
      bodyShape,
      ...getRandomColors(),
      baseWearables: {
        ...(state.baseWearables ?? emptySelections()),
        [bodyShape]: getRandomBaseWearables(catalog, bodyShape)
      }
    }))
  },
  dress: item => set(state => dressOne(state, item)),
  undress: itemId => set(state => undressOne(state, itemId)),
  toggleDressed: item =>
    set(state => (state.dressedItemIds.includes(item.id) ? undressOne(state, item.id) : dressOne(state, item))),
  setDressed: items => set(() => items.reduce(dressOne, EMPTY_DRESSING)),
  clearDressed: () => set(EMPTY_DRESSING),
  setEmote: emote => set({ emote }),
  endEmote: () => set({ emote: PreviewEmote.IDLE, isPlaying: false }),
  setPlaying: isPlaying => set({ isPlaying }),
  toggleSceneBoundaries: () => set(state => ({ showSceneBoundaries: !state.showSceneBoundaries }))
}))

const EMPTY_DRESSING: Dressing = { dressedItemIds: [], dressedKinds: {} }

/** Whether a dressed item has to come off for `item` to go on: the emote playing, or the same slot. */
function competes(kind: DressedKind | undefined, item: Dressable): boolean {
  if (!kind || kind.type !== item.type) return false
  if (item.type === ItemType.EMOTE) return true
  return item.category !== undefined && kind.category === item.category
}

function dressOne(state: Dressing, item: Dressable): Dressing {
  const ids = state.dressedItemIds.filter(id => id === item.id || !competes(state.dressedKinds[id], item))
  const kinds = { ...state.dressedKinds, [item.id]: { type: item.type, category: item.category } }
  return toDressing(ids.includes(item.id) ? ids : [...ids, item.id], kinds)
}

function undressOne(state: Dressing, itemId: string): Dressing {
  return toDressing(
    state.dressedItemIds.filter(id => id !== itemId),
    state.dressedKinds
  )
}

/** Keeps the kinds down to the ids still dressed, so nothing accumulates across collections. */
function toDressing(dressedItemIds: string[], kinds: Record<string, DressedKind>): Dressing {
  const dressedKinds: Record<string, DressedKind> = {}
  for (const id of dressedItemIds) if (kinds[id]) dressedKinds[id] = kinds[id]
  return { dressedItemIds, dressedKinds }
}

function emptySelections(): Record<BodyShape, BaseWearableSelection> {
  const empty: BaseWearableSelection = { hair: null, facial_hair: null, upper_body: null, lower_body: null }
  return { [BodyShape.MALE]: { ...empty }, [BodyShape.FEMALE]: { ...empty } }
}

/** The fields the preview iframe's attributes are derived from. */
export type AvatarSelection = Pick<AvatarPreviewState, 'bodyShape' | 'skin' | 'eyes' | 'hair' | 'baseWearables'>

/** The attributes the preview iframe needs, derived from the store (or from the same fields held elsewhere). */
export function selectAvatarAttributes(state: AvatarSelection): AvatarAttributes {
  return {
    bodyShape: state.bodyShape,
    skin: state.skin,
    eyes: state.eyes,
    hair: state.hair,
    baseWearableUrns: toBaseWearableUrns(state.baseWearables?.[state.bodyShape], state.bodyShape),
    hasOutfit: state.baseWearables !== null
  }
}

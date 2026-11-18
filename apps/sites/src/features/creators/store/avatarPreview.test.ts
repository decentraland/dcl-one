import { beforeEach, describe, expect, it } from 'vitest'
import { BodyShape, PreviewEmote, WearableCategory } from '@dcl/schemas'
import { type BaseWearable } from '~/lib/avatar'
import { ItemType } from '~/lib/items'
import { selectAvatarAttributes, useAvatarPreview } from './avatarPreview'

const catalog: BaseWearable[] = [
  { urn: 'urn:hair_m', category: WearableCategory.HAIR, bodyShapes: [BodyShape.MALE], name: 'Hair' },
  { urn: 'urn:hair_f', category: WearableCategory.HAIR, bodyShapes: [BodyShape.FEMALE], name: 'Hair' },
  {
    urn: 'urn:shirt',
    category: WearableCategory.UPPER_BODY,
    bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
    name: 'Shirt'
  }
]

beforeEach(() => {
  useAvatarPreview.setState({
    baseWearables: null,
    dressedItemIds: [],
    dressedKinds: {},
    emote: PreviewEmote.IDLE,
    isPlaying: false,
    showSceneBoundaries: false,
    bodyShape: BodyShape.MALE
  })
})

describe('avatar preview store', () => {
  it('dresses items and lets only one emote stay on the avatar', () => {
    const { dress, toggleDressed, undress } = useAvatarPreview.getState()
    dress({ id: 'hat', type: ItemType.WEARABLE, category: 'hat' })
    dress({ id: 'e1', type: ItemType.EMOTE })
    dress({ id: 'e2', type: ItemType.EMOTE })
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['hat', 'e2'])
    toggleDressed({ id: 'hat', type: ItemType.WEARABLE, category: 'hat' })
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['e2'])
    undress('e2')
    expect(useAvatarPreview.getState().dressedItemIds).toEqual([])
  })

  it('swaps out a dressed wearable of the same category, keeping the rest', () => {
    const { dress, toggleDressed } = useAvatarPreview.getState()
    dress({ id: 'hat1', type: ItemType.WEARABLE, category: 'hat' })
    dress({ id: 'shirt', type: ItemType.WEARABLE, category: 'upper_body' })
    toggleDressed({ id: 'hat2', type: ItemType.WEARABLE, category: 'hat' })
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['shirt', 'hat2'])
    // Unknown categories never push anything out.
    dress({ id: 'mystery', type: ItemType.WEARABLE })
    dress({ id: 'mystery2', type: ItemType.WEARABLE })
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['shirt', 'hat2', 'mystery', 'mystery2'])
  })

  it('forgets what was dressed when the avatar is cleared, so nothing lingers between collections', () => {
    const { dress, clearDressed } = useAvatarPreview.getState()
    dress({ id: 'hat1', type: ItemType.WEARABLE, category: 'hat' })
    clearDressed()
    expect(useAvatarPreview.getState().dressedItemIds).toEqual([])
    // The cleared hat must not push the new one out: it is no longer on the avatar.
    dress({ id: 'shirt', type: ItemType.WEARABLE, category: 'upper_body' })
    dress({ id: 'hat2', type: ItemType.WEARABLE, category: 'hat' })
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['shirt', 'hat2'])
  })

  it('seeds a random base outfit once and reports the urns for the current body shape', () => {
    const { seedBaseWearables } = useAvatarPreview.getState()
    seedBaseWearables(catalog)
    const seeded = useAvatarPreview.getState().baseWearables
    expect(seeded?.[BodyShape.MALE].hair).toBe('urn:hair_m')
    expect(seeded?.[BodyShape.FEMALE].hair).toBe('urn:hair_f')
    seedBaseWearables([])
    expect(useAvatarPreview.getState().baseWearables).toBe(seeded)
    expect(selectAvatarAttributes(useAvatarPreview.getState()).baseWearableUrns).toEqual([
      'urn:hair_m',
      'urn:shirt',
      'urn:decentraland:off-chain:base-avatars:sneakers'
    ])
  })

  it('lets a slot be changed or emptied and re-rolls everything on randomize', () => {
    const state = useAvatarPreview.getState()
    state.seedBaseWearables(catalog)
    state.setBaseWearable(BodyShape.MALE, WearableCategory.UPPER_BODY, null)
    expect(selectAvatarAttributes(useAvatarPreview.getState()).baseWearableUrns).toEqual([
      'urn:hair_m',
      'urn:decentraland:off-chain:base-avatars:sneakers'
    ])
    state.randomize(catalog)
    const after = useAvatarPreview.getState()
    expect(after.baseWearables?.[after.bodyShape].upper_body).toBe('urn:shirt')
    expect(after.skin).toMatch(/^[0-9a-f]{6}$/)
  })

  it('toggles the scene boundaries', () => {
    const state = useAvatarPreview.getState()
    state.toggleSceneBoundaries()
    expect(useAvatarPreview.getState().showSceneBoundaries).toBe(true)
    state.toggleSceneBoundaries()
    expect(useAvatarPreview.getState().showSceneBoundaries).toBe(false)
  })
})

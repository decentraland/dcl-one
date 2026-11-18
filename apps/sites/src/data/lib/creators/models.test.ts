import { describe, expect, it } from 'vitest'
import { getEmoteMetrics } from './models'

const gltf = (armatures: string[], propDuration = 1) =>
  ({
    scene: { children: armatures.map(name => ({ name, children: [] })) },
    animations: [{ duration: 1, tracks: [] }, ...(armatures.length > 1 ? [{ duration: propDuration, tracks: [] }] : [])]
  }) as unknown as Parameters<typeof getEmoteMetrics>[0]

describe('getEmoteMetrics', () => {
  it('rejects a social emote', () => {
    expect(() => getEmoteMetrics(gltf(['Armature', 'Armature_Other']))).toThrow(
      expect.objectContaining({ name: 'ItemFileError', messageKey: 'emote_social_unsupported' })
    )
  })

  it('rejects a prop animation whose duration differs from the avatar one', () => {
    expect(() => getEmoteMetrics(gltf(['Armature', 'Armature_Prop'], 2))).toThrow(
      expect.objectContaining({ name: 'ItemFileError', messageKey: 'emote_animations_out_of_sync' })
    )
  })

  it('reads a plain emote with a prop', () => {
    expect(getEmoteMetrics(gltf(['Armature', 'Armature_Prop']))).toMatchObject({ props: 1, additionalArmatures: 0 })
  })
})

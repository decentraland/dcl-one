import { describe, expect, it } from 'vitest'
import { AnimationClip, Bone, Object3D, QuaternionKeyframeTrack, VectorKeyframeTrack } from 'three'
import { applyPose } from './renderPosedThumbnail'

function rig() {
  const root = new Object3D()
  const hips = new Bone()
  hips.name = 'Avatar_Hips'
  const spine = new Bone()
  spine.name = 'Avatar_Spine001'
  const other = new Bone()
  other.name = 'Avatar_Head'
  root.add(hips)
  hips.add(spine)
  hips.add(other)
  return { root, hips, spine, other }
}

describe('applyPose', () => {
  it('writes the first keyframe of each track into the bone with the same base name', () => {
    const { root, hips, spine, other } = rig()
    const clip = new AnimationClip('pose', 1, [
      new QuaternionKeyframeTrack('Avatar_Hips.quaternion', [0, 1], [0, 0, 0, 1, 0, 1, 0, 0]),
      new VectorKeyframeTrack('Avatar_Spine.position', [0], [1, 2, 3]),
      new VectorKeyframeTrack('Avatar_Spine.scale', [0], [2, 2, 2])
    ])

    applyPose(root, [clip])

    expect(hips.quaternion.toArray()).toEqual([0, 0, 0, 1])
    expect(spine.position.toArray()).toEqual([1, 2, 3])
    expect(spine.scale.toArray()).toEqual([2, 2, 2])
    expect(other.position.toArray()).toEqual([0, 0, 0])
  })

  it('ignores tracks for bones the model does not have and non-transform properties', () => {
    const { root, hips } = rig()
    const clip = new AnimationClip('pose', 1, [
      new VectorKeyframeTrack('Avatar_Tail.position', [0], [1, 1, 1]),
      new VectorKeyframeTrack('Avatar_Hips.morphTargetInfluences', [0], [1, 1, 1])
    ])

    expect(() => applyPose(root, [clip])).not.toThrow()
    expect(hips.position.toArray()).toEqual([0, 0, 0])
  })
})

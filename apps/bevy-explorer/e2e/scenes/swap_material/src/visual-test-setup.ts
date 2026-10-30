import {
  AvatarModifierArea,
  AvatarModifierType,
  Entity,
  MainCamera,
  Transform,
  VirtualCamera,
  engine,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

/**
 * Visual-regression scenes need two things real Decentraland scenes don't:
 *
 *   1. The local avatar hidden. Test snapshots are pixel-diffed against a
 *      committed baseline; a wandering avatar in frame produces a different
 *      diff every run. AvatarModifierArea + AMT_HIDE_AVATARS removes it from
 *      the area covering the parcel. (Always applied.)
 *
 *   2. A fixed camera pose. The default third-person camera follows the
 *      player, so any spawn jitter or animation shifts the framing. A
 *      VirtualCamera locked to a transform pins the shot. (Applied only
 *      when `lookAt` or `lookAtPos` is provided — UI-only scenes don't
 *      need a virtual camera.)
 *
 * Aim target options:
 *   - `lookAt: Entity` — frame on a specific entity.
 *   - `lookAtPos: Vector3` — frame on an arbitrary world position. Useful when
 *     the subject is a composition (a grid, a multi-asset scene) with no single
 *     anchor. Pass the geometric center of what you want in frame.
 *
 * Pass at most one of `lookAt`/`lookAtPos`. If both are set, `lookAt` wins.
 */
export function setupVisualTest(
  opts: { lookAt?: Entity; lookAtPos?: Vector3; cameraPos?: Vector3 } = {},
) {
  const { lookAt, lookAtPos, cameraPos = Vector3.create(8, 3, 0.5) } = opts

  const hideArea = engine.addEntity()
  Transform.create(hideArea, { position: Vector3.create(8, 1, 8) })
  AvatarModifierArea.create(hideArea, {
    area: Vector3.create(16, 16, 16),
    modifiers: [AvatarModifierType.AMT_HIDE_AVATARS, AvatarModifierType.AMT_DISABLE_PASSPORTS],
    excludeIds: [],
  })

  if (lookAt !== undefined) {
    const cam = engine.addEntity()
    Transform.create(cam, { position: cameraPos })
    VirtualCamera.create(cam, { lookAtEntity: lookAt })
    MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: cam })
    return
  }

  if (lookAtPos !== undefined) {
    const anchor = engine.addEntity()
    Transform.create(anchor, { position: lookAtPos })

    const cam = engine.addEntity()
    Transform.create(cam, { position: cameraPos })
    VirtualCamera.create(cam, { lookAtEntity: anchor })
    MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: cam })
  }
}

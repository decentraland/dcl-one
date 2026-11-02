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
 *   - `lookAt: Entity` — frame on a specific entity. Useful when one
 *     subject anchors the shot.
 *   - `lookAtPos: Vector3` — frame on an arbitrary world position.
 *     Useful when the subject is a *composition* with no single anchor
 *     (a grid, a multi-asset scene, a transform-tree group). Pass the
 *     geometric center of what you want in frame.
 *
 * Pass at most one of `lookAt`/`lookAtPos`. If both are set, `lookAt` wins.
 *
 * Call this at the END of `main()` after the scene's content entities have
 * been created (entity-aim needs the entity to exist).
 *
 * Override or skip this helper when the scene specifically tests avatar
 * rendering, camera behavior, or any feature this setup would suppress.
 *
 * If you change camera position or aim for a scene, re-record its baselines:
 *   metaforge explorer test --filter "..." --record-baselines
 */
export function setupVisualTest(
  opts: {
    lookAt?: Entity
    lookAtPos?: Vector3
    cameraPos?: Vector3
    /** Center of the AvatarModifierArea. Default: (8,1,8) — single 0,0 parcel. */
    hideAreaCenter?: Vector3
    /** Box size of the AvatarModifierArea. Default: 16×16×16 — single parcel. */
    hideAreaSize?: Vector3
  } = {},
) {
  const {
    lookAt,
    lookAtPos,
    cameraPos = Vector3.create(8, 3, -15),
    hideAreaCenter = Vector3.create(8, 1, 8),
    hideAreaSize = Vector3.create(16, 16, 16),
  } = opts

  const hideArea = engine.addEntity()
  Transform.create(hideArea, { position: hideAreaCenter })
  AvatarModifierArea.create(hideArea, {
    area: hideAreaSize,
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
    // VirtualCamera only knows how to look at an Entity, so spawn an invisible
    // anchor at the requested world position and aim at it.
    const anchor = engine.addEntity()
    Transform.create(anchor, { position: lookAtPos })

    const cam = engine.addEntity()
    Transform.create(cam, { position: cameraPos })
    VirtualCamera.create(cam, { lookAtEntity: anchor })
    MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: cam })
  }
}

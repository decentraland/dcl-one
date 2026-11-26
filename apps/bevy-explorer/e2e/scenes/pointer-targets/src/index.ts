import type { Entity } from '@dcl/sdk/ecs'
import {
  ColliderLayer,
  GltfContainer,
  InputAction,
  MeshCollider,
  MeshRenderer,
  PointerEventType,
  PointerEvents,
  Transform,
  engine,
  inputSystem,
} from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

/**
 * pointer-targets: the targets of `specs/visual/pointer.test.ts`, which hovers and clicks them
 * through `/hover_entity` and `/click_entity`.
 *
 * The player spawns at (8, 0, 8). The spawn point's `cameraTarget` faces +X, but the explorer
 * does not turn the player to it on a teleport, so the player may face +Z with the camera about
 * 5 m behind, near (9, 2, 3), or face +X with the camera near (3, 2, 8). Every ray starts at the
 * camera, so the layout holds for both:
 *   - z = 10 to 16: `near` (12, 1, 11), then `wall` (14, 3, 13), 0.5 by 6 by 6, then `behind`
 *     (16, 1, 15), which the wall hides from either camera;
 *   - z = 8: `far` (58, 1, 8), 50 m out, whose ray crosses x = 14 below z = 10 from either camera.
 * So the wall blocks only `behind`.
 * `model` is the avocado glTF, its pointer events on the container, with the fruit at (4, 0, 12),
 * left of every other ray. The glb's root node carries the export's placement, a translation of
 * (8.93, 0, 20.04) in scene axes and a scale of 38.8: the container, scaled 0.4, sits at that
 * translation times 0.4 short of (4, 0, 12), and the fruit is about 0.7 by 1 m. The avocado has
 * no `_collider` mesh, so its visible meshes collide on CL_POINTER and CL_PHYSICS
 * (`visibleMeshesCollisionMask`) for the pointer ray to hit it.
 *
 * `far` must be reachable at 50 m from the camera: its entry sets `maxCameraDistance: 60`
 * (`max_camera_distance`, present in the vendored SDK), which allows the interaction by camera
 * distance whatever the player distance's default of 10 m says.
 *
 * Every frame, for each target and for PET_DOWN and PET_UP on IA_POINTER, the scene reads the
 * last input command and logs each new one once: `pointer down entity=<id> tick=<tick>` or
 * `pointer up entity=<id> tick=<tick>`. `pointer-targets ready` is logged once the entities
 * exist.
 */

/** The translation of `avocado.glb`'s root node, in scene axes (the glTF's x and z turned). */
const AVOCADO_NODE = Vector3.create(8.926521, 0, 20.036209)
const MODEL_SCALE = 0.4
const MODEL_AT = Vector3.create(4, 0, 12)

function box(position: Vector3, scale: Vector3 = Vector3.One()): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position, scale })
  MeshRenderer.setBox(entity)
  MeshCollider.setBox(entity)
  return entity
}

export function main() {
  const near = box(Vector3.create(12, 1, 11))
  PointerEvents.create(near, {
    pointerEvents: [
      { eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Near' } },
      { eventType: PointerEventType.PET_UP, eventInfo: { button: InputAction.IA_POINTER } },
    ],
  })

  box(Vector3.create(14, 3, 13), Vector3.create(0.5, 6, 6))

  const behind = box(Vector3.create(16, 1, 15))
  PointerEvents.create(behind, {
    pointerEvents: [{ eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Behind' } }],
  })

  const far = box(Vector3.create(58, 1, 8))
  PointerEvents.create(far, {
    pointerEvents: [
      { eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Far', maxCameraDistance: 60 } },
    ],
  })

  const model = engine.addEntity()
  Transform.create(model, {
    position: Vector3.create(
      MODEL_AT.x - AVOCADO_NODE.x * MODEL_SCALE,
      MODEL_AT.y - AVOCADO_NODE.y * MODEL_SCALE,
      MODEL_AT.z - AVOCADO_NODE.z * MODEL_SCALE,
    ),
    scale: Vector3.create(MODEL_SCALE, MODEL_SCALE, MODEL_SCALE),
  })
  GltfContainer.create(model, {
    src: 'assets/avocado.glb',
    visibleMeshesCollisionMask: ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS,
  })
  PointerEvents.create(model, {
    pointerEvents: [{ eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Model' } }],
  })

  const targets = [near, behind, far, model]
  const kinds: [PointerEventType, string][] = [
    [PointerEventType.PET_DOWN, 'down'],
    [PointerEventType.PET_UP, 'up'],
  ]
  const seen = new Map<string, number>()
  engine.addSystem(() => {
    for (const entity of targets) {
      for (const [eventType, state] of kinds) {
        const cmd = inputSystem.getInputCommand(InputAction.IA_POINTER, eventType, entity)
        if (!cmd) continue
        const key = `${state} ${entity}`
        if (seen.get(key) === cmd.timestamp) continue
        seen.set(key, cmd.timestamp)
        console.log(`pointer ${state} entity=${entity} tick=${cmd.tickNumber}`)
      }
    }
  })

  console.log('pointer-targets ready')
}

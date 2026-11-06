import {
  Entity,
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
 * The player spawns at (8, 0, 8) facing +X, so the camera sits on the line z = 8 and every ray
 * starts there. Two lanes keep the cases apart:
 *   - z = 4: `near` (12, 1, 4), then `wall` (14, 3, 4), 0.5 by 6 by 6, then `behind` (16, 1, 4),
 *     which the wall hides from the camera;
 *   - z = 8 and z = 12: `far` (58, 1, 8), 50 m out, and `beyond` (158, 1, 12), 150 m out, past
 *     the cursor ray's 100 m. On the z = 8 line the wall would hide both; `beyond` is off that
 *     line so the ray to it passes beside `far`.
 * `model` is the avocado glTF at (10, 0, 12), scaled 4, with its pointer events on the container.
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

function box(position: Vector3, scale: Vector3 = Vector3.One()): Entity {
  const entity = engine.addEntity()
  Transform.create(entity, { position, scale })
  MeshRenderer.setBox(entity)
  MeshCollider.setBox(entity)
  return entity
}

export function main() {
  const near = box(Vector3.create(12, 1, 4))
  PointerEvents.create(near, {
    pointerEvents: [
      { eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Near' } },
      { eventType: PointerEventType.PET_UP, eventInfo: { button: InputAction.IA_POINTER } },
    ],
  })

  box(Vector3.create(14, 3, 4), Vector3.create(0.5, 6, 6))

  const behind = box(Vector3.create(16, 1, 4))
  PointerEvents.create(behind, {
    pointerEvents: [{ eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Behind' } }],
  })

  const far = box(Vector3.create(58, 1, 8))
  PointerEvents.create(far, {
    pointerEvents: [
      { eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Far', maxCameraDistance: 60 } },
    ],
  })

  const beyond = box(Vector3.create(158, 1, 12))
  PointerEvents.create(beyond, {
    pointerEvents: [{ eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Beyond' } }],
  })

  const model = engine.addEntity()
  Transform.create(model, { position: Vector3.create(10, 0, 12), scale: Vector3.create(4, 4, 4) })
  GltfContainer.create(model, { src: 'assets/avocado.glb' })
  PointerEvents.create(model, {
    pointerEvents: [{ eventType: PointerEventType.PET_DOWN, eventInfo: { button: InputAction.IA_POINTER, hoverText: 'Model' } }],
  })

  const targets = [near, behind, far, beyond, model]
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

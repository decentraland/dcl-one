import { MeshCollider, RaycastQueryType, Transform, engine, raycastSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

/**
 * raycast-storm: the load of `specs/visual/raycast-budget.test.ts`. A 10 by 20 grid of 200
 * invisible entities at height 1 across the parcel, each with a continuous local-direction
 * raycast straight down (10 m, hit first), over one floor box collider at (8, -0.5, 8) scaled
 * (16, 1, 16). Every frame asks for 200 casts, so a low `/raycast_budget` defers most of them.
 *
 * The callback logs `raycast entity=<id>` the first time it fires for an entity, and
 * `raycast-storm complete` once all 200 have fired.
 */

const COLUMNS = 10
const ROWS = 20

export function main() {
  const floor = engine.addEntity()
  Transform.create(floor, { position: Vector3.create(8, -0.5, 8), scale: Vector3.create(16, 1, 16) })
  MeshCollider.setBox(floor)

  const answered = new Set<number>()
  const total = COLUMNS * ROWS
  for (let column = 0; column < COLUMNS; column++) {
    for (let row = 0; row < ROWS; row++) {
      const entity = engine.addEntity()
      Transform.create(entity, {
        position: Vector3.create(((column + 0.5) * 16) / COLUMNS, 1, ((row + 0.5) * 16) / ROWS),
      })
      raycastSystem.registerLocalDirectionRaycast(
        {
          entity,
          opts: {
            direction: Vector3.Down(),
            maxDistance: 10,
            continuous: true,
            queryType: RaycastQueryType.RQT_HIT_FIRST,
          },
        },
        () => {
          if (answered.has(entity)) return
          answered.add(entity)
          console.log(`raycast entity=${entity}`)
          if (answered.size === total) console.log('raycast-storm complete')
        }
      )
    }
  }
}

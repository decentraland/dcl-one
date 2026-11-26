import type { Entity } from '@dcl/sdk/ecs'
import { engine, Transform, MeshCollider } from '@dcl/sdk/ecs'
export * from '@dcl/sdk'

const cube = engine.addEntity()
Transform.create(cube)

let prevEntity: Entity | null = null
function system() {
  if (prevEntity) engine.removeEntity(prevEntity)
  prevEntity = engine.addEntity()
  MeshCollider.setBox(prevEntity)
}

engine.addSystem(system)

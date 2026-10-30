import { engine, GltfContainer, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

export function main() {
  const model = engine.addEntity()
  Transform.create(model, {
    position: Vector3.create(8, 8, 8),
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  GltfContainer.create(model, {
    src: 'assets/Models/grid_uvs.glb',
  })

  setupVisualTest({
    lookAtPos: Vector3.create(8, 8, 8),
    cameraPos: Vector3.create(8, 8, 0.5),
  })
}

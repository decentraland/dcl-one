import { engine, GltfContainer, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

export function main() {
  const materialGrid = engine.addEntity()
  Transform.create(materialGrid, {
    position: Vector3.create(8, 8, 8),
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  GltfContainer.create(materialGrid, {
    src: 'assets/Models/gltf_material_test_grid.glb',
  })

  setupVisualTest({
    lookAtPos: Vector3.create(8, 8, 8),
    cameraPos: Vector3.create(8, 8, -0.5),
  })
}

import { engine, GltfContainer, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

// The grid is about 18 m wide: centred on the scene's two parcels (x 16) it stays inside them. At
// x 8 its west end crossed the scene's edge, where the engine dissolves geometry with a noise that
// moves with time, so no two frames agreed.
const CENTRE = Vector3.create(16, 8, 8)

export function main() {
  const materialGrid = engine.addEntity()
  Transform.create(materialGrid, {
    position: CENTRE,
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  GltfContainer.create(materialGrid, {
    src: 'assets/Models/gltf_material_test_grid.glb',
  })

  setupVisualTest({
    lookAtPos: CENTRE,
    cameraPos: Vector3.create(CENTRE.x, CENTRE.y, -0.5),
  })
}

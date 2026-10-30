import { engine, GltfContainer, Material, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

export function main() {


  // ── hero: SampleScene_*.glb panels at distinct positions ─────────────────
  // 01 left, 02 right at the same height; 03 lifted above and centered so
  // the three showcases don't visually overlap. y-lift is screenshot-only —
  // ground contact doesn't matter for visual diff.
  const sampleScene = engine.addEntity()
  Transform.create(sampleScene, {
    scale: Vector3.create(0.5, 0.5, 0.5),
    position: Vector3.create(16, 0, 1),
    rotation: Quaternion.fromEulerDegrees(0, 0, 0)
  })
  GltfContainer.create(sampleScene, { src: 'assets/testscene2.glb' })

  // Camera on the +Z side of the panels so the showcase fronts face it.
  // Pulled back to z=30 to fit the full 32m-wide layout (tree at x=3 →
  // treehouse at x=29) inside the HFOV. Aim raised to y=5 so the lifted
  // SampleScene_03 (y=8+) and the y=2 foreground accents both fit vertically.
  setupVisualTest({
    lookAtPos: Vector3.create(16, 3.3, 6),
    cameraPos: Vector3.create(16, 4, -5),
  })


}

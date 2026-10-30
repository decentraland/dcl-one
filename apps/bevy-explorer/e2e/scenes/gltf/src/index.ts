import {
  Animator,
  GltfContainer,
  LightSource,
  Material,
  MeshRenderer,
  Transform,
  engine,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * gltf — GLTF asset import + multi-mesh assemblies + PBR showcase walls.
 *
 * Visual contract: any regression in the GLTF importer (vertex layouts,
 * embedded materials, asset bundle conversion, mesh hierarchy, transparency
 * pipeline) shows up here.
 *
 * Hero subjects: the three SampleScene_*.glb panels — each a distinct PBR /
 * material showcase (rows of spheres, alpha modes, vertex-color cells, the
 * damagedHelmet asset, etc.). Laid out side-by-side; SampleScene_03 is
 * raised above the other two so it doesn't visually overlap them.
 *
 * Static accent models flank the panels:
 *   tree · treehouse · gecko-statue · avocado · npc-robot
 *
 * Animated/rigged assets from the source scene (AnimatedCube, *Morph,
 * RiggedAnimation, RiggedDracoAnimation) are intentionally omitted — their
 * runtime animation state can't be reliably pixel-diffed. SampleScene_03's
 * one embedded animation (the rotating damagedHelmet) is suppressed via an
 * Animator with the clip explicitly held at playing=false.
 *
 * The virtual camera sits on the +Z side of the panels so the showcase
 * fronts face it. Player spawn is set on the same side so manual inspection
 * matches the recorded shot.
 */
export function main() {
  // ── ground covering both parcels ─────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(16, 0, 8),
    scale: Vector3.create(32, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.42, 0.45, 0.4, 1),
    roughness: 0.95,
  })

  // ── hero: SampleScene_*.glb panels at distinct positions ─────────────────
  // 01 left, 02 right at the same height; 03 lifted above and centered so
  // the three showcases don't visually overlap. y-lift is screenshot-only —
  // ground contact doesn't matter for visual diff.
  // const sampleScene = engine.addEntity()
  // Transform.create(sampleScene, { position: Vector3.create(8, 3, 4) })
  // GltfContainer.create(sampleScene, { src: 'assets/sdk7-models/SampleScene.glb' })



  const sampleScene03 = engine.addEntity()
  Transform.create(sampleScene03, { position: Vector3.create(14, 6, 2) })
  GltfContainer.create(sampleScene03, {
    src: 'assets/sdk7-models/SampleScene_03.glb',
  })
  // The damagedHelmet inside SampleScene_03 has an embedded animation that
  // would auto-play and break the snapshot diff. Declare the clip with
  // playing=false to keep it pinned at frame 0.
  Animator.create(sampleScene03)

  // ── accent models flanking the panels ────────────────────────────────────
  // Camera looks toward -z; default GLB orientation faces +Z, so accents
  // placed at smaller z than the camera face the camera with no rotation.
  // tree/treehouse outboard of the side panels at z=4; gecko/avocado/npc-robot
  // closer to camera at z=11.
  const tree = engine.addEntity()
  Transform.create(tree, {
    position: Vector3.create(2, 0, 14),
    scale: Vector3.create(0.6, 0.6, 0.6),
  })
  GltfContainer.create(tree, { src: 'assets/genesis/tree.glb' })

  const treehouse = engine.addEntity()
  Transform.create(treehouse, {
    position: Vector3.create(22, 0, 14),
    scale: Vector3.create(0.6, 0.6, 0.6),
  })
  GltfContainer.create(treehouse, { src: 'assets/genesis/treehouse.glb' })

  // Foreground accents lifted to y=2 so they aren't clipped at the frame
  // bottom, and pulled in toward x=10..22 so they stay inside the HFOV at
  // z=11 (closest row, narrowest visible width).
  const gecko = engine.addEntity()
  Transform.create(gecko, {
    position: Vector3.create(10, 0, 14),
    scale: Vector3.create(2, 2, 2),
  })
  GltfContainer.create(gecko, { src: 'assets/genesis/gecko-statue.glb' })

  // avocado is ~5cm in source; scale up so it reads in frame.
  const avocado = engine.addEntity()
  Transform.create(avocado, {
    position: Vector3.create(5, 1, -5),
    scale: Vector3.create(1, 1, 1),
    // rotation: Quaternion.fromEulerDegrees(0, 180, 0)
  })
  GltfContainer.create(avocado, { src: 'assets/sdk7-models/avocado.glb' })
  Animator.create(avocado)


  const robot = engine.addEntity()
  Transform.create(robot, {
    position: Vector3.create(18, 1, 14),
    scale: Vector3.create(2, 2, 2),
    rotation: Quaternion.fromEulerDegrees(0, 180, 0)
  })
  GltfContainer.create(robot, { src: 'assets/sdk7-models/external-deps/npc-robot/s0_NPC_Robot_Art_1__01.glb' })

  Animator.create(robot)


  // no Transform, partially out of bounds
  const table = engine.addEntity()

  GltfContainer.create(table, { src: 'assets/sdk7-models/WoodRoundTable_01.glb' })

  // ── lighting ─────────────────────────────────────────────────────────────
  // Two point lights spread over the wider 32m × 16m scene, one over each
  // depth band so panels and foreground are evenly lit.
  const keyLight = engine.addEntity()
  Transform.create(keyLight, { position: Vector3.create(16, 9, 6) })
  LightSource.create(keyLight, {
    color: Color3.create(1, 0.97, 0.9),
    intensity: 130000,
    range: 34,
    active: true,
    shadow: false,
    type: LightSource.Type.Point({}),
  })

  const fillLight = engine.addEntity()
  Transform.create(fillLight, { position: Vector3.create(16, 6, 12) })
  LightSource.create(fillLight, {
    color: Color3.create(0.95, 0.95, 1),
    intensity: 80000,
    range: 26,
    active: true,
    shadow: false,
    type: LightSource.Type.Point({}),
  })

  // Camera on the +Z side of the panels so the showcase fronts face it.
  // Pulled back to z=30 to fit the full 32m-wide layout (tree at x=3 →
  // treehouse at x=29) inside the HFOV. Aim raised to y=5 so the lifted
  // SampleScene_03 (y=8+) and the y=2 foreground accents both fit vertically.
  setupVisualTest({
    lookAtPos: Vector3.create(12, 4, 6),
    cameraPos: Vector3.create(12, 5, 25),
  })
}

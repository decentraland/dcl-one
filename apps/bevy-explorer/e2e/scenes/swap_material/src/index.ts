import {
  Entity,
  GltfContainer,
  GltfNodeModifiers,
  InputAction,
  LightSource,
  Material,
  MaterialTransparencyMode,
  MeshRenderer,
  TextAlignMode,
  TextShape,
  Transform,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * swap_material — exhaustive GltfNodeModifiers coverage.
 *
 * Visual contract: any regression in GLTF node targeting, material override
 * dispatch, or per-node shadow toggle shows up here.
 *
 * Subjects:
 *   - Robots.glb — 4 sibling sub-meshes side-by-side (Drawer_03, Droid_01,
 *     Droid_02, FloorModuleSciFi_01) sharing one material. Used to verify
 *     path='' (apply to all), path=<specific node> (apply to one), multiple
 *     modifiers in the same array (apply to two), and castShadows toggle.
 *   - ban-cube.glb — a single-node cube, perfect for testing texture-bearing
 *     material overrides (Images/lava-texture and Images/smoke-puff).
 *
 * Layout: back row = 5 Robot variations (baseline + 4 modifier scenarios);
 * sides = 2 ban-cube texture overrides. Camera is pulled back on -Z so every
 * subject is in frame for the snapshot.
 */

const ROBOTS = 'assets/Models/Robots.glb'
const BAN_CUBE = 'assets/Models/ban-cube.glb'
const LAVA_TEX = 'assets/Images/lava-texture.jpg'
const SMOKE_TEX = 'assets/Images/smoke-puff.png'

const ROBOT_SCALE = Vector3.create(0.75, 0.75, 0.75)
const CUBE_SCALE = Vector3.create(1.5, 1.5, 1.5)

export function main() {
  // ── ground (lets per-node shadow-toggle differences read) ───────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.45, 0.46, 0.48, 1),
    roughness: 0.95,
    metallic: 0,
  })

  // ═══ ROBOTS row — z=12, scale 0.5, x spread across the parcel ═══════════
  // Each Robots.glb instance contains 4 sibling top-level nodes:
  //   Drawer_03 · Droid_01 · Droid_02 · FloorModuleSciFi_01
  // All share material 'SciFiPack_MAT'.
  const robotZ = 12
  const robotXs = [1.5, 5, 8.5, 12, 15.5]

  // R1 — baseline. No GltfNodeModifiers. Control for visual diff.
  {
    const e = mkRobot(robotXs[0], robotZ)
    attachLabel('baseline', robotXs[0], robotZ)
    attachHint(e, 'Baseline · no GltfNodeModifiers (control)')
  }

  // R2 — swap ALL: path='' applies the modifier to every node in the GLTF.
  // Vivid red emissive so any node still rendering the original SciFiPack
  // material stands out immediately in the diff.
  {
    const e = mkRobot(robotXs[1], robotZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: '',
          material: pbr({
            albedoColor: Color4.create(0.9, 0.1, 0.1, 1),
            emissiveColor: Color3.create(1, 0.1, 0.1),
            emissiveIntensity: 1.5,
            metallic: 0,
            roughness: 0.4,
          }),
        },
      ],
    })
    attachLabel('swap all → red', robotXs[1], robotZ)
    attachHint(e, "Swap ALL · path='' applies override to every node (red emissive)")
  }

  // R3 — swap ONE: target only Droid_01. The other three nodes (Drawer_03,
  // Droid_02, FloorModuleSciFi_01) must keep the original SciFiPack material.
  // If path matching regresses, either Droid_01 stays original (no green
  // sphere) or other nodes turn green too.
  {
    const e = mkRobot(robotXs[2], robotZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: 'Droid_01',
          material: pbr({
            albedoColor: Color4.create(0.1, 0.85, 0.2, 1),
            emissiveColor: Color3.create(0.1, 1, 0.2),
            emissiveIntensity: 1.5,
            metallic: 0,
            roughness: 0.4,
          }),
        },
      ],
    })
    attachLabel('swap one (Droid_01)', robotXs[2], robotZ)
    attachHint(e, 'Swap ONE · path=Droid_01 only (green) · other nodes keep original')
  }

  // R4 — swap TWO: two modifiers in the same array, each targeting a
  // different node. Verifies the renderer iterates modifiers rather than
  // stopping at the first match.
  {
    const e = mkRobot(robotXs[3], robotZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: 'Droid_01',
          material: pbr({
            albedoColor: Color4.create(0.1, 0.7, 0.95, 1),
            emissiveColor: Color3.create(0.1, 0.8, 1),
            emissiveIntensity: 1.5,
            metallic: 0,
            roughness: 0.4,
          }),
        },
        {
          path: 'Droid_02',
          material: pbr({
            albedoColor: Color4.create(0.95, 0.1, 0.85, 1),
            emissiveColor: Color3.create(1, 0.1, 0.9),
            emissiveIntensity: 1.5,
            metallic: 0,
            roughness: 0.4,
          }),
        },
      ],
    })
    attachLabel('swap two', robotXs[3], robotZ)
    attachHint(e, 'Swap TWO · Droid_01=cyan · Droid_02=magenta · siblings keep original')
  }

  // R5 — shadow opt-out: path='' with castShadows=false (no material
  // override). The four nodes still render with their original SciFiPack
  // material but their ground shadows must disappear. Diff against R1
  // (baseline, shadows on) is the controlled comparison.
  {
    const e = mkRobot(robotXs[4], robotZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: '',
          castShadows: false,
        },
      ],
    })
    attachLabel('no shadows', robotXs[4], robotZ)
    attachHint(
      e,
      "No shadows · path='' castShadows=false · same materials as baseline, no ground shadow",
    )
  }

  // ═══ BAN-CUBE row — z=5, sides of the parcel ════════════════════════════
  // Single-node GLTF ('Cube'). Apply the modifier with path='' (single node,
  // so path='' and path='Cube' are equivalent — '' is the simpler form) and
  // override the material to carry a texture from assets/Images.
  // Cubes are pushed to the x-edges so they don't occlude the central robots
  // from the camera's POV.
  const cubeZ = 12
  const cubeY = 6
  const cubeLabelY = cubeY + 1.5

  // C1 — lava texture (opaque JPG, full UV).
  {
    const e = mkBanCube(2.5, cubeY, cubeZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: '',
          material: pbr({
            texture: Material.Texture.Common({ src: LAVA_TEX }),
            emissiveColor: Color3.create(1, 0.4, 0.1),
            emissiveTexture: Material.Texture.Common({ src: LAVA_TEX }),
            emissiveIntensity: 1.2,
            metallic: 0,
            roughness: 0.6,
          }),
        },
      ],
    })
    attachLabel('lava texture', 2.5, cubeZ, cubeLabelY + 1)
    attachHint(e, 'Texture override · lava-texture.jpg via GltfNodeModifiers')
  }

  // C2 — smoke-puff texture (alpha-blended PNG with soft edges). Tests that
  // texture overrides honor transparency mode + albedo color tint.
  {
    const e = mkBanCube(13.5, cubeY, cubeZ)
    GltfNodeModifiers.create(e, {
      modifiers: [
        {
          path: '',
          material: pbr({
            texture: Material.Texture.Common({ src: SMOKE_TEX }),
            albedoColor: Color4.create(0.6, 0.8, 1, 1),
            transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
            metallic: 0,
            roughness: 0.7,
          }),
        },
      ],
    })
    attachLabel('smoke texture', 13.5, cubeZ, cubeLabelY + 1)
    attachHint(e, 'Texture override · smoke-puff.png (alpha-blend) via GltfNodeModifiers')
  }

  // ── lighting ────────────────────────────────────────────────────────────
  // Warm key + cool fill. Range tuned so the back robots at z=12 still
  // receive direct light, otherwise emissive overrides would carry the whole
  // visual signal and dim the baseline / shadow-off control rows.
  const keyLight = engine.addEntity()
  Transform.create(keyLight, { position: Vector3.create(8, 8, 4) })
  LightSource.create(keyLight, {
    color: Color3.create(1, 0.97, 0.9),
    intensity: 130000,
    range: 28,
    active: true,
    shadow: false,
    type: LightSource.Type.Point({}),
  })

  const fillLight = engine.addEntity()
  Transform.create(fillLight, {
    position: Vector3.create(8, 9, 14),
    rotation: Quaternion.fromEulerDegrees(45, 0, 0),
  })
  LightSource.create(fillLight, {
    color: Color3.create(0.7, 0.8, 1),
    intensity: 70000,
    range: 24,
    active: true,
    shadow: false,
    type: LightSource.Type.Spot({ innerAngle: 30, outerAngle: 70 }),
  })

  // Camera framed so back row (z=12) and side cubes (z=5) are both in shot.
  // Aim is biased toward the back row's vertical mid-line and slightly past
  // scene center on z so the robots dominate the frame.
  setupVisualTest({
    lookAtPos: Vector3.create(8, 4, 9),
    cameraPos: Vector3.create(8, 5, 2),
  })
}

function mkRobot(x: number, z: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, 0, z),
    scale: ROBOT_SCALE,
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  GltfContainer.create(e, { src: ROBOTS })
  return e
}

function mkBanCube(x: number, y: number, z: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, y, z),
    scale: CUBE_SCALE,
    rotation: Quaternion.fromEulerDegrees(0, 25, 0),
  })
  GltfContainer.create(e, { src: BAN_CUBE })
  return e
}

// Wraps a PbrMaterial into the PBMaterial union form that GltfNodeModifier
// expects on its `material` field.
function pbr(pbr: NonNullable<Parameters<typeof Material.setPbrMaterial>[1]>) {
  return { material: { $case: 'pbr' as const, pbr } }
}

// Floating caption above each subject so the rendered scene is self-documenting.
function attachLabel(text: string, x: number, z: number, y: number = 3.4) {
  const label = engine.addEntity()
  Transform.create(label, {
    position: Vector3.create(x, y, z)
  })
  TextShape.create(label, {
    text,
    fontSize: 3,
    textColor: Color4.Black(),
    outlineColor: Color4.White(),
    outlineWidth: 0.15,
    textAlign: TextAlignMode.TAM_MIDDLE_CENTER,
  })
}

const HINT_MAX_DISTANCE = 25

// Flip to true to wire up hover tooltips while debugging the scene by hand.
// Kept off for visual-test runs so the cursor never paints a hover outline.
const DEBUG_HOVER_HINTS = false

function attachHint(entity: Entity, text: string) {
  if (!DEBUG_HOVER_HINTS) return
  pointerEventsSystem.onPointerDown(
    {
      entity,
      opts: {
        button: InputAction.IA_POINTER,
        hoverText: text,
        maxDistance: HINT_MAX_DISTANCE,
      },
    },
    () => { },
  )
}

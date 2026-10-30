import {
  Entity,
  InputAction,
  LightSource,
  Material,
  MaterialTransparencyMode,
  MeshCollider,
  MeshRenderer,
  TextureFilterMode,
  TextureWrapMode,
  Transform,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector2, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * materials — exhaustive PBR/Basic property + texture matrix.
 *
 * Visual contract: a 4-layer grid (4 rows × 5 cols per layer) of subjects lit
 * identically, each row varying ONE material axis. Any shader regression —
 * BRDF rewrite, lighting model tweak, alpha pipeline change, transparency-mode
 * reshuffle, sampler/UV pipeline change — shows up as per-cell drift in the
 * diff.
 *
 * Layer Y (camera looks slightly down/forward from −Z, full grid in frame):
 *   y=1.0  Core PBR BRDF       (metallic / roughness / specular axes)
 *   y=3.3  Emissive + lighting (LDR/HDR emissive, color, directIntensity)
 *   y=5.6  Transparency + edge (alpha ramp, all 5 modes, combos, geometry)
 *   y=7.9  Textures            (sprite UV, tiling/wrap/filter, texture roles)
 *
 * Layer 1 carries an extra row 4 in *front* of the main grid (z=3.5, ahead of
 * row 0's z=4.25) for shadow opt-out + Material lifecycle (deleteFrom,
 * createOrReplace). It sits in front because at the scene's fixed-noon sun,
 * shadows fall straight down — and any cell stacked under upper-layer entities
 * shares its ground shadow with three other layers, hiding the absence of a
 * single sphere's shadow. Putting these tests in front of row 0 means no
 * other entity is in the same Y-column, so each cell's potential shadow
 * lands on otherwise-empty ground and stands out unambiguously.
 *
 * Odd rows are X-staggered by half a column so back-row spheres sit in the
 * gaps between front-row spheres rather than directly behind them.
 *
 * Every cell carries a hover hint describing what it tests, so a developer
 * inspecting the running scene can identify any subject by aiming at it.
 */

const PIZZA_TEX = 'assets/Images/pizza.png'
const LAVA_TEX = 'assets/Images/lava-texture.jpg'
const SMOKE_TEX = 'assets/Images/smoke-puff.png'

// Pizza is a 4×4 sprite sheet; each frame is 1/4 of the texture per axis.
const PIZZA_FRAME_TILING = Vector2.create(0.25, 0.25)

export function main() {
  const cols = 5
  const colSpacing = 3.0
  const rowSpacing = 2.5
  const rowsPerLayer = 4

  // Layers spaced 2.3 apart vertically so spheres don't visually merge between
  // layers; the four-layer stack fills the camera frame top-to-bottom.
  const yLayers = [1.0, 3.3, 5.6, 7.9]

  // Odd rows shift +half-colSpacing in X so back-row spheres fall into the
  // gaps between front-row spheres rather than directly behind them. From the
  // camera (centered at x=8), this is what stops the center column from
  // collapsing into a single occluded stack.
  const rowStagger = colSpacing / 2
  const gridX = (col: number, row: number) =>
    8 + (col - (cols - 1) / 2) * colSpacing + (row % 2 === 1 ? rowStagger : 0)
  const gridZ = (row: number) => 8 + (row - (rowsPerLayer - 1) / 2) * rowSpacing

  // ── ground ───────────────────────────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.4, 0.4, 0.42, 1),
    roughness: 0.95,
    metallic: 0,
  })

  // ═══ LAYER 1 — Core PBR BRDF ═════════════════════════════════════════════
  const y1 = yLayers[0]

  // Row 0: dielectric (metallic=0), roughness 0 → 1.
  // Captures non-metal surface response across the full roughness range.
  for (let col = 0; col < cols; col++) {
    const r = col / (cols - 1)
    const e = mkSphere(gridX(col, 0), y1, gridZ(0))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.85, 0.3, 0.3, 1),
      metallic: 0,
      roughness: r,
    })
    attachHint(e, `Dielectric · metallic=0 · roughness=${r.toFixed(2)}`)
  }

  // Row 1: metal (metallic=1), roughness 0 → 1.
  // Mirror to Row 0 for the metallic branch of the BRDF.
  for (let col = 0; col < cols; col++) {
    const r = col / (cols - 1)
    const e = mkSphere(gridX(col, 1), y1, gridZ(1))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.85, 0.85, 0.9, 1),
      metallic: 1,
      roughness: r,
    })
    attachHint(e, `Metal · metallic=1 · roughness=${r.toFixed(2)}`)
  }

  // Row 2: metallic 0 → 1 at fixed mid-low roughness.
  // Catches BRDF-transition regressions that pure-dielectric / pure-metal rows
  // miss (e.g. partial-metal lerp bugs around 0.5).
  for (let col = 0; col < cols; col++) {
    const m = col / (cols - 1)
    const e = mkSphere(gridX(col, 2), y1, gridZ(2))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.5, 0.4, 0.85, 1),
      metallic: m,
      roughness: 0.3,
    })
    attachHint(e, `Metallic axis · metallic=${m.toFixed(2)} · roughness=0.30`)
  }

  // Row 3: specularIntensity 0 → 1 (dielectric, fixed roughness).
  // Isolates the dielectric F0 control — invisible at metallic=1, so this row
  // is intentionally non-metal.
  for (let col = 0; col < cols; col++) {
    const s = col / (cols - 1)
    const e = mkSphere(gridX(col, 3), y1, gridZ(3))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.4, 0.7, 0.4, 1),
      metallic: 0,
      roughness: 0.3,
      specularIntensity: s,
    })
    attachHint(e, `Specular intensity · specularIntensity=${s.toFixed(2)} · dielectric`)
  }

  // Row 4 (Layer 1 only, FRONT row at z=3.5): shadow opt-out + Material
  // lifecycle. Lives in front of the main grid (row 0 is at z=4.25) so:
  //   1. The ground-cast shadow under each subject is in frame — closer to
  //      the camera, no front-row spheres occluding the line of sight to the
  //      ground.
  //   2. No upper-layer entity is in the same Y-column (layers 2-4 only have
  //      rows at z={4.25, 6.75, 9.25, 11.75}). At the scene's fixed-noon sun,
  //      shadows fall straight down, so a cell at z=3.5 has clean ground
  //      below it — its potential shadow has no co-located shadows from
  //      stacked upper-layer spheres masking it.
  // Cols 0 & 1 are adjacent shadow control + test for direct visual diff;
  // col 2 is empty to break the shadow tests apart from the lifecycle tests
  // in cols 3 & 4.
  // col 0: castShadows=true control     (vivid orange, shadow visible)
  // col 1: castShadows=false test       (same vivid orange, NO shadow)
  // col 3: material added → deleteFrom  (vivid magenta gone, default render)
  // col 4: material added → replaced    (vivid red replaced w/ emissive cyan)
  const lifecycleZ = 3.5
  {
    const shadowControl = mkSphere(gridX(0, 4), y1, lifecycleZ)
    Material.setPbrMaterial(shadowControl, {
      albedoColor: Color4.create(0.95, 0.5, 0.15, 1),
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(shadowControl, 'castShadows control · castShadows=true (shadow visible)')

    const shadowOff = mkSphere(gridX(1, 4), y1, lifecycleZ)
    Material.setPbrMaterial(shadowOff, {
      albedoColor: Color4.create(0.95, 0.5, 0.15, 1),
      castShadows: false,
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(shadowOff, 'castShadows opt-out · castShadows=false (no shadow under it)')

    // Apply a vivid magenta material, then delete the Material component.
    // The entity should fall back to the engine's default appearance (no
    // Material). If deleteFrom regresses (e.g., is treated as a no-op), the
    // sphere will still render magenta — easy to spot in the diff.
    const materialDeleted = mkSphere(gridX(3, 4), y1, lifecycleZ)
    Material.setPbrMaterial(materialDeleted, {
      albedoColor: Color4.create(1, 0, 0.8, 1),
      emissiveColor: Color3.create(1, 0, 0.8),
      emissiveIntensity: 2,
      metallic: 0,
      roughness: 0.4,
    })
    Material.deleteFrom(materialDeleted)
    attachHint(materialDeleted, 'Material lifecycle · added (magenta) then deleteFrom — renders default')

    // Apply a vivid red opaque material, then call createOrReplace with a
    // distinctly different emissive cyan PBR. The cell should render as the
    // *second* material; if createOrReplace regresses to a merge or no-op,
    // the diff will show red instead of cyan.
    const materialReplaced = mkSphere(gridX(4, 4), y1, lifecycleZ)
    Material.setPbrMaterial(materialReplaced, {
      albedoColor: Color4.create(1, 0.05, 0.05, 1),
      metallic: 0,
      roughness: 0.5,
    })
    Material.createOrReplace(materialReplaced, {
      material: {
        $case: 'pbr',
        pbr: {
          albedoColor: Color4.create(0.05, 0.05, 0.05, 1),
          emissiveColor: Color3.create(0, 1, 1),
          emissiveIntensity: 3,
          metallic: 0,
          roughness: 0.4,
        },
      },
    })
    attachHint(
      materialReplaced,
      'Material lifecycle · added (red) then createOrReplace (emissive cyan)',
    )
  }

  // ═══ LAYER 2 — Emissive + Color + Direct Lighting ════════════════════════
  const y2 = yLayers[1]

  // Row 0: emissive intensity LDR ramp (0, 0.5, 1, 2, 4).
  // First cell (intensity=0) is the no-emissive baseline — should look black.
  const ldrIntensities = [0, 0.5, 1, 2, 4]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 0), y2, gridZ(0))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.05, 0.05, 0.05, 1),
      emissiveColor: Color3.create(0.95, 0.4, 0.1),
      emissiveIntensity: ldrIntensities[col],
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(e, `Emissive LDR · intensity=${ldrIntensities[col]}`)
  }

  // Row 1: emissive intensity HDR ramp (4 → 64).
  // Drives bloom / tonemap / exposure pipelines. Verifies HDR emissive doesn't
  // saturate to flat white past some clamp.
  const hdrIntensities = [4, 8, 16, 32, 64]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 1), y2, gridZ(1))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.02, 0.02, 0.02, 1),
      emissiveColor: Color3.create(0.4, 0.7, 1),
      emissiveIntensity: hdrIntensities[col],
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(e, `Emissive HDR · intensity=${hdrIntensities[col]}`)
  }

  // Row 2: emissive color spectrum at fixed intensity — pure R, G, B, W, magenta.
  // Single-channel cells catch per-channel saturation/clipping bugs that
  // grayscale ramps wouldn't.
  const emissiveColors = [
    Color3.create(1, 0, 0),
    Color3.create(0, 1, 0),
    Color3.create(0, 0, 1),
    Color3.create(1, 1, 1),
    Color3.create(1, 0, 1),
  ]
  const emissiveColorNames = ['red', 'green', 'blue', 'white', 'magenta']
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 2), y2, gridZ(2))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.05, 0.05, 0.05, 1),
      emissiveColor: emissiveColors[col],
      emissiveIntensity: 2,
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(e, `Emissive color · ${emissiveColorNames[col]} · intensity=2`)
  }

  // Row 3: directIntensity 0 → 4 — multiplier on direct-light contribution.
  // First cell (=0) should show only ambient/IBL response; last cell over-lit.
  const directIntensities = [0, 0.5, 1, 2, 4]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 3), y2, gridZ(3))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.85, 0.6, 0.3, 1),
      metallic: 0,
      roughness: 0.4,
      directIntensity: directIntensities[col],
    })
    attachHint(e, `Direct light · directIntensity=${directIntensities[col]}`)
  }

  // ═══ LAYER 3 — Transparency + Combos + Geometry ══════════════════════════
  const y3 = yLayers[2]

  // Row 0: alpha ramp via alpha-blend (0.1, 0.3, 0.5, 0.7, 0.9).
  // Verifies smooth-transparency interpolation across the alpha range; avoids
  // the trivial endpoints (0=invisible, 1=opaque indistinguishable).
  const alphas = [0.1, 0.3, 0.5, 0.7, 0.9]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 0), y3, gridZ(0))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.2, 0.7, 0.95, alphas[col]),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
      metallic: 0,
      roughness: 0.3,
    })
    attachHint(e, `Alpha-blend · alpha=${alphas[col]}`)
  }

  // Row 1: every transparency mode side-by-side with the same source color
  // (alpha 0.7) and alphaTest 0.5. Catches pipeline divergence between modes
  // and any change to the AUTO heuristic.
  // alpha=0.7 with alphaTest=0.5 keeps ALPHA_TEST visible (alpha > threshold)
  // while still showing partial transparency under blend modes.
  const transparencyModes = [
    MaterialTransparencyMode.MTM_OPAQUE,
    MaterialTransparencyMode.MTM_AUTO,
    MaterialTransparencyMode.MTM_ALPHA_TEST,
    MaterialTransparencyMode.MTM_ALPHA_BLEND,
    MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND,
  ]
  const transparencyModeNames = [
    'OPAQUE',
    'AUTO',
    'ALPHA_TEST',
    'ALPHA_BLEND',
    'ALPHA_TEST_AND_ALPHA_BLEND',
  ]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 1), y3, gridZ(1))
    Material.setPbrMaterial(e, {
      albedoColor: Color4.create(0.95, 0.4, 0.85, 0.7),
      transparencyMode: transparencyModes[col],
      alphaTest: 0.5,
      metallic: 0,
      roughness: 0.4,
    })
    attachHint(e, `Transparency mode · ${transparencyModeNames[col]} · alpha=0.7 alphaTest=0.5`)
  }

  // Row 2: combo cases — feature interactions that are easy to miss when each
  // axis is tested in isolation.
  // col 0: emissive metal           (does emissive add over metallic specular?)
  // col 1: alpha-blend metal        (transparent reflective surface)
  // col 2: emissive + alpha-blend   (glow that fades through itself)
  // col 3: Basic / unlit            (skips lighting entirely — parity check)
  // col 4: Basic + low alpha        (alpha pipeline on Basic material)
  {
    const emissiveMetal = mkSphere(gridX(0, 2), y3, gridZ(2))
    Material.setPbrMaterial(emissiveMetal, {
      albedoColor: Color4.create(0.85, 0.7, 0.3, 1),
      emissiveColor: Color3.create(1, 0.3, 0),
      emissiveIntensity: 2,
      metallic: 1,
      roughness: 0.2,
    })
    attachHint(emissiveMetal, 'Combo · emissive metal (emissive + metallic=1)')

    const transparentMetal = mkSphere(gridX(1, 2), y3, gridZ(2))
    Material.setPbrMaterial(transparentMetal, {
      albedoColor: Color4.create(0.6, 0.85, 0.85, 0.5),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
      metallic: 1,
      roughness: 0.15,
    })
    attachHint(transparentMetal, 'Combo · transparent metal (alpha-blend + metallic=1)')

    const emissiveTransparent = mkSphere(gridX(2, 2), y3, gridZ(2))
    Material.setPbrMaterial(emissiveTransparent, {
      albedoColor: Color4.create(0.05, 0.05, 0.05, 0.5),
      emissiveColor: Color3.create(0.2, 1, 0.4),
      emissiveIntensity: 3,
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(emissiveTransparent, 'Combo · emissive + alpha-blend (glow that fades)')

    const unlitOpaque = mkSphere(gridX(3, 2), y3, gridZ(2))
    Material.setBasicMaterial(unlitOpaque, {
      diffuseColor: Color4.create(0.95, 0.85, 0.2, 1),
    })
    attachHint(unlitOpaque, 'Combo · Basic / unlit (no lighting interaction)')

    const unlitTransparent = mkSphere(gridX(4, 2), y3, gridZ(2))
    Material.setBasicMaterial(unlitTransparent, {
      diffuseColor: Color4.create(0.95, 0.4, 0.85, 0.4),
    })
    attachHint(unlitTransparent, 'Combo · Basic + low alpha (alpha on unlit)')
  }

  // Row 3: identical PBR material applied to different mesh primitives.
  // A material change that breaks one geometry but not another (e.g. UV
  // assumptions, normal handling, two-sided fallback for Plane) lights up here.
  // sphere · box · cylinder · plane (slight Y-tilt) · cone (cylinder w/ top=0).
  {
    const matApply = (e: Entity) =>
      Material.setPbrMaterial(e, {
        albedoColor: Color4.create(0.4, 0.55, 0.85, 1),
        metallic: 1,
        roughness: 0.25,
      })

    const sphere = mkSphere(gridX(0, 3), y3, gridZ(3))
    matApply(sphere)
    attachHint(sphere, 'Geometry parity · sphere (same metal material)')

    const box = engine.addEntity()
    Transform.create(box, {
      position: Vector3.create(gridX(1, 3), y3, gridZ(3)),
      scale: Vector3.create(0.7, 0.7, 0.7),
    })
    MeshRenderer.setBox(box)
    MeshCollider.setBox(box)
    matApply(box)
    attachHint(box, 'Geometry parity · box (same metal material)')

    const cyl = engine.addEntity()
    Transform.create(cyl, {
      position: Vector3.create(gridX(2, 3), y3, gridZ(3)),
      scale: Vector3.create(0.7, 0.7, 0.7),
    })
    MeshRenderer.setCylinder(cyl)
    MeshCollider.setCylinder(cyl)
    matApply(cyl)
    attachHint(cyl, 'Geometry parity · cylinder (same metal material)')

    // Slight Y-tilt on the plane so it reads as a quad, not a flat rectangle,
    // and so any back-face-culling regression would surface the silhouette.
    const plane = engine.addEntity()
    Transform.create(plane, {
      position: Vector3.create(gridX(3, 3), y3, gridZ(3)),
      scale: Vector3.create(0.7, 0.7, 0.7),
      rotation: Quaternion.fromEulerDegrees(0, 30, 0),
    })
    MeshRenderer.setPlane(plane)
    MeshCollider.setPlane(plane)
    matApply(plane)
    attachHint(plane, 'Geometry parity · plane (same metal material)')

    const cone = engine.addEntity()
    Transform.create(cone, {
      position: Vector3.create(gridX(4, 3), y3, gridZ(3)),
      scale: Vector3.create(0.7, 0.7, 0.7),
    })
    MeshRenderer.setCylinder(cone, 0.5, 0)
    MeshCollider.setCylinder(cone, 0.5, 0)
    matApply(cone)
    attachHint(cone, 'Geometry parity · cone (cylinder with top radius=0)')
  }

  // Row 4 (Layer 3 only, FRONT row at z=3.5): fully-transparent corner case.
  // The alpha ramp in Row 0 excludes alpha=0 as "trivially invisible", but the
  // alpha=0 boundary has its own failure modes — premultiplied-alpha math,
  // blend equations that divide by alpha, and renderer early-outs that can
  // accidentally fire on alpha>0 cells. Visual contract: the sphere must be
  // invisible against clean ground. A regression that renders alpha=0 (full
  // opacity, faint tint, edge silhouette) shows a sphere where there was empty
  // ground. Sits above empty col 2 of Layer 1's lifecycle row so no other
  // entity is in the same Y-column. castShadows=false because a fully-
  // transparent surface should not cast — and a phantom shadow would otherwise
  // muddy the diff.
  {
    const fullyTransparent = mkSphere(gridX(2, 4), y3, 3.5)
    Material.setPbrMaterial(fullyTransparent, {
      albedoColor: Color4.create(0.2, 0.7, 0.95, 0),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
      castShadows: false,
      metallic: 0,
      roughness: 0.3,
    })
    attachHint(
      fullyTransparent,
      'Alpha-blend · alpha=0 (fully transparent, should be invisible)',
    )
  }

  // ═══ LAYER 4 — Textures ══════════════════════════════════════════════════
  // Tests the sampler / UV pipeline: sprite-frame UV cropping, tiling, wrap
  // mode, filter mode, and the different roles a texture can play in a
  // material (albedo, emissive map, bump map, alpha source).
  const y4 = yLayers[3]

  // Row 0: pizza sprite — single-frame UV mapping, applied to CUBES.
  // Pizza is a flat sprite sheet; mapping it to a cube (each face renders one
  // frame) is a much truer test of the offset+tiling formula than a sphere
  // (which distorts the sprite around the poles). With tiling=(0.25, 0.25),
  // each face's UV [0,1]² is remapped to [offset, offset+0.25]² — one frame.
  // Cubes carry a slight Y-tilt so the 3D shape reads (otherwise the camera
  // sees only a flat front face).
  const pizzaFrameOffsets = [
    Vector2.create(0.0, 0.0),    // bottom-left frame
    Vector2.create(0.25, 0.25),  // one cell in
    Vector2.create(0.5, 0.5),    // center
    Vector2.create(0.75, 0.5),   // mid-right band
    Vector2.create(0.75, 0.75),  // top-right frame
  ]
  for (let col = 0; col < cols; col++) {
    const off = pizzaFrameOffsets[col]
    const e = mkBox(gridX(col, 0), y4, gridZ(0), 20)
    Material.setPbrMaterial(e, {
      texture: Material.Texture.Common({
        src: PIZZA_TEX,
        offset: off,
        tiling: PIZZA_FRAME_TILING,
        wrapMode: TextureWrapMode.TWM_CLAMP,
      }),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
      alphaTest: 0.5,
      metallic: 0,
      roughness: 0.6,
    })
    attachHint(
      e,
      `Pizza sprite frame · offset=(${off.x}, ${off.y}) · tiling=(0.25, 0.25)`,
    )
  }

  // Row 1: lava texture — tiling × wrap mode.
  // tiling > 1 takes UV outside [0,1], which exposes the wrapMode behavior:
  // REPEAT tiles, CLAMP stretches edges, MIRROR ping-pongs.
  const tilingCases: Array<{
    tiling: Vector2
    wrap: TextureWrapMode
    offset?: Vector2
    label: string
  }> = [
      {
        tiling: Vector2.create(1, 1),
        wrap: TextureWrapMode.TWM_CLAMP,
        label: '1×1 CLAMP (full texture, baseline)',
      },
      {
        tiling: Vector2.create(2, 2),
        wrap: TextureWrapMode.TWM_REPEAT,
        label: '2×2 REPEAT (4 tiles)',
      },
      {
        tiling: Vector2.create(4, 4),
        wrap: TextureWrapMode.TWM_REPEAT,
        label: '4×4 REPEAT (16 tiles, fine-grain)',
      },
      {
        tiling: Vector2.create(2, 2),
        wrap: TextureWrapMode.TWM_MIRROR,
        label: '2×2 MIRROR (mirrored seams)',
      },
      {
        tiling: Vector2.create(2, 2),
        wrap: TextureWrapMode.TWM_CLAMP,
        offset: Vector2.create(0.3, 0.3),
        label: '2×2 CLAMP w/ offset (edges stretch past 1)',
      },
    ]
  for (let col = 0; col < cols; col++) {
    const e = mkSphere(gridX(col, 1), y4, gridZ(1))
    const tc = tilingCases[col]
    Material.setPbrMaterial(e, {
      texture: Material.Texture.Common({
        src: LAVA_TEX,
        tiling: tc.tiling,
        wrapMode: tc.wrap,
        offset: tc.offset,
      }),
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(e, `Lava ${tc.label}`)
  }

  // Row 2: filter mode + smooth-alpha texture.
  // Filter mode changes how the sampler interpolates between texels — most
  // visible on a pixel-art sprite where individual texels project to large
  // screen areas (POINT) or are smoothly blended (BILINEAR/TRILINEAR).
  // Pizza cells are CUBES for the same flat-mapping reason as Row 0.
  const filterModes = [
    TextureFilterMode.TFM_POINT,
    TextureFilterMode.TFM_BILINEAR,
    TextureFilterMode.TFM_TRILINEAR,
  ]
  const filterModeNames = ['POINT (pixelated)', 'BILINEAR (smooth)', 'TRILINEAR (mip-filtered)']
  for (let col = 0; col < 3; col++) {
    const e = mkBox(gridX(col, 2), y4, gridZ(2), 20)
    Material.setPbrMaterial(e, {
      texture: Material.Texture.Common({
        src: PIZZA_TEX,
        offset: Vector2.create(0.25, 0.25),
        tiling: PIZZA_FRAME_TILING,
        filterMode: filterModes[col],
        wrapMode: TextureWrapMode.TWM_CLAMP,
      }),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
      alphaTest: 0.5,
      metallic: 0,
      roughness: 0.6,
    })
    attachHint(e, `Pizza filter · ${filterModeNames[col]}`)
  }
  {
    const smokePbr = mkSphere(gridX(3, 2), y4, gridZ(2))
    Material.setPbrMaterial(smokePbr, {
      texture: Material.Texture.Common({ src: SMOKE_TEX }),
      transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
      albedoColor: Color4.create(1, 1, 1, 1),
      metallic: 0,
      roughness: 0.7,
    })
    attachHint(smokePbr, 'Smoke alpha-blend · PBR · soft alpha gradient')

    const smokeBasic = mkSphere(gridX(4, 2), y4, gridZ(2))
    Material.setBasicMaterial(smokeBasic, {
      texture: Material.Texture.Common({ src: SMOKE_TEX }),
      diffuseColor: Color4.create(0.4, 0.7, 1, 1),
    })
    attachHint(smokeBasic, 'Smoke alpha · Basic + diffuseColor tint')
  }

  // Row 3: texture roles — same lava source plugged into different material
  // slots. Verifies each sampler binding (albedo / emissive / bump) targets
  // the right shader uniform.
  {
    const albedoOnly = mkSphere(gridX(0, 3), y4, gridZ(3))
    Material.setPbrMaterial(albedoOnly, {
      texture: Material.Texture.Common({ src: LAVA_TEX }),
      metallic: 0,
      roughness: 0.6,
    })
    attachHint(albedoOnly, 'Texture role · albedo only')

    const albedoPlusEmissive = mkSphere(gridX(1, 3), y4, gridZ(3))
    Material.setPbrMaterial(albedoPlusEmissive, {
      texture: Material.Texture.Common({ src: LAVA_TEX }),
      emissiveTexture: Material.Texture.Common({ src: LAVA_TEX }),
      emissiveColor: Color3.create(1, 0.6, 0.2),
      emissiveIntensity: 3,
      metallic: 0,
      roughness: 0.5,
    })
    attachHint(albedoPlusEmissive, 'Texture role · albedo + emissive map (HDR glow)')

    const bumpOnly = mkSphere(gridX(2, 3), y4, gridZ(3))
    Material.setPbrMaterial(bumpOnly, {
      albedoColor: Color4.create(0.85, 0.85, 0.85, 1),
      bumpTexture: Material.Texture.Common({ src: LAVA_TEX }),
      metallic: 0,
      roughness: 0.4,
    })
    attachHint(bumpOnly, 'Texture role · bumpTexture only (normal-map effect)')

    const lavaBasic = mkSphere(gridX(3, 3), y4, gridZ(3))
    Material.setBasicMaterial(lavaBasic, {
      texture: Material.Texture.Common({ src: LAVA_TEX }),
    })
    attachHint(lavaBasic, 'Texture role · Basic / unlit (no lighting)')

    const lavaTiledMetal = mkSphere(gridX(4, 3), y4, gridZ(3))
    Material.setPbrMaterial(lavaTiledMetal, {
      texture: Material.Texture.Common({
        src: LAVA_TEX,
        tiling: Vector2.create(4, 4),
        wrapMode: TextureWrapMode.TWM_REPEAT,
      }),
      metallic: 0.7,
      roughness: 0.25,
    })
    attachHint(lavaTiledMetal, 'Texture role · 4×4 REPEAT + metallic=0.7')
  }

  // ── lighting ─────────────────────────────────────────────────────────────
  // Key (warm point) + fill (cool spot). Range and intensity tuned so the top
  // layer at y=7.9 still receives meaningful direct light.
  const keyLight = engine.addEntity()
  Transform.create(keyLight, { position: Vector3.create(8, 11, 4) })
  LightSource.create(keyLight, {
    color: Color3.create(1, 0.97, 0.9),
    intensity: 130000,
    range: 30,
    active: true,
    shadow: false,
    type: LightSource.Type.Point({}),
  })

  const fillLight = engine.addEntity()
  Transform.create(fillLight, {
    position: Vector3.create(8, 10, 12),
    rotation: Quaternion.fromEulerDegrees(40, 0, 0),
  })
  LightSource.create(fillLight, {
    color: Color3.create(0.6, 0.75, 1),
    intensity: 80000,
    range: 26,
    active: true,
    shadow: false,
    type: LightSource.Type.Spot({ innerAngle: 25, outerAngle: 70 }),
  })

  // Frame the geometric center of the 4-layer grid. The camera is pulled
  // further back and up to fit the wider (12-unit) and taller (7-unit) grid
  // without crowding. lookAt sits between layers 2 and 3 so vertical framing
  // is balanced.
  setupVisualTest({
    lookAtPos: Vector3.create(8, 4, 8),
    cameraPos: Vector3.create(8, 4.5, -3.5),
  })
}

function mkSphere(x: number, y: number, z: number): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, y, z),
    scale: Vector3.create(0.7, 0.7, 0.7),
  })
  MeshRenderer.setSphere(e)
  MeshCollider.setSphere(e)
  return e
}

// rotationY tilts the cube around Y so the camera sees two faces (and so the
// shape reads as 3D rather than a flat square). Pizza-sprite cells use this
// to keep the texture readable while still showing the cube silhouette.
function mkBox(x: number, y: number, z: number, rotationY = 0): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, y, z),
    scale: Vector3.create(0.7, 0.7, 0.7),
    rotation: Quaternion.fromEulerDegrees(0, rotationY, 0),
  })
  MeshRenderer.setBox(e)
  MeshCollider.setBox(e)
  return e
}

// Hover hint: display `text` when the cursor is on the entity. The empty
// click handler is intentional — the goal is the tooltip-on-hover, not a
// click action. `maxDistance` is bumped well past the SDK default (~10) so
// that back-row top-layer spheres ~18 units from the fixed camera still
// register the hover; the player can't move closer because the avatar is
// hidden and the camera is locked by the visual-test setup.
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

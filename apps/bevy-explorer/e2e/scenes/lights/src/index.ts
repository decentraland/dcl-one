import {
  Entity,
  InputAction,
  LightSource,
  Material,
  MeshCollider,
  MeshRenderer,
  SkyboxTime,
  TextShape,
  Transform,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'



/// NOTE: IMPORTANT KNOWN ISSUE: CURRENTLY THE ENGINE ONLY SHOWS 4 LIGHTS MAX, WHEN A BUG IS FIXED WE SHOULD SEE ALL 5 LIGHTS AT THE SAME TIME





/**
 * lights — dynamic LightSource property matrix.
 *
 * Visual contract: 5 "light booths" arranged left-to-right along X. Each booth
 * is a white back-wall + white floor patch acting as a projection surface. The
 * light is placed at the mouth of the booth pointing at the wall so its
 * footprint is isolated and the screenshot reads cleanly as a column.
 *
 * Skybox is locked to midnight (fixedTime = 0) so ambient light is near-zero
 * and each dynamic light's contribution is unmistakable.
 *
 * Scene size = 5 parcels (an L-shape: front row 0,0/1,0/2,0 plus 0,1/1,1),
 * but the booths all live in the front-left parcel (0,0). Decentraland's
 * dynamic-light budget scales with parcel count, so a 1-parcel scene only
 * renders one shadow-casting light at a time. The extra 4 parcels are purely
 * budget headroom for the 5 lights below — one parcel of budget per light —
 * and the layout itself is unchanged from a single-parcel scene.
 *
 * Light matrix — one row per axis under test:
 *
 *  Col  X   Light type  Color        Intensity  Range   InnerAngle  OuterAngle  Mask?
 *  ─────────────────────────────────────────────────────────────────────────────────
 *   0   2   point       white        16000      auto    —           —           no
 *   1   5   spot        red          8000       4 m     25°         50°  TILTED  no
 *   2   8   spot        green        8000       8 m     30°         60°  TILTED  no
 *   3  11   spot        blue         20000      4 m     15°         30°          no
 *   4  14   spot        yellow       6000       4 m     30°         50°          YES (light-mask)
 *
 * Axes covered per column (capitals = primary axis for that column):
 *   col 0: LIGHT TYPE (point vs spot)
 *   col 1+2: CONE OVERLAP — two spots tilted toward each other with wide enough
 *           apertures that their cones partially overlap on the booth surfaces.
 *           Red aims at (6.0, 0, 7.5); green aims at (7.0, 0, 7.5); the cones
 *           cross in the gap and spill onto each other's booth (additive blend).
 *   col 3: HIGH INTENSITY (20000 cd), blue color
 *   col 4: MASK TEXTURE (shadowMaskTexture), yellow
 *
 * Every entity carries a hover hint (PointerEvents) describing the case it
 * covers. maxDistance is bumped well past the SDK default so the virtual
 * camera resolves hovers across the full parcel depth.
 */
export function main() {
  // ── midnight skybox ──────────────────────────────────────────────────────
  // fixedTime=0 → 00:00 hs → near-total darkness, so every dynamic light's
  // contribution is unambiguous in the screenshot.
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime: 0 })

  // No shared ground — a single ground mesh spanning the full parcel would be
  // touched by every light's range at once, hitting Unity URP's per-object
  // additional-light cap (4) and forcing one light to be dropped per frame
  // (symptom: 4-of-5 lights visible, set rotates with camera). Each booth has
  // its own floor patch (see makeBooth) so the lights still have surfaces to
  // project onto, and the gaps between booths show through to the void —
  // fine, since the scene is at midnight.

  // ── light booths ─────────────────────────────────────────────────────────
  // col 0 — point light, white
  makeBooth({
    x: 2,
    label: 'point · white',
    wallColor: Color4.create(0.9, 0.9, 0.9, 1),
  })
  // Explicit short range. Without it, intensity=16000 auto-computes
  // range=pow(16000, 0.25)≈11.3 m, which spills into every neighboring booth
  // and pushes per-surface light count past Unity URP's per-object cap of 4
  // (symptom: 4-of-5 lights visible at a time, set rotates with camera).
  LightSource.create(makeLightEntity(2, 2.5, 7.5), {
    color: Color3.create(1, 1, 1),
    intensity: 16000,
    range: 2.5,
    shadow: true,
    type: LightSource.Type.Point({}),
  })
  makeLabel(3, 'point · white\n16000 cd', 'point · white · 16000 cd · range=2.5 m')

  // col 1 — spot · red · tilted toward col 2, wide enough to partially overlap
  // with col 2's cone. Position unchanged; only rotation + aperture change.
  makeBooth({
    x: 5,
    label: 'spot · red · overlap',
    wallColor: Color4.create(0.9, 0.88, 0.88, 1),
  })
  LightSource.create(
    // Aim from (5, 2.5, 7.5) toward (6, 0, 7.5) — tilts ~22° off straight-down
    // in the +X direction so the cone reaches into col 2's booth.
    makeLightEntity(
      5,
      2.5,
      7.5,
      Quaternion.fromToRotation(Vector3.Forward(), Vector3.create(1, -2.5, 0)),
    ),
    {
      color: Color3.create(1, 0.1, 0.1),
      intensity: 8000,
      range: 4,
      shadow: true,
      type: LightSource.Type.Spot({ innerAngle: 25, outerAngle: 50 }),
    },
  )
  makeLabel(5, 'spot · red\ntilt +X · 50°', 'spot · red · tilted +X · outerAngle=50° · range=4 m · 8000 cd')

  // col 2 — spot · green · tilted toward col 1, wide aperture so its cone
  // partially overlaps col 1's cone. Position unchanged.
  makeBooth({
    x: 8,
    label: 'spot · green · overlap',
    wallColor: Color4.create(0.88, 0.9, 0.88, 1),
  })
  LightSource.create(
    // Aim from (8, 2.5, 7.5) toward (7, 0, 7.5) — tilts ~22° off straight-down
    // in the −X direction so the cone reaches into col 1's booth.
    makeLightEntity(
      8,
      2.5,
      7.5,
      Quaternion.fromToRotation(Vector3.Forward(), Vector3.create(-1, -2.5, 0)),
    ),
    {
      color: Color3.create(0.1, 1, 0.2),
      intensity: 8000,
      range: 8,
      shadow: true,
      type: LightSource.Type.Spot({ innerAngle: 30, outerAngle: 60 }),
    },
  )
  makeLabel(8, 'spot · green\ntilt −X · 60°', 'spot · green · tilted −X · outerAngle=60° · range=8 m · 8000 cd')

  // col 3 — spot · blue · high intensity (20000 cd)
  makeBooth({
    x: 11,
    label: 'spot · blue · high intensity',
    wallColor: Color4.create(0.88, 0.88, 0.9, 1),
  })
  LightSource.create(makeLightEntity(11, 2.5, 7.5, Quaternion.fromEulerDegrees(90, 0, 0)), {
    color: Color3.create(0.25, 0.55, 1),
    intensity: 20000,
    range: 4,
    shadow: true,
    type: LightSource.Type.Spot({ innerAngle: 15, outerAngle: 30 }),
  })
  makeLabel(11, 'spot · blue\n20000 cd', 'spot · blue · HIGH intensity=20000 cd · range=4 m · outerAngle=30°')

  // col 4 — spot · yellow · mask texture (shadowMaskTexture)
  makeBooth({
    x: 14,
    label: 'spot · yellow · masked',
    wallColor: Color4.create(0.9, 0.9, 0.85, 1),
  })
  LightSource.create(makeLightEntity(14, 2.5, 7.5, Quaternion.fromEulerDegrees(90, 0, 0)), {
    color: Color3.create(1, 0.9, 0.2),
    intensity: 6000,
    range: 4,
    shadow: true,
    shadowMaskTexture: {
      tex: {
        $case: 'texture',
        texture: { src: 'assets/Images/light-mask.png' },
      },
    },
    type: LightSource.Type.Spot({ innerAngle: 30, outerAngle: 50 }),
  })
  makeLabel(13, 'spot · yellow\nmasked', 'spot · yellow · MASK texture · 4000 cd · range=4 m · outerAngle=30°')

  // ── camera ───────────────────────────────────────────────────────────────
  // Pulled back to z=−2 so all five booths fit in FOV. Y=3 centers the
  // 0-to-6 m vertical range of the booths.
  setupVisualTest({
    lookAtPos: Vector3.create(8, 0, 8),
    cameraPos: Vector3.create(8, 3, 0),
    // 5-parcel L-shape — widen the AvatarModifierArea to cover the bounding
    // box (x=0..48, z=0..32). The box overlaps the unowned (2,1) parcel,
    // but AvatarModifierArea is clipped to the scene's parcels at runtime.
    hideAreaCenter: Vector3.create(24, 8, 16),
    hideAreaSize: Vector3.create(48, 16, 32),
  })
}

// ── helpers ───────────────────────────────────────────────────────────────

const HINT_MAX_DISTANCE = 100

// Flip to true to wire up hover tooltips while debugging the scene by hand.
// Kept off for visual-test runs so the cursor never paints a hover outline.
const DEBUG_HOVER_HINTS = false

function attachHint(entity: Entity, text: string): void {
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

/** Create an invisible entity to host a LightSource component. */
function makeLightEntity(
  x: number,
  y: number,
  z: number,
  rotation?: Quaternion,
): Entity {
  const e = engine.addEntity()
  Transform.create(e, {
    position: Vector3.create(x, y, z),
    rotation: rotation ?? Quaternion.Identity(),
  })
  return e
}

interface BoothOpts {
  /** Center X of this booth column */
  x: number
  label: string
  wallColor: Color4
}

/**
 * Build a "light booth": a back wall and a floor patch that acts as a
 * projection surface. The wall sits at z=8, the floor patch between z=5..8.
 * Width is 3 m so adjacent booths share an edge (columns are 3 m apart).
 */
function makeBooth(opts: BoothOpts): void {
  const { x, wallColor } = opts

  // Back wall: 3 m wide, 5 m tall, at z=8
  const wall = engine.addEntity()
  Transform.create(wall, {
    position: Vector3.create(x, 2.5, 8),
    scale: Vector3.create(3, 5, 0.05),
  })
  MeshRenderer.setBox(wall)
  MeshCollider.setBox(wall)
  Material.setPbrMaterial(wall, {
    albedoColor: wallColor,
    roughness: 1,
    metallic: 0,
    emissiveColor: Color3.create(0, 0, 0),
    emissiveIntensity: 0,
  })
  attachHint(wall, `Back wall · ${opts.label}`)

  // Floor patch: 3 m wide, 3 m deep (z 5→8), sits flush with ground
  const floor = engine.addEntity()
  Transform.create(floor, {
    position: Vector3.create(x, 0.03, 6.5),
    scale: Vector3.create(3, 0.02, 3),
  })
  MeshRenderer.setBox(floor)
  MeshCollider.setBox(floor)
  Material.setPbrMaterial(floor, {
    albedoColor: wallColor,
    roughness: 1,
    metallic: 0,
    emissiveColor: Color3.create(0, 0, 0),
    emissiveIntensity: 0,
  })
  attachHint(floor, `Floor patch · ${opts.label}`)
}

/**
 * Place a small TextShape label below the booth at the front of the parcel,
 * describing the light variation under test. Faces −Z (default DCL text
 * orientation), which is toward the camera at z=−2.
 */
function makeLabel(x: number, text: string, hint: string): void {
  const e = engine.addEntity()
  Transform.create(e, { position: Vector3.create(x, 0.4, 4.8) })
  TextShape.create(e, {
    text,
    fontSize: 1,
    textColor: Color4.create(0.9, 0.9, 0.9, 1),
    textWrapping: false,
  })
  MeshCollider.setPlane(e)
  attachHint(e, hint)
}

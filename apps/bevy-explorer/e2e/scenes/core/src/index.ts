import {
  Billboard,
  BillboardMode,
  Entity,
  InputAction,
  LightSource,
  Material,
  MeshCollider,
  MeshRenderer,
  Transform,
  VisibilityComponent,
  engine,
  pointerEventsSystem,
} from '@dcl/sdk/ecs'
import { Color3, Color4, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * core — primitives + transforms + lighting + per-component edge cases.
 *
 * Visual contract: any regression in mesh primitive rendering, transform-tree
 * composition, point/spot lighting, or PBR base shading shows up here first.
 * Subjects are stacked at multiple Y heights so the camera frame holds:
 *   - y≈0    cube without a Transform (renders at the origin)
 *   - y=1    primitives row WITH PBR material
 *   - y=2.5  hierarchical transform group (parent rotation × child translation)
 *   - y=3    primitives row WITHOUT a Material (engine default appearance)
 *   - y=5    Billboard + VisibilityComponent edge cases
 *   - y=5.5  parent/child with non-default scale, position, rotation on each
 *
 * Every entity carries a hover hint so a developer aiming at it can read off
 * the test case it covers; max distance is bumped well past the SDK default
 * so the locked virtual camera at z=-11 still resolves all hovers.
 */
export function main() {
  // ── ground plane ─────────────────────────────────────────────────────────
  const ground = engine.addEntity()
  Transform.create(ground, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(ground)
  Material.setPbrMaterial(ground, {
    albedoColor: Color4.create(0.4, 0.4, 0.42, 1),
    roughness: 0.9,
    metallic: 0,
  })

  // ── primitives row at y=1 (with PBR material) ────────────────────────────
  const yRow = 1
  const zRow = 8

  const box = engine.addEntity()
  Transform.create(box, { position: Vector3.create(4, yRow, zRow) })
  MeshRenderer.setBox(box)
  MeshCollider.setBox(box)
  Material.setPbrMaterial(box, {
    albedoColor: Color4.create(0.85, 0.2, 0.2, 1),
    roughness: 0.5,
  })
  attachHint(box, 'Box · MeshRenderer + PBR material')

  const sphere = engine.addEntity()
  Transform.create(sphere, { position: Vector3.create(6.5, yRow, zRow) })
  MeshRenderer.setSphere(sphere)
  MeshCollider.setSphere(sphere)
  Material.setPbrMaterial(sphere, {
    albedoColor: Color4.create(0.2, 0.7, 0.3, 1),
    roughness: 0.4,
  })
  attachHint(sphere, 'Sphere · MeshRenderer + PBR material')

  const cylinder = engine.addEntity()
  Transform.create(cylinder, { position: Vector3.create(9, yRow, zRow) })
  MeshRenderer.setCylinder(cylinder)
  MeshCollider.setCylinder(cylinder)
  Material.setPbrMaterial(cylinder, {
    albedoColor: Color4.create(0.2, 0.4, 0.85, 1),
    roughness: 0.5,
  })
  attachHint(cylinder, 'Cylinder · MeshRenderer + PBR material')

  const plane = engine.addEntity()
  Transform.create(plane, {
    position: Vector3.create(11.5, yRow, zRow),
    rotation: Quaternion.fromEulerDegrees(0, 30, 0),
  })
  MeshRenderer.setPlane(plane)
  MeshCollider.setPlane(plane)
  Material.setPbrMaterial(plane, {
    albedoColor: Color4.create(0.85, 0.75, 0.2, 1),
  })
  attachHint(plane, 'Plane · MeshRenderer + PBR material (Y-tilted)')

  // ── primitives row at y=3 WITHOUT a Material component ───────────────────
  // Each shape is rendered with no Material — should fall back to whatever the
  // engine renders for an unmaterialed mesh. A cone is included as a cylinder
  // with radiusTop=0.
  const noMatY = 3

  const noMatBox = engine.addEntity()
  Transform.create(noMatBox, { position: Vector3.create(3, noMatY, zRow) })
  MeshRenderer.setBox(noMatBox)
  MeshCollider.setBox(noMatBox)
  attachHint(noMatBox, 'Box · NO Material (engine default appearance)')

  const noMatSphere = engine.addEntity()
  Transform.create(noMatSphere, { position: Vector3.create(5.5, noMatY, zRow) })
  MeshRenderer.setSphere(noMatSphere)
  MeshCollider.setSphere(noMatSphere)
  attachHint(noMatSphere, 'Sphere · NO Material (engine default appearance)')

  const noMatCylinder = engine.addEntity()
  Transform.create(noMatCylinder, { position: Vector3.create(8, noMatY, zRow) })
  MeshRenderer.setCylinder(noMatCylinder)
  MeshCollider.setCylinder(noMatCylinder)
  attachHint(noMatCylinder, 'Cylinder · NO Material (engine default appearance)')

  const noMatPlane = engine.addEntity()
  Transform.create(noMatPlane, {
    position: Vector3.create(10.5, noMatY, zRow),
    rotation: Quaternion.fromEulerDegrees(0, 30, 0),
  })
  MeshRenderer.setPlane(noMatPlane)
  MeshCollider.setPlane(noMatPlane)
  attachHint(noMatPlane, 'Plane · NO Material (engine default appearance)')

  // Cone == cylinder with radiusTop=0. Same null-Material treatment as the rest
  // of the row, with a matching collider so the hover hint registers.
  const noMatCone = engine.addEntity()
  Transform.create(noMatCone, { position: Vector3.create(13, noMatY, zRow) })
  MeshRenderer.setCylinder(noMatCone, 0.5, 0)
  MeshCollider.setCylinder(noMatCone, 0.5, 0)
  attachHint(noMatCone, 'Cone · cylinder with radiusTop=0 · NO Material')

  // ── Billboard plane ──────────────────────────────────────────────────────
  // Spawned with rotation 180° around Y so the plane's natural facing is AWAY
  // from the camera (which is on the −Z side). Billboard with BM_ALL must
  // override that rotation and orient the plane toward the camera every frame.
  // A regression where Billboard stops applying would leave the plane backside
  // to the camera, unlit, with the colored front not visible.
  const billboardPlane = engine.addEntity()
  Transform.create(billboardPlane, {
    position: Vector3.create(4, 5, 9),
    rotation: Quaternion.fromEulerDegrees(0, 180, 0),
  })
  MeshRenderer.setPlane(billboardPlane)
  MeshCollider.setPlane(billboardPlane)
  Material.setPbrMaterial(billboardPlane, {
    albedoColor: Color4.create(0.95, 0.4, 0.85, 1),
    emissiveColor: Color3.create(0.95, 0.4, 0.85),
    emissiveIntensity: 0.6,
    metallic: 0,
    roughness: 0.4,
  })
  Billboard.create(billboardPlane, { billboardMode: BillboardMode.BM_ALL })
  attachHint(
    billboardPlane,
    'Billboard plane · initial rotation 180°Y (faces away) · BM_ALL must orient to camera',
  )

  // ── Invisible entity (VisibilityComponent.visible = false) ───────────────
  // Vivid emissive sphere that should NOT appear in frame. If VisibilityComponent
  // regresses, the magenta glow would light up the diff immediately. The hover
  // hint still resolves on the (invisible) collider — useful for manual probing.
  const invisible = engine.addEntity()
  Transform.create(invisible, { position: Vector3.create(12, 5, 9) })
  MeshRenderer.setSphere(invisible)
  MeshCollider.setSphere(invisible)
  Material.setPbrMaterial(invisible, {
    albedoColor: Color4.create(1, 0, 1, 1),
    emissiveColor: Color3.create(1, 0, 1),
    emissiveIntensity: 4,
    metallic: 0,
    roughness: 0.4,
  })
  VisibilityComponent.create(invisible, { visible: false })
  attachHint(
    invisible,
    'VisibilityComponent · visible=false · vivid magenta sphere should be hidden',
  )

  // ── Parent + child with deviations on every axis ─────────────────────────
  // Parent has non-default position, rotation AND scale. Child has its own
  // non-default offset, rotation and scale on each axis. The child's world
  // transform must compose with the parent — any regression in transform-tree
  // composition (parent's rotation, scale, or position not propagating) will
  // shift the child's screen position visibly.
  const transformParent = engine.addEntity()
  Transform.create(transformParent, {
    position: Vector3.create(8, 5.5, 11),
    rotation: Quaternion.fromEulerDegrees(0, 45, 0),
    scale: Vector3.create(1.5, 1.2, 1.5),
  })
  MeshRenderer.setBox(transformParent)
  MeshCollider.setBox(transformParent)
  Material.setPbrMaterial(transformParent, {
    albedoColor: Color4.create(0.3, 0.55, 0.9, 1),
    roughness: 0.5,
  })
  attachHint(
    transformParent,
    'Parent · pos=(8,5.5,11) rot=(0,45°,0) scale=(1.5,1.2,1.5)',
  )

  const transformChild = engine.addEntity()
  Transform.create(transformChild, {
    parent: transformParent,
    position: Vector3.create(1, 0.6, 0.3),
    rotation: Quaternion.fromEulerDegrees(0, 0, 30),
    scale: Vector3.create(0.5, 0.8, 0.4),
  })
  MeshRenderer.setBox(transformChild)
  MeshCollider.setBox(transformChild)
  Material.setPbrMaterial(transformChild, {
    albedoColor: Color4.create(0.95, 0.6, 0.2, 1),
    roughness: 0.5,
  })
  attachHint(
    transformChild,
    'Child · local pos=(1,0.6,0.3) rot=(0,0,30°) scale=(0.5,0.8,0.4) · world = parent ∘ child',
  )

  // ── Cube WITHOUT a Transform component ───────────────────────────────────
  // Only MeshRenderer + Material. With no Transform, the engine should render
  // it at the identity transform (position (0,0,0), no rotation, scale 1).
  // Camera is angled to keep the parcel corner in frame so this cube is visible.
  const noTransform = engine.addEntity()
  MeshRenderer.setBox(noTransform)
  MeshCollider.setBox(noTransform)
  Material.setPbrMaterial(noTransform, {
    albedoColor: Color4.create(0.95, 0.95, 0.2, 1),
    emissiveColor: Color3.create(0.95, 0.95, 0.2),
    emissiveIntensity: 1.5,
    roughness: 0.5,
  })
  attachHint(noTransform, 'No Transform · should render at the origin (0, 0, 0)')

  // ── existing hierarchical transform chain (parent rotation × child offset)─
  const parent = engine.addEntity()
  Transform.create(parent, {
    position: Vector3.create(10, 2.5, 14),
    rotation: Quaternion.fromEulerDegrees(0, 30, 0),
  })

  makePrimitive(
    parent,
    Vector3.create(-0.6, 0, 0),
    'box',
    Color4.create(0.9, 0.9, 0.9, 1),
    'Group child · box at local x=-0.6 (parent rotated 30°Y)',
  )
  makePrimitive(
    parent,
    Vector3.create(0.6, 0, 0),
    'sphere',
    Color4.create(0.9, 0.5, 0.2, 1),
    'Group child · sphere at local x=+0.6 (parent rotated 30°Y)',
  )
  makePrimitive(
    parent,
    Vector3.create(0, 0.6, 0),
    'cylinder',
    Color4.create(0.5, 0.2, 0.8, 1),
    'Group child · cylinder at local y=+0.6 (parent rotated 30°Y)',
  )

  // ── lighting ─────────────────────────────────────────────────────────────
  const pointLight = engine.addEntity()
  Transform.create(pointLight, { position: Vector3.create(8, 4, 5) })
  LightSource.create(pointLight, {
    color: Color3.create(1, 0.95, 0.85),
    intensity: 40000,
    range: 12,
    active: true,
    shadow: false,
    type: LightSource.Type.Point({}),
  })

  const spotLight = engine.addEntity()
  Transform.create(spotLight, {
    position: Vector3.create(11, 5, 5),
    rotation: Quaternion.fromEulerDegrees(45, 180, 0),
  })
  LightSource.create(spotLight, {
    color: Color3.create(0.6, 0.8, 1),
    intensity: 60000,
    range: 14,
    active: true,
    shadow: false,
    type: LightSource.Type.Spot({ innerAngle: 12, outerAngle: 30 }),
  })


  // Camera pulled back/up and aimed slightly off-center so the cube at the
  // parcel corner (0, 0, 0) stays in frame alongside the y-stacked rows in
  // the parcel interior.
  setupVisualTest({
    lookAtPos: Vector3.create(8, 3, 8),
    cameraPos: Vector3.create(8, 4, 1),
  })
}

function makePrimitive(
  parent: Entity,
  offset: Vector3,
  shape: 'box' | 'sphere' | 'cylinder',
  color: Color4,
  hint: string,
): Entity {
  const e = engine.addEntity()
  Transform.create(e, { parent, position: offset, scale: Vector3.create(0.5, 0.5, 0.5) })
  if (shape === 'box') {
    MeshRenderer.setBox(e)
    MeshCollider.setBox(e)
  } else if (shape === 'sphere') {
    MeshRenderer.setSphere(e)
    MeshCollider.setSphere(e)
  } else {
    MeshRenderer.setCylinder(e)
    MeshCollider.setCylinder(e)
  }
  Material.setPbrMaterial(e, { albedoColor: color, roughness: 0.5 })
  attachHint(e, hint)
  return e
}

// Hover hint: show `text` while the cursor is over the entity. The empty
// click handler is intentional — the goal is the on-hover tooltip, not a
// click action. maxDistance is bumped well past the SDK default (~10) so
// the locked virtual camera (z=-11) still resolves hovers on every entity,
// including the cube at the parcel origin.
const HINT_MAX_DISTANCE = 100

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

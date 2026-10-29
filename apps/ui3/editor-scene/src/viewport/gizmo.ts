import {
  engine,
  Transform,
  MeshRenderer,
  MeshCollider,
  Material,
  MaterialTransparencyMode,
  inputSystem,
  InputAction,
  PointerEventType,
  PrimaryPointerInfo,
  ColliderLayer,
  type Entity
} from '@dcl/sdk/ecs'
import { CameraLayers } from '../camera-layers'
import { Vector3, Quaternion, Color3, Color4 } from '@dcl/sdk/math'
import { state, topLevelSelected, parentOf, revealGizmoForSelection } from '../state'
import {
  worldTransformOf,
  worldToLocalPosition,
  worldToLocalRotation,
  worldScaleOf
} from '../world-pos'
import { fireTransform } from '../inspector/mutate'
import {
  type Axis,
  type HandleKind,
  type PickConfig,
  axisVec,
  otherAxes,
  rayPlaneIntersect,
  pickHandleKind,
  axisDragPlaneNormal,
  angleOnPlane,
  constrainTranslation
} from './gizmo-pick'

type Local = { x: number; y: number; z: number }
type LocalRot = { x: number; y: number; z: number; w: number }

const SHAFT_LENGTH = 1.0
const SHAFT_RADIUS = 0.022
const TIP_LENGTH = 0.26
const TIP_RADIUS = 0.085
const HANDLE_RADIUS = 0.32
const PLANE_OFFSET = 0.42
const PLANE_SIZE = 0.22
const RING_RADIUS = 0.92
const RING_SEGMENTS = 32
const RING_SEGMENT_THICKNESS = 0.022
const RING_COLLIDER_THICKNESS = 0.2
const CUBE_SIZE = 0.16
const GIZMO_SCALE_FACTOR = 0.11
const GIZMO_MIN_SCALE = 0.45
const GIZMO_MAX_SCALE = 1000

const PICK_CFG: PickConfig = {
  armLength: SHAFT_LENGTH + TIP_LENGTH,
  planeOffset: PLANE_OFFSET,
  ringRadius: RING_RADIUS,
  axisTol: 0.34,
  planeHalf: PLANE_SIZE * 1.15,
  ringTol: 0.16,
  centerTol: CUBE_SIZE * 1.7
}

const AXIS_COLORS: Record<Axis, { c4: Color4; c3: Color3 }> = {
  x: { c4: Color4.create(0.82, 0.14, 0.16, 1), c3: Color3.create(0.82, 0.14, 0.16) },
  y: { c4: Color4.create(0.2, 0.62, 0.24, 1), c3: Color3.create(0.2, 0.62, 0.24) },
  z: { c4: Color4.create(0.18, 0.36, 0.86, 1), c3: Color3.create(0.18, 0.36, 0.86) }
}
const UNIFORM_COLOR = { c4: Color4.create(0.9, 0.9, 0.92, 1), c3: Color3.create(0.9, 0.9, 0.92) }
const HOVER_COLOR = { c4: Color4.create(1, 0.78, 0.12, 1), c3: Color3.create(1, 0.78, 0.12) }

const AXIS_ROTATION: Record<Axis, LocalRot> = {
  x: Quaternion.fromEulerDegrees(0, 0, -90),
  y: Quaternion.Identity(),
  z: Quaternion.fromEulerDegrees(90, 0, 0)
}

type HandleEntry = { handle: Entity; visuals: Entity[]; kind: HandleKind; hoverId: string }

let gizmoRoot: Entity | null = null
let gizmoEntities: Entity[] = []
let handles: HandleEntry[] = []
let builtSig = ''


function hoverId(kind: HandleKind): string {
  switch (kind.op) {
    case 'translate-axis':
      return kind.axis
    case 'translate-plane':
      return otherAxes(kind.normal).join('')
    case 'rotate':
      return `r${kind.axis}`
    case 'scale-axis':
      return `s${kind.axis}`
    case 'scale-uniform':
      return 'sc'
  }
}

function handleColors(kind: HandleKind): { c4: Color4; c3: Color3 } {
  switch (kind.op) {
    case 'translate-axis':
    case 'rotate':
    case 'scale-axis':
      return AXIS_COLORS[(kind as { axis: Axis }).axis]
    case 'translate-plane': {
      const [a, b] = otherAxes(kind.normal)
      const ca = AXIS_COLORS[a]
      const cb = AXIS_COLORS[b]
      return {
        c4: Color4.create((ca.c4.r + cb.c4.r) / 2, (ca.c4.g + cb.c4.g) / 2, (ca.c4.b + cb.c4.b) / 2, 0.6),
        c3: Color3.create((ca.c3.r + cb.c3.r) / 2, (ca.c3.g + cb.c3.g) / 2, (ca.c3.b + cb.c3.b) / 2)
      }
    }
    case 'scale-uniform':
      return UNIFORM_COLOR
  }
}

function saturateHue(c: Color3): Color3 {
  const mx = Math.max(c.r, c.g, c.b, 0.001)
  const p = 2.2
  return Color3.create(Math.pow(c.r / mx, p), Math.pow(c.g / mx, p), Math.pow(c.b / mx, p))
}

function applyMaterial(entry: HandleEntry, hovered: boolean): void {
  const { c4, c3 } = hovered ? HOVER_COLOR : handleColors(entry.kind)
  const translucent = entry.kind.op === 'translate-plane'
  const em = saturateHue(c3)
  for (const v of entry.visuals) {
    Material.setPbrMaterial(v, {
      albedoColor: translucent ? Color4.create(em.r, em.g, em.b, hovered ? 0.9 : 0.55) : Color4.create(em.r, em.g, em.b, 1),
      emissiveColor: em,
      emissiveIntensity: hovered ? 2.2 : 1.1,
      metallic: 0,
      roughness: 0.6,
      ...(translucent ? { transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND } : {})
    })
  }
}

function track(e: Entity): Entity {
  gizmoEntities.push(e)
  return e
}

function registerHandle(handle: Entity, visuals: Entity[], kind: HandleKind, _label: string): void {
  const entry: HandleEntry = { handle, visuals, kind, hoverId: hoverId(kind) }
  handles.push(entry)
  applyMaterial(entry, false)
}


function createArrow(axis: Axis, root: Entity): void {
  const container = track(engine.addEntity())
  Transform.create(container, { rotation: AXIS_ROTATION[axis], parent: root })

  const shaft = track(engine.addEntity())
  Transform.create(shaft, {
    position: Vector3.create(0, SHAFT_LENGTH / 2, 0),
    scale: Vector3.create(1, SHAFT_LENGTH, 1),
    parent: container
  })
  MeshRenderer.setCylinder(shaft, SHAFT_RADIUS, SHAFT_RADIUS)

  const tip = track(engine.addEntity())
  Transform.create(tip, {
    position: Vector3.create(0, SHAFT_LENGTH + TIP_LENGTH / 2, 0),
    scale: Vector3.create(1, TIP_LENGTH, 1),
    parent: container
  })
  MeshRenderer.setCylinder(tip, TIP_RADIUS, 0)

  const handle = track(engine.addEntity())
  Transform.create(handle, {
    position: Vector3.create(0, (SHAFT_LENGTH + TIP_LENGTH) / 2, 0),
    scale: Vector3.create(1, SHAFT_LENGTH + TIP_LENGTH, 1),
    parent: container
  })
  MeshCollider.setCylinder(handle, HANDLE_RADIUS, HANDLE_RADIUS, ColliderLayer.CL_POINTER)

  registerHandle(handle, [shaft, tip], { op: 'translate-axis', axis }, `Move ${axis.toUpperCase()}`)
}

function createPlaneHandle(normal: Axis, root: Entity): void {
  const axes = otherAxes(normal)
  const pos = Vector3.create(
    axes.includes('x') ? PLANE_OFFSET : 0,
    axes.includes('y') ? PLANE_OFFSET : 0,
    axes.includes('z') ? PLANE_OFFSET : 0
  )
  const visual = track(engine.addEntity())
  Transform.create(visual, {
    position: pos,
    scale: Vector3.create(
      normal === 'x' ? 0.02 : PLANE_SIZE,
      normal === 'y' ? 0.02 : PLANE_SIZE,
      normal === 'z' ? 0.02 : PLANE_SIZE
    ),
    parent: root
  })
  MeshRenderer.setBox(visual)

  const handle = track(engine.addEntity())
  Transform.create(handle, {
    position: pos,
    scale: Vector3.create(
      normal === 'x' ? 0.22 : PLANE_SIZE * 1.9,
      normal === 'y' ? 0.22 : PLANE_SIZE * 1.9,
      normal === 'z' ? 0.22 : PLANE_SIZE * 1.9
    ),
    parent: root
  })
  MeshCollider.setBox(handle, ColliderLayer.CL_POINTER)

  registerHandle(
    handle,
    [visual],
    { op: 'translate-plane', normal },
    `Move ${axes.map((a) => a.toUpperCase()).join('')}`
  )
}

function createRing(axis: Axis, root: Entity): void {
  const container = track(engine.addEntity())
  Transform.create(container, { rotation: AXIS_ROTATION[axis], parent: root })

  const segAngle = (Math.PI * 2) / RING_SEGMENTS
  const segLength = RING_RADIUS * 2 * Math.sin(segAngle / 2) * 1.05
  const visuals: Entity[] = []
  for (let i = 0; i < RING_SEGMENTS; i++) {
    const angle = i * segAngle
    const seg = track(engine.addEntity())
    Transform.create(seg, {
      position: Vector3.create(Math.cos(angle) * RING_RADIUS, 0, Math.sin(angle) * RING_RADIUS),
      rotation: Quaternion.fromEulerDegrees(0, (-(angle + Math.PI / 2) * 180) / Math.PI, 0),
      scale: Vector3.create(segLength, RING_SEGMENT_THICKNESS, RING_SEGMENT_THICKNESS),
      parent: container
    })
    MeshRenderer.setBox(seg)
    visuals.push(seg)
  }

  const handle = track(engine.addEntity())
  Transform.create(handle, {
    scale: Vector3.create(RING_RADIUS * 2, RING_COLLIDER_THICKNESS, RING_RADIUS * 2),
    parent: container
  })
  MeshCollider.setCylinder(handle, 0.5, 0.5, ColliderLayer.CL_POINTER)

  registerHandle(handle, visuals, { op: 'rotate', axis }, `Rotate ${axis.toUpperCase()}`)
}

function createScaleHandle(axis: Axis, root: Entity): void {
  const container = track(engine.addEntity())
  Transform.create(container, { rotation: AXIS_ROTATION[axis], parent: root })

  const shaft = track(engine.addEntity())
  Transform.create(shaft, {
    position: Vector3.create(0, SHAFT_LENGTH / 2, 0),
    scale: Vector3.create(1, SHAFT_LENGTH, 1),
    parent: container
  })
  MeshRenderer.setCylinder(shaft, SHAFT_RADIUS, SHAFT_RADIUS)

  const cube = track(engine.addEntity())
  Transform.create(cube, {
    position: Vector3.create(0, SHAFT_LENGTH + CUBE_SIZE / 2, 0),
    scale: Vector3.create(CUBE_SIZE, CUBE_SIZE, CUBE_SIZE),
    parent: container
  })
  MeshRenderer.setBox(cube)

  const handle = track(engine.addEntity())
  Transform.create(handle, {
    position: Vector3.create(0, (SHAFT_LENGTH + CUBE_SIZE) / 2, 0),
    scale: Vector3.create(1, SHAFT_LENGTH + CUBE_SIZE, 1),
    parent: container
  })
  MeshCollider.setCylinder(handle, HANDLE_RADIUS, HANDLE_RADIUS, ColliderLayer.CL_POINTER)

  registerHandle(handle, [shaft, cube], { op: 'scale-axis', axis }, `Scale ${axis.toUpperCase()}`)
}

function createUniformScaleHandle(root: Entity): void {
  const cube = track(engine.addEntity())
  Transform.create(cube, { scale: Vector3.create(CUBE_SIZE * 1.4, CUBE_SIZE * 1.4, CUBE_SIZE * 1.4), parent: root })
  MeshRenderer.setBox(cube)

  const handle = track(engine.addEntity())
  Transform.create(handle, { scale: Vector3.create(CUBE_SIZE * 3, CUBE_SIZE * 3, CUBE_SIZE * 3), parent: root })
  MeshCollider.setBox(handle, ColliderLayer.CL_POINTER)

  registerHandle(handle, [cube], { op: 'scale-uniform' }, 'Scale uniformly')
}

function destroyGizmo(): void {
  for (const e of gizmoEntities) engine.removeEntity(e)
  if (gizmoRoot !== null) engine.removeEntity(gizmoRoot)
  gizmoRoot = null
  gizmoEntities = []
  handles = []
  builtSig = ''
  if (state.gizmoHover !== null) state.gizmoHover = null
}

function buildGizmo(mode: string): void {
  destroyGizmo()
  gizmoRoot = engine.addEntity()
  Transform.create(gizmoRoot, {})
  CameraLayers.create(gizmoRoot, { layers: [7] })
  if (mode === 'translate') {
    createArrow('x', gizmoRoot)
    createArrow('y', gizmoRoot)
    createArrow('z', gizmoRoot)
    createPlaneHandle('x', gizmoRoot)
    createPlaneHandle('y', gizmoRoot)
    createPlaneHandle('z', gizmoRoot)
  } else if (mode === 'rotate') {
    createRing('x', gizmoRoot)
    createRing('y', gizmoRoot)
    createRing('z', gizmoRoot)
  } else {
    createScaleHandle('x', gizmoRoot)
    createScaleHandle('y', gizmoRoot)
    createScaleHandle('z', gizmoRoot)
    createUniformScaleHandle(gizmoRoot)
  }
}


type GroupEntry = {
  id: string
  startWorldPos: Vector3
  startWorldRot: LocalRot
  position: Local
  rotation: LocalRot
  scale: Local
  parent: number
}

type DragState = {
  kind: HandleKind
  entry: HandleEntry
  center: Vector3
  anchorStart?: Vector3
  planeNormal: Vector3
  startHit: Vector3
  axisDir?: Vector3
  startAngle?: number
  gizmoScale: number
  group: GroupEntry[]
}

let drag: DragState | null = null
let liveDelta: Vector3 | null = null
let liveRoots: Set<string> | null = null


function captureGroup(): GroupEntry[] {
  const out: GroupEntry[] = []
  for (const id of topLevelSelected(state.snapshot)) {
    const wt = worldTransformOf(state.snapshot, id)
    if (wt === null) continue
    const t = state.snapshot[id]?.Transform as
      | { position?: Local; rotation?: LocalRot; scale?: Local; parent?: number }
      | undefined
    out.push({
      id,
      startWorldPos: { ...wt.position },
      startWorldRot: { ...wt.rotation },
      position: t?.position ?? { x: 0, y: 0, z: 0 },
      rotation: t?.rotation ?? { x: 0, y: 0, z: 0, w: 1 },
      scale: t?.scale ?? { x: 1, y: 1, z: 1 },
      parent: t?.parent ?? 0
    })
  }
  return out
}

function pointerRay(): { origin: Vector3; dir: Vector3 } | null {
  const p = PrimaryPointerInfo.getOrNull(engine.RootEntity)
  const camT = Transform.getOrNull(engine.CameraEntity)
  if (p?.worldRayDirection === undefined || camT === null) return null
  return { origin: camT.position, dir: p.worldRayDirection as Vector3 }
}

function beginDrag(entry: HandleEntry): void {
  if (state.activeEntity === null || gizmoRoot === null) return
  const ray = pointerRay()
  if (ray === null) return
  const rootT = Transform.get(gizmoRoot)
  const center = { ...rootT.position }
  const gizmoScale = rootT.scale.x
  const kind = entry.kind

  let planeNormal: Vector3
  let axisDir: Vector3 | undefined
  let startAngle: number | undefined

  const camT = Transform.getOrNull(engine.CameraEntity)
  const camForward = camT === null ? Vector3.Forward() : Vector3.rotate(Vector3.Forward(), camT.rotation)

  if (kind.op === 'translate-axis') {
    axisDir = axisVec(kind.axis)
    planeNormal = axisDragPlaneNormal(axisDir, camForward)
  } else if (kind.op === 'translate-plane') {
    planeNormal = axisVec(kind.normal)
  } else if (kind.op === 'rotate') {
    planeNormal = axisVec(kind.axis)
  } else if (kind.op === 'scale-axis') {
    axisDir = axisVec(kind.axis)
    planeNormal = axisDragPlaneNormal(axisDir, camForward)
  } else {
    planeNormal = Vector3.scale(camForward, -1)
  }

  const startHit = rayPlaneIntersect(ray.origin, ray.dir, center, planeNormal)
  if (startHit === null) return
  if (kind.op === 'rotate') startAngle = angleOnPlane(startHit, center, planeNormal)

  const group = captureGroup()
  let anchorStart: Vector3 | undefined
  if (group.length > 0) {
    let sum = Vector3.Zero()
    for (const g of group) sum = Vector3.add(sum, g.startWorldPos)
    anchorStart = Vector3.scale(sum, 1 / group.length)
  }
  drag = {
    kind,
    entry,
    center,
    anchorStart,
    planeNormal,
    startHit,
    axisDir,
    startAngle,
    gizmoScale,
    group
  }
  liveDelta = null
  liveRoots = new Set(drag.group.map((g) => g.id))
  state.gizmoDragging = true
}

export function endGizmoDrag(): void {
  if (drag === null) {
    state.gizmoDragging = false
    return
  }
  drag = null
  liveDelta = null
  liveRoots = null
  state.gizmoDragging = false
  for (const h of handles) applyMaterial(h, false)
}

function writeEntry(g: GroupEntry, t: { position: Local; rotation: LocalRot; scale: Local }): void {
  fireTransform(
    g.id,
    JSON.stringify({ position: t.position, rotation: t.rotation, scale: t.scale, parent: g.parent })
  )
}

function updateDrag(): void {
  const d = drag
  if (d === null) return
  const ray = pointerRay()
  if (ray === null) return
  const hit = rayPlaneIntersect(ray.origin, ray.dir, d.center, d.planeNormal)
  if (hit === null) return

  if (d.kind.op === 'translate-axis' || d.kind.op === 'translate-plane') {
    const constrained = constrainTranslation(d.kind, Vector3.subtract(hit, d.startHit))
    liveDelta = constrained
    for (const g of d.group) {
      const newWorld = Vector3.add(g.startWorldPos, constrained)
      const localPos = worldToLocalPosition(state.snapshot, g.id, newWorld)
      if (localPos === null) continue
      writeEntry(g, { position: localPos, rotation: g.rotation, scale: g.scale })
    }
  } else if (d.kind.op === 'rotate') {
    const angle = angleOnPlane(hit, d.center, d.planeNormal)
    const degrees = ((angle - (d.startAngle as number)) * 180) / Math.PI
    const incremental = Quaternion.fromAngleAxis(degrees, d.planeNormal)
    for (const g of d.group) {
      const newWorldRot = Quaternion.multiply(incremental, g.startWorldRot)
      const localRot = worldToLocalRotation(state.snapshot, g.id, newWorldRot)
      writeEntry(g, { position: g.position, rotation: localRot ?? g.rotation, scale: g.scale })
    }
  } else {
    let disp: number
    if (d.kind.op === 'scale-axis') {
      disp = Vector3.dot(Vector3.subtract(hit, d.startHit), d.axisDir as Vector3)
    } else {
      const rel = Vector3.subtract(hit, d.startHit)
      disp = (rel.x + rel.y) / 2 + rel.z / 2
    }
    const factor = Math.max(0.01, 1 + disp / Math.max(0.001, d.gizmoScale * 1.5))
    for (const g of d.group) {
      const scale =
        d.kind.op === 'scale-uniform'
          ? { x: g.scale.x * factor, y: g.scale.y * factor, z: g.scale.z * factor }
          : {
              x: d.kind.axis === 'x' ? g.scale.x * factor : g.scale.x,
              y: d.kind.axis === 'y' ? g.scale.y * factor : g.scale.y,
              z: d.kind.axis === 'z' ? g.scale.z * factor : g.scale.z
            }
      writeEntry(g, { position: g.position, rotation: g.rotation, scale })
    }
  }
}

function pickHandle(ray: { origin: Vector3; dir: Vector3 }): HandleEntry | null {
  if (gizmoRoot === null) return null
  const rt = Transform.getOrNull(gizmoRoot)
  if (rt === null) return null
  const idx = pickHandleKind(handles.map((h) => h.kind), rt.position, rt.scale.x, ray, PICK_CFG)
  return idx < 0 ? null : handles[idx]
}


function gizmoActive(): boolean {
  const mode = state.activeAction
  return (
    !state.playing &&
    state.status === 'ready' &&
    (mode === 'translate' || mode === 'rotate' || mode === 'scale') &&
    state.activeEntity !== null &&
    state.snapshot[state.activeEntity] !== undefined
  )
}

function gizmoSystem(_dt: number): void {
  if (drag !== null) {
    if (
      inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_UP) ||
      !inputSystem.isPressed(InputAction.IA_POINTER)
    ) {
      endGizmoDrag()
    } else {
      updateDrag()
    }
  }

  revealGizmoForSelection()
  if (!gizmoActive()) {
    if (gizmoRoot !== null) destroyGizmo()
    return
  }

  const sig = `${state.activeAction}`
  if (gizmoRoot === null || builtSig !== sig) {
    buildGizmo(state.activeAction)
    builtSig = sig
  }

  const active = state.activeEntity as string
  if (gizmoRoot === null) return
  const roots = topLevelSelected(state.snapshot)
  let anchor: Vector3 | null = null
  if (roots.length > 1) {
    let sum = Vector3.Zero()
    let n = 0
    for (const id of roots) {
      const w = worldTransformOf(state.snapshot, id)
      if (w === null) continue
      sum = Vector3.add(sum, w.position)
      n++
    }
    if (n > 0) anchor = Vector3.scale(sum, 1 / n)
  }
  if (anchor === null) {
    const wt = worldTransformOf(state.snapshot, active)
    if (wt === null) return
    anchor = wt.position
  }
  const camT = Transform.getOrNull(engine.CameraEntity)
  let base = anchor
  if (drag !== null && liveDelta !== null && drag.anchorStart !== undefined) {
    base = Vector3.add(drag.anchorStart, liveDelta)
  }

  const dist = camT === null ? 8 : Vector3.distance(camT.position, base)
  const s = Math.min(GIZMO_MAX_SCALE, Math.max(GIZMO_MIN_SCALE, dist * GIZMO_SCALE_FACTOR))

  const pos = base

  const t = Transform.getMutable(gizmoRoot)
  t.position = { ...pos }
  t.rotation = Quaternion.Identity()
  t.scale = Vector3.create(s, s, s)

  if (drag === null) {
    const ray = pointerRay()
    const hovered = ray === null ? null : pickHandle(ray)
    const hid = hovered === null ? null : hovered.hoverId
    if (hid !== state.gizmoHover) {
      state.gizmoHover = hid
      for (const h of handles) applyMaterial(h, h === hovered)
    }
    if (hovered !== null && inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
      beginDrag(hovered)
    }
  }
}

export function liveWorldPos(id: string, snapshotWorld: Vector3): Vector3 {
  if (drag === null || liveDelta === null || liveRoots === null) return snapshotWorld
  let cur: string | null = id
  while (cur !== null) {
    if (liveRoots.has(cur)) return Vector3.add(snapshotWorld, liveDelta)
    cur = parentOf(state.snapshot, cur)
  }
  return snapshotWorld
}

export function setupGizmo(): void {
  engine.addSystem(gizmoSystem)
}

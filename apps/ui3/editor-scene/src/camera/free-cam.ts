import {
  engine,
  Transform,
  VirtualCamera,
  MainCamera,
  InputModifier,
  PointerLock,
  PrimaryPointerInfo,
  UiCanvasInformation,
  InputAction,
  inputSystem,
  type Entity
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'
import { state } from '../state'
import { NAME_COMPONENT } from '../custom-components'
import {
  worldTransformOf,
  worldPositionOf,
  computeWorldPositions,
  rootLocalForWorld
} from '../world-pos'
import { cameraFovY } from './camera-projection'
import { rotateVec3ByQuat } from './perspective-to-screen'

const MOUSE_SENSITIVITY = 0.003
let flySpeed = 15
export function adjustFlySpeed(factor: number): void {
  flySpeed = Math.max(1, Math.min(200, flySpeed * factor))
}

const camPrefs = {
  sensitivity: { orbit: 1, pan: 1, zoom: 1 },
  invertY: false
}
function clampSens(n: number): number {
  return Number.isFinite(n) ? Math.max(0.1, Math.min(3, n)) : 1
}
export function setCameraSettings(
  sensitivity: { orbit: number; pan: number; zoom: number },
  invertY: boolean
): void {
  camPrefs.sensitivity = {
    orbit: clampSens(sensitivity?.orbit ?? 1),
    pan: clampSens(sensitivity?.pan ?? 1),
    zoom: clampSens(sensitivity?.zoom ?? 1)
  }
  camPrefs.invertY = !!invertY
}
const FREE_MOVE_DIST = 5

const DROP_AHEAD = 6
const DROP_MAX_GROUND = 40

export function cameraPose(): { x: number; y: number; z: number; yaw: number; pitch: number } | null {
  if (state.camMode === 'none') {
    const camera = Transform.getOrNull(engine.CameraEntity)
    if (camera === null) return null
    const direction = rotateVec3ByQuat(Vector3.Forward(), camera.rotation as Quaternion)
    return {
      ...camera.position,
      yaw: Math.atan2(direction.x, direction.z) * RAD_TO_DEG,
      pitch: Math.atan2(-direction.y, Math.hypot(direction.x, direction.z)) * RAD_TO_DEG
    }
  }
  if (camEntity === null) return null
  const p = Transform.get(camEntity).position
  return { x: p.x, y: p.y, z: p.z, yaw: yaw * RAD_TO_DEG, pitch: pitch * RAD_TO_DEG }
}

export function cameraDropLocal(): { x: number; y: number; z: number } | null {
  const camT = Transform.getOrNull(engine.CameraEntity)
  if (camT === null) return null
  const cam = camT.position
  const fwd = rotateVec3ByQuat(Vector3.create(0, 0, 1), camT.rotation as Quaternion)
  let world: Vector3
  if (fwd.y < -0.05 && cam.y > 0) {
    const dist = Math.min(cam.y / -fwd.y, DROP_MAX_GROUND)
    world = Vector3.add(cam, Vector3.scale(fwd, dist))
  } else {
    const horiz = Vector3.create(fwd.x, 0, fwd.z)
    const len = Vector3.length(horiz)
    const dir = len > 1e-4 ? Vector3.scale(horiz, 1 / len) : Vector3.create(0, 0, 1)
    world = Vector3.add(cam, Vector3.scale(dir, DROP_AHEAD))
  }
  return rootLocalForWorld(state.snapshot, Vector3.create(world.x, 0, world.z))
}
const ORBIT_KEY_SPEED = 1.6
const DOLLY_SPEED = 8
const MIN_DIST = 0.5
const PITCH_LIMIT = Math.PI / 2 - 0.01
const RAD_TO_DEG = 180 / Math.PI
const TWEEN_DURATION = 0.3

const OVERHEAD_YAW = Math.PI / 4
const OVERHEAD_PITCH = 0.96
const OVERHEAD_DISTANCE = 24
const SATELLITE_DISTANCE = 16
const SATELLITE_PITCH = Math.PI * 0.23
const SATELLITE_TWEEN_S = 0.2

let camEntity: Entity | null = null
let yaw = 0
let pitch = 0
let distance = 5
let focusPoint: Vector3 | null = null
let tween: {
  fromPos: Vector3
  toPos: Vector3
  fromYaw: number
  toYaw: number
  fromPitch: number
  toPitch: number
  elapsed: number
  duration: number
} | null = null

function clampPitch(p: number): number {
  return Math.max(-PITCH_LIMIT, Math.min(PITCH_LIMIT, p))
}

function lookRotation(): Quaternion {
  const yawQ = Quaternion.fromAngleAxis(yaw * RAD_TO_DEG, Vector3.Up())
  const pitchQ = Quaternion.fromAngleAxis(pitch * RAD_TO_DEG, Vector3.Right())
  return Quaternion.multiply(yawQ, pitchQ)
}

function aimAlong(dir: Vector3): void {
  const horiz = Math.sqrt(dir.x * dir.x + dir.z * dir.z)
  yaw = Math.atan2(dir.x, dir.z)
  pitch = clampPitch(Math.atan2(-dir.y, horiz))
}

function startTween(toPos: Vector3, lookDir: Vector3, duration: number = TWEEN_DURATION): void {
  if (camEntity === null) return
  const horiz = Math.sqrt(lookDir.x * lookDir.x + lookDir.z * lookDir.z)
  const rawYaw = Math.atan2(lookDir.x, lookDir.z)
  const dYaw = Math.atan2(Math.sin(rawYaw - yaw), Math.cos(rawYaw - yaw))
  tween = {
    fromPos: { ...Transform.get(camEntity).position },
    toPos: { ...toPos },
    fromYaw: yaw,
    toYaw: yaw + dYaw,
    fromPitch: pitch,
    toPitch: clampPitch(Math.atan2(-lookDir.y, horiz)),
    elapsed: 0,
    duration
  }
}

let orbitTargetId: string | null = null

function sceneCenter(): Vector3 {
  const world = computeWorldPositions(state.snapshot)
  if (world !== null) {
    let n = 0
    const acc = Vector3.Zero()
    for (const [id, pos] of world) {
      if (Number(id) < 512) continue
      if (state.snapshot[id]?.[NAME_COMPONENT] === undefined) continue
      acc.x += pos.x
      acc.y += pos.y
      acc.z += pos.z
      n += 1
    }
    if (n > 0) return Vector3.create(acc.x / n, acc.y / n, acc.z / n)
  }
  return Vector3.create(8, 0, 8)
}

function activeTarget(): Vector3 | null {
  const id =
    state.camMode === 'target' && orbitTargetId !== null ? orbitTargetId : state.activeEntity
  if (id === null) return null
  return worldTransformOf(state.snapshot, id)?.position ?? null
}

function sphericalOffset(yawA: number, pitchA: number, dist: number): Vector3 {
  // Camera position relative to the target for a camera aimed at the target
  // with the given yaw/pitch (pitch > 0 looks down).
  const cp = Math.cos(pitchA)
  return Vector3.create(
    -Math.sin(yawA) * cp * dist,
    Math.sin(pitchA) * dist,
    -Math.cos(yawA) * cp * dist
  )
}


export function setupCamera(): void {
  if (camEntity !== null) return
  const cam = engine.addEntity()
  Transform.create(cam)
  // The renderer otherwise applies its default transition from the player
  // camera when the edit camera first takes over.
  VirtualCamera.create(cam, { defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0) } })
  camEntity = cam
  engine.addSystem(cameraSystem)
}

export function setCamMode(mode: 'none' | 'free' | 'target'): void {
  if (camEntity === null || mode === state.camMode) return
  const wasCam = state.camMode !== 'none'
  const isCam = mode !== 'none'

  if (!wasCam && isCam) {
    const camT = Transform.getOrNull(engine.CameraEntity)
    const t = Transform.getMutable(camEntity)
    if (camT !== null) {
      t.position = { ...camT.position }
      aimAlong(Vector3.rotate(Vector3.Forward(), camT.rotation as Quaternion))
    }
    t.rotation = lookRotation()
    MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: camEntity })
  }
  if (wasCam && !isCam) {
    MainCamera.deleteFrom(engine.CameraEntity)
  }

  state.camMode = mode
  tween = null
  orbitTargetId = mode === 'target' ? state.activeEntity : null
  if (mode === 'target') initTarget()
  reconcileAvatarInput()
}

let restoreEditorCamera: (() => void) | null = null
export function setPlayingCamera(playing: boolean): void {
  if (playing && restoreEditorCamera === null && camEntity !== null) {
    const mode = state.camMode
    const position = { ...Transform.get(camEntity).position }
    const savedYaw = yaw
    const savedPitch = pitch
    const savedDistance = distance
    const savedFocus = focusPoint === null ? null : { ...focusPoint }
    const savedTarget = orbitTargetId
    restoreEditorCamera = () => {
      setCamMode(mode)
      if (camEntity === null) return
      yaw = savedYaw
      pitch = savedPitch
      distance = savedDistance
      focusPoint = savedFocus
      orbitTargetId = savedTarget
      tween = null
      const t = Transform.getMutable(camEntity)
      t.position = position
      t.rotation = lookRotation()
    }
    setCamMode('none')
  } else if (!playing && restoreEditorCamera !== null) {
    restoreEditorCamera()
    restoreEditorCamera = null
  }
  reconcileAvatarInput()
}

export function focusOrbitOn(id: string): void {
  orbitTargetId = id
  if (state.camMode === 'target') initTarget()
  else setCamMode('target')
}

export function focusSatellite(id: string): void {
  if (camEntity === null) return
  orbitTargetId = id
  if (state.camMode !== 'target') setCamMode('target')
  const target = worldPositionOf(state.snapshot, id)
  if (target === null) return
  focusPoint = { ...target }
  distance = SATELLITE_DISTANCE
  const camPos = Vector3.add(target, sphericalOffset(yaw, SATELLITE_PITCH, SATELLITE_DISTANCE))
  startTween(camPos, Vector3.subtract(target, camPos), SATELLITE_TWEEN_S)
}

export function frameEntityOnce(id: string): void {
  if (camEntity === null) return
  const target = worldTransformOf(state.snapshot, id)?.position
  if (target === undefined) return
  focusPoint = { ...target }
  setCamMode('free')
  const camPos = Transform.get(camEntity).position
  const toTarget = Vector3.subtract(target, camPos)
  const dist = Vector3.length(toTarget)
  const dir = dist > 1e-3 ? Vector3.scale(toTarget, 1 / dist) : Vector3.Forward()
  const frameDist = Math.min(Math.max(dist, MIN_DIST), 12)
  startTween(Vector3.subtract(target, Vector3.scale(dir, frameDist)), dir)
}

let framedOnce = false

function initTarget(): void {
  if (camEntity === null) return
  const target = activeTarget() ?? sceneCenter()
  focusPoint = { ...target }
  const pos = Transform.get(camEntity).position
  if (!framedOnce) {
    framedOnce = true
    distance = OVERHEAD_DISTANCE
    const camPos = Vector3.add(
      target,
      sphericalOffset(OVERHEAD_YAW, OVERHEAD_PITCH, OVERHEAD_DISTANCE)
    )
    // The first edit view must be in place before scene-ready is announced.
    // Tweening from the player camera exposes the avatar during editor boot.
    aimAlong(Vector3.subtract(target, camPos))
    const t = Transform.getMutable(camEntity)
    t.position = camPos
    t.rotation = lookRotation()
    return
  }
  distance = Math.max(MIN_DIST, Vector3.distance(pos, target))
  startTween({ ...pos }, Vector3.subtract(target, pos))
}


let wasGizmoDragging = false

let avatarWalkEnabled = true
function reconcileAvatarInput(): void {
  const enable = state.camMode === 'none'
  if (enable === avatarWalkEnabled) return
  avatarWalkEnabled = enable
  if (enable) {
    InputModifier.deleteFrom(engine.PlayerEntity)
  } else {
    InputModifier.createOrReplace(engine.PlayerEntity, {
      mode: { $case: 'standard', standard: { disableAll: true } }
    })
  }
}

function cameraSystem(dt: number): void {
  reconcileAvatarInput()
  if (camEntity === null) return

  if (wasGizmoDragging && !state.gizmoDragging && state.camMode === 'target') {
    const target = activeTarget()
    const pos = Transform.get(camEntity).position
    if (target !== null) {
      focusPoint = { ...target }
      distance = Math.max(MIN_DIST, Vector3.distance(pos, target))
      aimAlong(Vector3.subtract(target, pos))
    }
  }
  wasGizmoDragging = state.gizmoDragging

  if (state.camMode === 'none') return

  const locked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  const ptr = PrimaryPointerInfo.getOrNull(engine.RootEntity)
  const lookDx = locked ? ptr?.screenDelta?.x ?? 0 : 0
  const lookDy = locked ? ptr?.screenDelta?.y ?? 0 : 0
  const anyKey =
    inputSystem.isPressed(InputAction.IA_FORWARD) ||
    inputSystem.isPressed(InputAction.IA_BACKWARD) ||
    inputSystem.isPressed(InputAction.IA_LEFT) ||
    inputSystem.isPressed(InputAction.IA_RIGHT) ||
    inputSystem.isPressed(InputAction.IA_JUMP) ||
    inputSystem.isPressed(InputAction.IA_WALK)

  if (tween !== null) {
    if (anyKey || lookDx !== 0 || lookDy !== 0) {
      tween = null
    } else {
      tween.elapsed += dt
      const u = Math.min(1, tween.elapsed / tween.duration)
      const e = u * u * (3 - 2 * u)
      yaw = tween.fromYaw + (tween.toYaw - tween.fromYaw) * e
      pitch = tween.fromPitch + (tween.toPitch - tween.fromPitch) * e
      const tw = Transform.getMutable(camEntity)
      tw.position = Vector3.add(
        tween.fromPos,
        Vector3.scale(Vector3.subtract(tween.toPos, tween.fromPos), e)
      )
      tw.rotation = lookRotation()
      if (u >= 1) tween = null
      return
    }
  }

  if (state.camMode === 'free') flyStep(dt, lookDx, lookDy)
  else orbitStep(dt, lookDx, lookDy)
}

function flyStep(dt: number, lookDx: number, lookDy: number): void {
  if (camEntity === null) return
  yaw += lookDx * MOUSE_SENSITIVITY * camPrefs.sensitivity.orbit
  pitch = clampPitch(pitch + (camPrefs.invertY ? -lookDy : lookDy) * MOUSE_SENSITIVITY * camPrefs.sensitivity.orbit)

  const rotation = lookRotation()
  const forward = Vector3.rotate(Vector3.Forward(), rotation)
  const right = Vector3.rotate(Vector3.Right(), rotation)
  let move = Vector3.Zero()
  if (inputSystem.isPressed(InputAction.IA_FORWARD)) move = Vector3.add(move, forward)
  if (inputSystem.isPressed(InputAction.IA_BACKWARD)) move = Vector3.subtract(move, forward)
  if (inputSystem.isPressed(InputAction.IA_RIGHT)) move = Vector3.add(move, right)
  if (inputSystem.isPressed(InputAction.IA_LEFT)) move = Vector3.subtract(move, right)
  if (inputSystem.isPressed(InputAction.IA_JUMP)) move = Vector3.add(move, Vector3.Up())
  if (inputSystem.isPressed(InputAction.IA_WALK)) move = Vector3.subtract(move, Vector3.Up())

  const t = Transform.getMutable(camEntity)
  if (Vector3.lengthSquared(move) > 1e-6) {
    t.position = Vector3.add(t.position, Vector3.scale(Vector3.normalize(move), flySpeed * dt))
  }
  t.rotation = rotation
}

function orbitStep(dt: number, lookDx: number, lookDy: number): void {
  if (camEntity === null) return
  if (state.gizmoDragging) return
  const target = focusPoint ?? activeTarget() ?? sceneCenter()

  yaw += lookDx * MOUSE_SENSITIVITY * camPrefs.sensitivity.orbit
  pitch = clampPitch(pitch + (camPrefs.invertY ? -lookDy : lookDy) * MOUSE_SENSITIVITY * camPrefs.sensitivity.orbit)
  // Tool shortcuts share WASD/E with SDK input. Only navigate with those keys
  // while looking; otherwise selecting Move also dollies the orbit camera.
  if (PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) {
    if (inputSystem.isPressed(InputAction.IA_LEFT)) yaw -= ORBIT_KEY_SPEED * dt
    if (inputSystem.isPressed(InputAction.IA_RIGHT)) yaw += ORBIT_KEY_SPEED * dt
    if (inputSystem.isPressed(InputAction.IA_SECONDARY)) pitch = clampPitch(pitch + ORBIT_KEY_SPEED * dt)
    if (inputSystem.isPressed(InputAction.IA_PRIMARY)) pitch = clampPitch(pitch - ORBIT_KEY_SPEED * dt)
    if (inputSystem.isPressed(InputAction.IA_FORWARD)) distance -= DOLLY_SPEED * camPrefs.sensitivity.zoom * dt
    if (inputSystem.isPressed(InputAction.IA_BACKWARD)) distance += DOLLY_SPEED * camPrefs.sensitivity.zoom * dt
  }
  distance = Math.max(MIN_DIST, distance)

  const rotation = lookRotation()
  const forward = Vector3.rotate(Vector3.Forward(), rotation)
  const t = Transform.getMutable(camEntity)
  t.position = Vector3.subtract(target, Vector3.scale(forward, distance))
  t.rotation = rotation
}

export function currentOrbitDistance(): number {
  return distance
}

export function applyCameraInput(d: {
  orbitYaw?: number
  orbitPitch?: number
  panX?: number
  panY?: number
  dolly?: number
}): void {
  if (camEntity === null || state.camMode === 'none') return
  tween = null
  const orbitYaw = d.orbitYaw ?? 0
  const orbitPitch = d.orbitPitch ?? 0
  const panX = d.panX ?? 0
  const panY = d.panY ?? 0
  const dolly = d.dolly ?? 0

  yaw += orbitYaw
  pitch = clampPitch(pitch + orbitPitch)

  const rotation = lookRotation()
  const forward = Vector3.rotate(Vector3.Forward(), rotation)
  const right = Vector3.rotate(Vector3.Right(), rotation)
  const up = Vector3.rotate(Vector3.Up(), rotation)
  const t = Transform.getMutable(camEntity)

  if (state.camMode === 'target') {
    if (focusPoint === null) focusPoint = activeTarget() ?? sceneCenter()
    if (dolly !== 0) distance = Math.max(MIN_DIST, distance * (1 - dolly))
    if (panX !== 0 || panY !== 0) {
      focusPoint = Vector3.add(
        focusPoint,
        Vector3.add(Vector3.scale(right, -panX * distance), Vector3.scale(up, panY * distance))
      )
    }
    t.position = Vector3.subtract(focusPoint, Vector3.scale(forward, distance))
    t.rotation = rotation
  } else {
    let pos = t.position
    if (panX !== 0 || panY !== 0) {
      pos = Vector3.add(
        pos,
        Vector3.add(
          Vector3.scale(right, -panX * FREE_MOVE_DIST),
          Vector3.scale(up, panY * FREE_MOVE_DIST)
        )
      )
    }
    if (dolly !== 0) pos = Vector3.add(pos, Vector3.scale(forward, dolly * FREE_MOVE_DIST))
    t.position = pos
    t.rotation = rotation
  }
}


function fitDistance(radius: number): number {
  const fovY = cameraFovY() ?? Math.PI / 4
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  const aspect = canvas !== null && canvas.height > 0 ? canvas.width / canvas.height : 16 / 9
  const fovX = 2 * Math.atan(Math.tan(fovY / 2) * aspect)
  const halfFov = Math.min(fovY, fovX) / 2
  const r = Math.max(radius, 1)
  return Math.max(4, (r / Math.sin(halfFov)) * 1.3)
}

function axisUnit(axis: 'x' | 'y' | 'z', sign: number): Vector3 {
  return axis === 'x'
    ? Vector3.create(sign, 0, 0)
    : axis === 'y'
      ? Vector3.create(0, sign, 0)
      : Vector3.create(0, 0, sign)
}

export function orientToAxis(axis: 'x' | 'y' | 'z', sign: number): void {
  if (camEntity === null || state.camMode === 'none') return
  const target = activeTarget()
  if (target === null) return

  let radius = 0
  const world = computeWorldPositions(state.snapshot)
  if (world !== null) {
    for (const id of state.selected) {
      const p = world.get(id)
      if (p !== undefined) radius = Math.max(radius, Vector3.distance(p, target))
    }
  }

  const d = fitDistance(radius)
  if (state.camMode === 'target') distance = d
  const camPos = Vector3.add(target, Vector3.scale(axisUnit(axis, sign), d))
  startTween(camPos, Vector3.subtract(target, camPos))
}

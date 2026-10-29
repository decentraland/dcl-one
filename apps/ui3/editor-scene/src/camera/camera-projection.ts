import {
  engine,
  executeTask,
  Transform,
  UiCanvasInformation
} from '@dcl/sdk/ecs'
import { type Quaternion, type Vector3 } from '@dcl/sdk/math'
import { rotateVec3ByQuat, worldToScreenPx } from './perspective-to-screen'

type RuntimeModule = { getCameraFov?: () => Promise<number> }

let cachedFovY: number | null = null

async function refreshCameraFov(): Promise<void> {
  try {
    const runtime = (globalThis as any).require('~system/Runtime') as RuntimeModule
    if (runtime.getCameraFov === undefined) return
    const fov = await runtime.getCameraFov()
    if (Number.isFinite(fov) && fov > 0) {
      cachedFovY = fov
    }
  } catch (err) {
    console.error('failed to read camera fov', err)
  }
}

let started = false
export function startCameraProjection(): void {
  if (started) return
  started = true
  executeTask(async () => {
    await refreshCameraFov()
  })
  let elapsed = 0
  engine.addSystem((dt: number) => {
    elapsed += dt
    if (elapsed >= 1.5) {
      elapsed = 0
      void refreshCameraFov()
    }
  })
}

export function cameraFovY(): number | null {
  return cachedFovY
}

/** Unproject a normalized viewport point (0..1, origin top-left) through the
    editor camera onto the y=0 ground plane. Returns null when the camera or
    fov is not ready, or the point looks at/above the horizon. Inverse of
    worldToScreenPx with its defaults (forward = +Z). */
export function groundPointFromScreen(nx: number, ny: number): Vector3 | null {
  if (cachedFovY === null) return null
  const camT = Transform.getOrNull(engine.CameraEntity)
  if (camT === null) return null
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  const aspect = canvas !== null && canvas.height > 0 ? canvas.width / canvas.height : 16 / 9
  const tanHalf = Math.tan(cachedFovY / 2)
  const local = {
    x: (nx * 2 - 1) * tanHalf * aspect,
    y: (1 - ny * 2) * tanHalf,
    z: 1
  } as Vector3
  const dir = rotateVec3ByQuat(local, camT.rotation as Quaternion)
  if (dir.y > -1e-4) return null
  const t = -camT.position.y / dir.y
  if (!Number.isFinite(t) || t <= 0) return null
  return {
    x: camT.position.x + dir.x * t,
    y: 0,
    z: camT.position.z + dir.z * t
  } as Vector3
}

export function projectWorldToScreen(
  world: Vector3
): { left: number; top: number; onScreen: boolean; behind: boolean } | null {
  if (cachedFovY === null) return null
  const camT = Transform.getOrNull(engine.CameraEntity)
  if (camT === null) return null
  const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
  if (canvas === null || canvas.width <= 0 || canvas.height <= 0) return null

  return worldToScreenPx(
    world,
    camT.position,
    camT.rotation as Quaternion,
    cachedFovY,
    canvas.width,
    canvas.height,
    { boundOutOfScreen: true }
  )
}

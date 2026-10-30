import { engine, PointerLock, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { BevyApi } from './bevy-api'
import { setActiveAction, state } from './state'


const TAP_MOVE_THRESHOLD = 12
const UNLOCK_HOLD = 0.25

let held = false
let movedPx = 0
let unlockFor = 0

export function startSystemActions(): void {
  engine.addSystem((dt: number) => {
    if (state.playing) {
      held = false
      unlockFor = 0
      return
    }
    if (held) {
      const d = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
      if (d !== undefined) movedPx += Math.abs(d.x) + Math.abs(d.y)
    }
    if (unlockFor > 0) {
      unlockFor -= dt
      PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: false })
    }
  })
  listen().catch((e) => {
    console.error('system action stream ended', e)
  })
}

async function listen(): Promise<void> {
  const stream = await BevyApi.getSystemActionStream()
  for await (const ev of stream) {
    if (ev.action !== 'CameraLock') continue
    if (state.playing) continue
    if (ev.pressed) {
      held = true
      movedPx = 0
    } else {
      const tap = held && movedPx < TAP_MOVE_THRESHOLD
      held = false
      if (tap) {
        unlockFor = UNLOCK_HOLD
        setActiveAction('select')
      }
    }
  }
}

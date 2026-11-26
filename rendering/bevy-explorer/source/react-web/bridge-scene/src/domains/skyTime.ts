// The time of day for the Skybox menu, read from `~system/Runtime.getWorldTime` while the page
// watches it. The engine sends that state to every scene each frame, so a read is local.
import { getWorldTime } from '~system/Runtime'
import type { Ctx } from '../bridge'
import { throttleByDt } from '../system-helpers'

// `seconds` is all the SDK's typing knows; the engine reports the rest next to it.
type WorldTime = { seconds: number; locked?: boolean; speed?: number; followsClock?: boolean }

export function registerSkyTime(ctx: Ctx): void {
  let watching = false
  const read = (): void => {
    getWorldTime({})
      .then((time: WorldTime) => {
        // an engine that reports the hour alone cannot tell the menu who holds the sky
        if (!watching || typeof time.speed !== 'number') return
        ctx.send({
          kind: 'skyTime',
          state: { hours: time.seconds / 3600, locked: time.locked === true, speed: time.speed, followsClock: time.followsClock === true }
        })
      })
      .catch((e: unknown) => console.error('[skyTime] getWorldTime failed', e))
  }
  let everySecond = throttleByDt(1, read)
  ctx.push((dt) => {
    if (watching) everySecond(dt)
  })
  ctx.on('watchSkyTime', (msg) => {
    watching = msg.on
    // the first read now, the next a second after it
    everySecond = throttleByDt(1, read)
  })
  // a page that reloads has no menu open
  ctx.on('hello', () => {
    watching = false
  })
}

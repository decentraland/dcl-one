// Smart wearables: relays the engine's authorization requests (a smart wearable the player logged in
// wearing or just equipped wants to run) to the HUD and sends the player's answer back, and polls
// what each smart wearable is doing so the HUD can show and toast it. Stop and Restart go through the
// engine console (`/smart_wearable kill|restart <urn>`, domains/world.ts).
//   from: SystemApi.getSmartWearableAuthorizationStream / answerSmartWearableAuthorization /
//   smartWearableState.
import { BevyApi } from '../bevy-api'
import { relay, singleFlight, throttleByDt } from '../system-helpers'
import type { Ctx } from '../bridge'

const STATE_POLL_INTERVAL = 1

export function registerSmartWearables(ctx: Ctx): void {
  relay('smartWearableAuth', async () => await BevyApi.getSmartWearableAuthorizationStream(), (req) => {
    ctx.send({ kind: 'smartWearableAuthRequest', ...req })
  })

  ctx.on('smartWearableAuthAnswer', (msg) => {
    BevyApi.answerSmartWearableAuthorization(msg.id, msg.allow)
  })

  // The state changes rarely, so only a change crosses to the page. A failing poll is logged once,
  // not every second.
  let lastSent = ''
  let failing = false
  ctx.push(
    throttleByDt(
      STATE_POLL_INTERVAL,
      singleFlight(async () => {
        let state
        try {
          state = await BevyApi.smartWearableState()
        } catch (e) {
          if (!failing) console.error('[smartWearables] smartWearableState failed', e)
          failing = true
          return
        }
        failing = false
        const json = JSON.stringify(state)
        if (json === lastSent) return
        lastSent = json
        ctx.send({ kind: 'smartWearableState', entries: state.entries, currentSceneAllows: state.currentSceneAllows, currentSceneHidesUi: state.currentSceneHidesUi })
      })
    )
  )
}

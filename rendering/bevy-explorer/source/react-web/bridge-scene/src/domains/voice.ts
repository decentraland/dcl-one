// Voice activity: who is talking right now.
//   from: BevyApi.getVoiceStream() (LiveKit active-speaker changes, local player included)
//   to:   the page (Nearby list) + the nametag speaking badge (isSpeaking)
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

/** The engine's channel for the voice room (comms `VOICE_CHAT_CHANNEL`). */
const CALL_CHANNEL = 'voice-chat'

const nearby = new Set<string>()
const call = new Set<string>()
let nearbyBlocked = false
let send: Ctx['send'] | undefined

/** One rule for the nametag badge and the page: a call speaker always shows, a nearby one unless the scene blocks nearby voice. */
function shown(address: string): boolean {
  return call.has(address) || (!nearbyBlocked && nearby.has(address))
}

export function isSpeaking(address: string): boolean {
  return shown(address.toLowerCase())
}

/** Nearby speakers stop showing as speaking while the scene blocks nearby voice, and show again after. */
export function setNearbyVoiceBlocked(blocked: boolean): void {
  if (blocked === nearbyBlocked) return
  nearbyBlocked = blocked
  for (const address of nearby) {
    if (!call.has(address)) send?.({ kind: 'voiceActivity', address, active: !blocked })
  }
}

export function registerVoice(ctx: Ctx): void {
  send = ctx.send
  // A reopened stream only reports new changes, so whoever was talking is reset first.
  const open = async (): Promise<Awaited<ReturnType<typeof BevyApi.getVoiceStream>>> => {
    for (const address of new Set([...nearby, ...call])) {
      if (shown(address)) ctx.send({ kind: 'voiceActivity', address, active: false })
    }
    nearby.clear()
    call.clear()
    return await BevyApi.getVoiceStream()
  }
  // The engine reports each room a peer talks in separately, so the page hears only a change of the combined answer.
  relay('voice', open, (v) => {
    const address = v.sender_address.toLowerCase()
    const was = shown(address)
    const room = v.channel === CALL_CHANNEL ? call : nearby
    if (v.active) room.add(address)
    else room.delete(address)
    const active = shown(address)
    if (active !== was) ctx.send({ kind: 'voiceActivity', address, active })
  })
}

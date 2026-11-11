// Voice activity: who is talking right now.
//   from: BevyApi.getVoiceStream() (LiveKit active-speaker changes, local player included)
//   to:   the page (Nearby list) + the nametag speaking badge (isSpeaking)
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

/** The engine's channel for the voice room (comms `VOICE_CHAT_CHANNEL`). */
const CALL_CHANNEL = 'voice-chat'

const speaking = new Set<string>()
const inCall = new Set<string>()
let nearbyBlocked = false
let send: Ctx['send'] | undefined

export function isSpeaking(address: string): boolean {
  const a = address.toLowerCase()
  return speaking.has(a) && (!nearbyBlocked || inCall.has(a))
}

/** Nearby speakers stop showing as speaking while the scene blocks nearby voice, and show again after. */
export function setNearbyVoiceBlocked(blocked: boolean): void {
  if (blocked === nearbyBlocked) return
  nearbyBlocked = blocked
  for (const address of speaking) {
    if (!inCall.has(address)) send?.({ kind: 'voiceActivity', address, active: !blocked })
  }
}

export function registerVoice(ctx: Ctx): void {
  send = ctx.send
  // A reopened stream only reports new changes, so whoever was talking is reset first.
  const open = async (): Promise<Awaited<ReturnType<typeof BevyApi.getVoiceStream>>> => {
    for (const address of speaking) ctx.send({ kind: 'voiceActivity', address, active: false })
    speaking.clear()
    inCall.clear()
    return await BevyApi.getVoiceStream()
  }
  relay('voice', open, (v) => {
    const address = v.sender_address.toLowerCase()
    if (v.active) speaking.add(address)
    else speaking.delete(address)
    if (v.channel === CALL_CHANNEL) {
      if (v.active) inCall.add(address)
      else inCall.delete(address)
    }
    if (!nearbyBlocked || v.channel === CALL_CHANNEL) ctx.send({ kind: 'voiceActivity', address, active: v.active })
  })
}

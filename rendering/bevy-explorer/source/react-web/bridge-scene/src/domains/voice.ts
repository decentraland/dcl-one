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

export function isSpeaking(address: string): boolean {
  const a = address.toLowerCase()
  return speaking.has(a) && (!nearbyBlocked || inCall.has(a))
}

export function setNearbyVoiceBlocked(blocked: boolean): void {
  nearbyBlocked = blocked
}

export function registerVoice(ctx: Ctx): void {
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
    ctx.send({ kind: 'voiceActivity', address, active: v.active })
  })
}

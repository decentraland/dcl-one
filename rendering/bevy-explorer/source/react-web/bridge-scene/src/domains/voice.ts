// Voice activity: who is talking right now.
//   from: BevyApi.getVoiceStream() (LiveKit active-speaker changes, local player included)
//   to:   the page (Nearby list) + the nametag speaking badge (isSpeaking, isSpeakingInCall)
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

// comms `VOICE_CHAT_CHANNEL`: calls and community streams, which a nearby mute leaves audible.
const VOICE_CHAT_CHANNEL = 'voice-chat'

const speaking = new Map<string, string>()

export function isSpeaking(address: string): boolean {
  return speaking.has(address.toLowerCase())
}

export function isSpeakingInCall(address: string): boolean {
  return speaking.get(address.toLowerCase()) === VOICE_CHAT_CHANNEL
}

export function registerVoice(ctx: Ctx): void {
  // A reopened stream only reports new changes, so whoever was talking is reset first.
  const open = async (): Promise<Awaited<ReturnType<typeof BevyApi.getVoiceStream>>> => {
    for (const address of speaking.keys()) ctx.send({ kind: 'voiceActivity', address, active: false })
    speaking.clear()
    return await BevyApi.getVoiceStream()
  }
  relay('voice', open, (v) => {
    const address = v.sender_address.toLowerCase()
    if (v.active) speaking.set(address, v.channel)
    else speaking.delete(address)
    ctx.send({ kind: 'voiceActivity', address, active: v.active })
  })
}

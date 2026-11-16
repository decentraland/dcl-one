// Voice activity: who is talking right now.
//   from: BevyApi.getVoiceStream() (LiveKit active-speaker changes, local player included)
//   to:   the page (Nearby list)
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

const speaking = new Set<string>()

export function registerVoice(ctx: Ctx): void {
  // A reopened stream only reports new changes, so whoever was talking is reset first.
  const open = async (): Promise<Awaited<ReturnType<typeof BevyApi.getVoiceStream>>> => {
    for (const address of speaking) ctx.send({ kind: 'voiceActivity', address, active: false })
    speaking.clear()
    return await BevyApi.getVoiceStream()
  }
  relay('voice', open, (v) => {
    const address = v.sender_address.toLowerCase()
    if (v.active) speaking.add(address)
    else speaking.delete(address)
    ctx.send({ kind: 'voiceActivity', address, active: v.active })
  })
}

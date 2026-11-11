// Saved mutes for nearby voice: the wallets the user hushed.
//   from: BevyApi.getNearbyMuteStream() (the engine's list, whole on every change; the engine
//         reads and saves it on the social service)
//   to:   the page (the profile card's Hush / Hear) + the nametag hushed badge (isHushed)
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

const hushed = new Set<string>()

export function isHushed(address: string): boolean {
  return hushed.has(address.toLowerCase())
}

export function registerNearbyMutes(ctx: Ctx): void {
  const send = (): void => {
    ctx.send({ kind: 'nearbyMutes', addresses: [...hushed] })
  }

  ctx.on('setNearbyMute', (msg) => {
    // the page changed its copy before asking: a refusal, or an engine that cannot mute, puts
    // the engine's list back
    const set = BevyApi.setNearbyMuted
    if (set == null) {
      send()
      return
    }
    set(msg.address, msg.muted).catch((e: unknown) => {
      console.error('[nearbyMutes] set failed', e)
      send()
    })
  })
  // A page that starts late or reloads missed the list.
  ctx.on('hello', send)

  const open = BevyApi.getNearbyMuteStream
  if (open == null) return
  relay('nearbyMutes', open, (addresses) => {
    hushed.clear()
    for (const address of addresses) hushed.add(address.toLowerCase())
    send()
  })
}

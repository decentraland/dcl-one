// The map's People layer uses scene identities and positions filtered by last known Pulse membership.
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'
import { getPlayer } from '@dcl/sdk/players'
import type { MapPlayer } from '@explorer-core/engine/protocol'
import type { Ctx } from '../bridge'
import type { AvatarModifierSnapshot } from './avatarModifiers'
import { throttleByDt } from '../system-helpers'

const PLAYERS_INTERVAL = 0.25

export function registerMapPlayers(ctx: Ctx, avatarModifiers: () => AvatarModifierSnapshot): void {
  let watching = false
  let lastKey = ''
  ctx.on('watchMapPlayers', (msg) => {
    watching = msg.on
    lastKey = '\u0000'
  })
  // A page that starts over is not watching.
  ctx.on('hello', () => {
    watching = false
  })

  ctx.push(
    throttleByDt(PLAYERS_INTERVAL, () => {
      if (!watching) return
      const me = getPlayer()?.userId?.toLowerCase()
      const { pulseAbsent } = avatarModifiers()
      const players: MapPlayer[] = []
      for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
        const address = data.address.toLowerCase()
        if (address === '' || address === me || pulseAbsent.has(address)) continue
        const position = Transform.getOrNull(entity)?.position
        if (position == null) continue
        players.push({ address: data.address, x: position.x, z: position.z })
      }
      // sent when someone comes, goes or moves by a metre, not on every read
      const key = players.map((p) => `${p.address}:${Math.round(p.x)}:${Math.round(p.z)}`).join('|')
      if (key === lastKey) return
      lastKey = key
      ctx.send({ kind: 'mapPlayers', players })
    })
  )
}

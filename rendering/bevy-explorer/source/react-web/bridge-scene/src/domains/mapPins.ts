// The map's scene pins and nearby players.
//   pins:    BevyApi.getMapPinsStream — every running scene's MapPin components, the whole set each
//            time it changes.
//   players: the avatars the engine has in the world (PlayerIdentityData + Transform), read a few
//            times a second while the page's map shows its People layer.
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'
import { getPlayer } from '@dcl/sdk/players'
import type { MapPlayer } from '@explorer-core/engine/protocol'
import { BevyApi, type MapPinInfo } from '../bevy-api'
import type { Ctx } from '../bridge'
import { relay, throttleByDt } from '../system-helpers'

const PLAYERS_INTERVAL = 0.25

export function registerMapPins(ctx: Ctx): void {
  let pins: MapPinInfo[] = []
  relay('mapPins', async () => await BevyApi.getMapPinsStream(), (next) => {
    pins = next
    ctx.send({ kind: 'mapPins', pins })
  })

  let watching = false
  let lastKey = ''
  ctx.on('watchMapPlayers', (msg) => {
    watching = msg.on
    lastKey = '\u0000'
  })
  // A page that starts over holds no pins and is not watching.
  ctx.on('hello', () => {
    watching = false
    ctx.send({ kind: 'mapPins', pins })
  })

  ctx.push(
    throttleByDt(PLAYERS_INTERVAL, () => {
      if (!watching) return
      const me = getPlayer()?.userId?.toLowerCase()
      const players: MapPlayer[] = []
      for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
        if (data.address === '' || data.address.toLowerCase() === me) continue
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

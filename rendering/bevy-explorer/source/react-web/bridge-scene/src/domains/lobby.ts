// The lobby's signed reads and writes, and the client's quit (the profile menu's Exit): the events list (each event carries whether the player marked
// it interested only when the read is signed) and the interested mark itself.
import type { LobbyEvent } from '@explorer-core/features/lobby/lobbyEvents'
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { serviceBase, signed } from '../http'

export function registerLobby(ctx: Ctx): void {
  const events = async (): Promise<string> => `${await serviceBase('events')}/api/events`

  ctx.on('getLobbyEvents', async () => {
    try {
      const list = await signed<LobbyEvent[]>(`${await events()}?with_connected_users=true`)
      ctx.send({ kind: 'lobbyEvents', events: Array.isArray(list) ? list : null })
    } catch (e) {
      console.error('[lobby] events', e)
      ctx.send({ kind: 'lobbyEvents', events: null })
    }
  })

  ctx.on('quit', () => {
    BevyApi.quit?.()
  })

  ctx.on('setEventInterest', async (msg) => {
    try {
      await signed(`${await events()}/${encodeURIComponent(msg.id)}/attendees`, msg.interested ? 'POST' : 'DELETE')
      ctx.send({ kind: 'eventInterest', id: msg.id, interested: msg.interested, ok: true })
    } catch (e) {
      console.error('[lobby] event interest', e)
      ctx.send({ kind: 'eventInterest', id: msg.id, interested: !msg.interested, ok: false })
    }
  })
}

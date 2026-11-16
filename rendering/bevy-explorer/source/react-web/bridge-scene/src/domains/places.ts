// Signed, so the places service answers with the player's own like, dislike and favourite.
import type { SignedPlace } from '@explorer-core/engine/protocol'
import type { Ctx } from '../bridge'
import { serviceBase, signed } from '../http'

export function registerPlaces(ctx: Ctx): void {
  ctx.on('getPlace', async (msg) => {
    try {
      const url = `${await serviceBase('places')}/api/destinations?ids=${encodeURIComponent(msg.id)}&with_connected_users=true`
      const rows = await signed<SignedPlace[]>(url)
      ctx.send({ kind: 'place', requestId: msg.requestId, place: Array.isArray(rows) ? (rows.find((p) => p.id === msg.id) ?? null) : null })
    } catch (e) {
      console.error('[places] place read failed', e)
      ctx.send({ kind: 'place', requestId: msg.requestId, place: null })
    }
  })
}

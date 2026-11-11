import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import { relay } from '../system-helpers'
import type { ConnectivitySnapshot } from '../../../src/engine/generated'

export function registerConnectivity(ctx: Ctx): void {
  let latest: ConnectivitySnapshot | null = null
  const send = (): void => {
    if (latest != null) ctx.send({ kind: 'connectivity', state: latest })
  }
  relay('connectivity', async () => await BevyApi.getConnectivityStream(), state => {
    latest = state
    send()
  })
  // A page can join after the initial engine snapshot, or re-enter the world later.
  ctx.on('hello', send)
  ctx.on('getConnectivity', send)
  ctx.on('retryConnectivity', async ({ requestId }) => {
    try {
      await BevyApi.retryConnectivity()
      ctx.send({ kind: 'connectivityRetry', requestId })
    } catch (error) {
      ctx.send({ kind: 'connectivityRetry', requestId, error: error instanceof Error ? error.message : String(error) })
    }
  })
}

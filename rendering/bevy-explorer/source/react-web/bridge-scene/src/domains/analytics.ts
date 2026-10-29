// Analytics: the engine's own Segment events as it queues them (name + properties, no common
// fields), relayed so the page can act on one — today the Google Ads conversion on
// "First Move In World" (features/session/useEngineSession.ts). The page's events never come
// back this way; it sent them.
//   from: BevyApi.getAnalyticsEventStream.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerAnalytics(ctx: Ctx): void {
  relay('analytics', async () => await BevyApi.getAnalyticsEventStream(), (e) => {
    ctx.send({ kind: 'analytics', event: e.event, properties: e.properties })
  })
}

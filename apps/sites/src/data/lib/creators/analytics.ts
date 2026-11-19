import type { TelemetryEvents } from '@/core/lib/telemetry/events'
import { withCreatorFunnel } from '@/core/lib/telemetry/creator-funnel'
import type { TrackContext } from '@/core/lib/telemetry/track'
import { isWalletRejection, isWalletStateError } from './walletErrors'

const send = withCreatorFunnel()

type CreatorsEvent = Extract<keyof TelemetryEvents, `bd_${string}`>

// No session until the page mounts.
const NONE: TrackContext = { sid: '' }

let pageContext = NONE

/** The telemetry session and story of the creators page on screen. Browser only: the shell sets it on mount. */
export function setTrackContext(context: TrackContext | null): void {
  pageContext = context ?? NONE
}

/** Reports a step of a creators flow under the page's story; a call before the page mounts is dropped. */
export function track<K extends CreatorsEvent>(
  event: K,
  props: TelemetryEvents[K],
  context: TrackContext = pageContext
): void {
  if (context.sid) send(event, props, context)
}

/**
 * A coarse bucket for a failed flow. Never the raw message: it can carry ids, urls or wallet copy, and
 * free text doesn't aggregate. The domain errors (`PublishCollectionError`, `SellItemError`, …) already
 * carry the reason the UI branches on, so that is what gets reported.
 */
export function errorCode(error: unknown): string {
  const reason = (error as { reason?: unknown } | null)?.reason
  if (typeof reason === 'string' && reason !== '') return reason
  if (isWalletRejection(error)) return 'rejected'
  if (isWalletStateError(error)) return 'wallet'
  return 'unknown'
}

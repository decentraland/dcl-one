// The creators flows report their steps through `track`. The events are not sent yet: they join the Sites
// telemetry catalogue, with their final names, in a later change.
import { isWalletRejection, isWalletStateError } from '@/data/lib/creators/walletErrors'

type Props = Record<string, unknown>

export function track(event: string, props: Props = {}): void {
  if (import.meta.env.DEV) console.debug('[creators] track', event, props)
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

import { useEffect, useState } from 'react'
import { CreditsServerError, pollCreditsOrder, type CreditsOrderOutcome } from '~/lib/credits'
import { captureError } from '~/lib/monitoring'

export type OrderOutcomeState = { status: 'confirming' } | CreditsOrderOutcome

/**
 * Follows a credit-pack order after the Stripe return until the webhook settles it. A failed read is
 * shown as `pending`: the payment may well have gone through, and the balance query catches up on its
 * own. Anything but a network or server failure is a bug, so it is reported before being shown that way.
 */
export function useCreditsOrderOutcome(address: string, orderId: string): OrderOutcomeState {
  const [state, setState] = useState<OrderOutcomeState>({ status: 'confirming' })

  useEffect(() => {
    const controller = new AbortController()
    setState({ status: 'confirming' })
    pollCreditsOrder(address, orderId, { signal: controller.signal })
      .then(outcome => {
        if (!controller.signal.aborted) setState(outcome)
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return
        if (!(error instanceof CreditsServerError) && !(error instanceof TypeError)) {
          captureError(error, { orderId, flow: 'credits-top-up' })
        }
        setState({ status: 'pending' })
      })
    return () => controller.abort()
  }, [address, orderId])

  return state
}

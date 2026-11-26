// "Guest Wallet Creation" around the thirdweb guest wallet step, as mobile reports its own:
// outcome, how long it took, and on failure a bucket read off the error text, since the shell
// hands back a message, not a response.

import { track } from '../../lib/analytics'
import type { GuestWalletStep } from './guest'

export type GuestWalletFailure = { failure_reason: string; http_status?: number }

export function guestWalletFailure(error: unknown): GuestWalletFailure {
  const text = error instanceof Error ? error.message : String(error)
  const http = /thirdweb (\d{3})/.exec(text)
  if (http != null) {
    const status = Number(http[1])
    return { failure_reason: status >= 500 ? 'http_5xx' : 'http_4xx', http_status: status }
  }
  if (/no answer|timed? ?out/i.test(text)) return { failure_reason: 'timeout' }
  if (/fetch|network|load failed/i.test(text)) return { failure_reason: 'network' }
  if (/sign/i.test(text)) return { failure_reason: 'sign_message' }
  return { failure_reason: 'bad_response' }
}

// Only a fresh session id mints a new thirdweb guest; the kept one reaches the same guest, and a kept
// guest whose sign-in still runs makes no wallet call, so it reports nothing. A shell that does not
// say (`wallet` absent) always mints a new one.
export async function trackGuestWalletCreation<T extends { wallet?: GuestWalletStep }>(create: () => Promise<T>): Promise<T> {
  const started = performance.now()
  const duration = (): number => Math.round(performance.now() - started)
  try {
    const result = await create()
    if (result.wallet !== null) track('Guest Wallet Creation', { outcome: 'success', is_new_user: result.wallet !== 'existing', duration_ms: duration() })
    return result
  } catch (e) {
    track('Guest Wallet Creation', { outcome: 'failure', is_new_user: false, ...guestWalletFailure(e), duration_ms: duration() })
    throw e
  }
}

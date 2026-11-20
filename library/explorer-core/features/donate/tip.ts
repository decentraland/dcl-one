// A MANA tip's two shared parts: the ERC-20 call it makes, and telling the catalyst about it once mined (the
// catalyst then sends the creator `tip_received`), retried while that may still succeed. Used by the web
// explorer's scene tip and by the Sites `/play` shell's Donate.

/** The ERC-20 `transfer`, the one call a MANA tip makes. */
export const TRANSFER_ABI = [
  {
    type: 'function',
    name: 'transfer',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'to', type: 'address' },
      { name: 'amount', type: 'uint256' }
    ],
    outputs: [{ name: '', type: 'bool' }]
  }
] as const

const FIRST_RETRY_MS = 5_000
const MAX_RETRY_MS = 120_000

/** The body code of the catalyst's 503 for a node with no tip index: its claim could only expire. */
export const TIP_INDEX_UNAVAILABLE = 'tip-index-unavailable'

/**
 * How a forward ended: the catalyst took it, refused it (asking again changes nothing), answered that it has no tip
 * index (`TIP_INDEX_UNAVAILABLE`), or time ran out.
 */
export type ForwardOutcome = 'notified' | 'refused' | 'unavailable' | 'gave-up'

/** A failed forward's HTTP status (0 or none: no answer), the seconds the server asked to wait and its body code. */
function failure(e: unknown): { status: number; retryAfterSecs: number; code?: string } {
  const { status, retryAfterSecs, code } = (e ?? {}) as { status?: unknown; retryAfterSecs?: unknown; code?: unknown }
  return {
    status: typeof status === 'number' ? status : 0,
    retryAfterSecs: typeof retryAfterSecs === 'number' && retryAfterSecs > 0 ? retryAfterSecs : 0,
    code: typeof code === 'string' ? code : undefined
  }
}

/**
 * Forwards a mined tip until `deadline`. It retries what may succeed later (429, any 5xx, no answer) with backoff,
 * never sooner than the server's Retry-After. It stops at a 503 whose body code is `TIP_INDEX_UNAVAILABLE`, and at
 * any other 4xx. A failure carries its `status`, `retryAfterSecs` and `code` (as the catalyst client's errors do).
 * Never throws.
 */
export async function forwardUntil(
  hash: string,
  forward: (transactionHash: string) => Promise<void>,
  opts: { sleep?: (ms: number) => Promise<void>; now?: () => number; deadline: number }
): Promise<ForwardOutcome> {
  const sleep = opts.sleep ?? ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)))
  const now = opts.now ?? Date.now
  let delay = FIRST_RETRY_MS
  for (;;) {
    let hint = 0
    try {
      await forward(hash)
      return 'notified'
    } catch (e) {
      const { status, retryAfterSecs, code } = failure(e)
      if (status === 503 && code === TIP_INDEX_UNAVAILABLE) return 'unavailable'
      if (!(status === 0 || status === 429 || status >= 500)) return 'refused'
      hint = retryAfterSecs * 1000
    }
    // Never sooner than the server's Retry-After, never longer than the max.
    const wait = Math.min(Math.max(delay, hint), MAX_RETRY_MS)
    if (now() + wait > opts.deadline) return 'gave-up'
    await sleep(wait)
    delay = Math.min(delay * 2, MAX_RETRY_MS)
  }
}

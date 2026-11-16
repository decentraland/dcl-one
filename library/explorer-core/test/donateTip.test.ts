// A mined tip's forward to the catalyst (features/donate/tip): retried while it may still succeed, never sooner
// than the server's Retry-After, never after a refusal.
import { describe, expect, it, vi } from 'vitest'
import { forwardUntil } from '../features/donate/tip'

const HASH = '0x' + 'ab'.repeat(32)
const failed = (status: number, retryAfterSecs?: number) => Object.assign(new Error(`HTTP ${status}`), { status, retryAfterSecs })

function clock() {
  let t = 0
  const waits: number[] = []
  return { waits, now: () => t, sleep: async (ms: number) => void (waits.push(ms), (t += ms)) }
}

describe('forwarding a mined tip', () => {
  it('retries a 5xx, a 429 and no answer with backoff, until the catalyst takes it', async () => {
    const c = clock()
    const forward = vi.fn().mockRejectedValueOnce(failed(503)).mockRejectedValueOnce(new Error('offline')).mockResolvedValue(undefined)
    expect(await forwardUntil(HASH, forward, { ...c, deadline: 600_000 })).toBe('notified')
    expect(c.waits).toEqual([5_000, 10_000])
  })

  it('waits at least as long as the server’s Retry-After', async () => {
    const c = clock()
    const forward = vi.fn().mockRejectedValueOnce(failed(429, 30)).mockResolvedValue(undefined)
    await forwardUntil(HASH, forward, { ...c, deadline: 600_000 })
    expect(c.waits).toEqual([30_000])
  })

  it('stops at the catalyst’s refusal, and gives up at the deadline', async () => {
    const refused = vi.fn().mockRejectedValue(failed(400))
    expect(await forwardUntil(HASH, refused, { ...clock(), deadline: 600_000 })).toBe('refused')
    expect(refused).toHaveBeenCalledOnce()
    const down = vi.fn().mockRejectedValue(failed(502))
    expect(await forwardUntil(HASH, down, { ...clock(), deadline: 20_000 })).toBe('gave-up')
    // At 0, 5 s and 15 s; the next wait (20 s) would end past the deadline.
    expect(down).toHaveBeenCalledTimes(3)
  })
})

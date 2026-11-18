import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// The memory also lives for the page's lifetime, so every spec loads a fresh module (a fresh visit).
async function load() {
  vi.resetModules()
  return import('./reviewModalMemory')
}

beforeEach(() => {
  localStorage.clear()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('review modal memory', () => {
  it('remembers the verdict modal a creator saw, per collection', async () => {
    const { markReviewModalSeen, wasReviewModalSeen } = await load()
    markReviewModalSeen('c1', 'verdict:e1')

    expect(wasReviewModalSeen('c1', 'verdict:e1')).toBe(true)
    expect(wasReviewModalSeen('c2', 'verdict:e1')).toBe(false)
  })

  it('shows the next verdict again', async () => {
    const { markReviewModalSeen, wasReviewModalSeen } = await load()
    markReviewModalSeen('c1', 'verdict:e1')

    expect(wasReviewModalSeen('c1', 'verdict:e2')).toBe(false)
  })

  it('remembers it on the next visit through local storage', async () => {
    const first = await load()
    first.markReviewModalSeen('c1', 'verdict:e1')

    const next = await load()
    expect(next.wasReviewModalSeen('c1', 'verdict:e1')).toBe(true)
  })

  it('keeps a closed modal closed for the visit when the browser refuses to store it', async () => {
    const { markReviewModalSeen, wasReviewModalSeen } = await load()
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceededError')
    })

    expect(() => markReviewModalSeen('c3', 'verdict:e1')).not.toThrow()
    expect(wasReviewModalSeen('c3', 'verdict:e1')).toBe(true)
  })

  it('keeps a closed modal closed when the browser blocks storage altogether', async () => {
    const { markReviewModalSeen, wasReviewModalSeen } = await load()
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('Blocked', 'SecurityError')
    })
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('Blocked', 'SecurityError')
    })

    markReviewModalSeen('c9', 'handoff:e1')

    expect(wasReviewModalSeen('c9', 'handoff:e1')).toBe(true)
    expect(wasReviewModalSeen('c9', 'handoff:e2')).toBe(false)
  })
})

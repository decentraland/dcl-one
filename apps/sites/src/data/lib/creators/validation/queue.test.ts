import { describe, expect, it } from 'vitest'
import { MAX_CONCURRENT, limited } from './queue'

describe('validation queue', () => {
  it(`never runs more than ${MAX_CONCURRENT} checks at once, and runs them all`, async () => {
    let running = 0
    let peak = 0
    const task = async (value: number) => {
      running++
      peak = Math.max(peak, running)
      await new Promise(resolve => setTimeout(resolve, 5))
      running--
      return value
    }
    const values = await Promise.all(Array.from({ length: 6 }, (_, index) => limited(() => task(index))))
    expect(values).toEqual([0, 1, 2, 3, 4, 5])
    expect(peak).toBe(MAX_CONCURRENT)
  })

  it('drops a waiting check whose signal aborted before it started', async () => {
    const blockers = Array.from({ length: MAX_CONCURRENT }, () => limited(() => new Promise(r => setTimeout(r, 5))))
    const controller = new AbortController()
    let started = false
    const waiting = limited(async () => {
      started = true
    }, controller.signal)
    controller.abort()
    await expect(waiting).rejects.toThrow()
    await Promise.all(blockers)
    expect(started).toBe(false)
  })
})

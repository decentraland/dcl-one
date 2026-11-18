import { describe, expect, it } from 'vitest'
import { createLimiter } from './limiter'

const tick = () => new Promise(resolve => setTimeout(resolve, 2))

describe('createLimiter', () => {
  it('runs at most the given number of tasks at once and runs every one', async () => {
    const limited = createLimiter(2)
    let inFlight = 0
    let peak = 0
    const task = async (value: number) => {
      inFlight++
      peak = Math.max(peak, inFlight)
      await tick()
      inFlight--
      return value
    }
    const results = await Promise.all([1, 2, 3, 4, 5].map(value => limited(() => task(value))))
    expect(results).toEqual([1, 2, 3, 4, 5])
    expect(peak).toBe(2)
  })

  it('never lets a new task take the slot a finished one hands to a waiting task', async () => {
    const limited = createLimiter(1)
    let inFlight = 0
    let peak = 0
    const task = async () => {
      inFlight++
      peak = Math.max(peak, inFlight)
      await tick()
      inFlight--
    }
    const first = limited(task)
    const second = limited(task)
    await first
    await Promise.all([second, limited(task)])
    expect(peak).toBe(1)
  })

  it('frees the slot of a task that fails', async () => {
    const limited = createLimiter(1)
    await expect(limited(async () => Promise.reject(new Error('boom')))).rejects.toThrow('boom')
    await expect(limited(async () => 'next')).resolves.toBe('next')
  })

  it('drops a waiting task whose signal aborted before it started', async () => {
    const limited = createLimiter(1)
    const blocker = limited(tick)
    const controller = new AbortController()
    let started = false
    const waiting = limited(async () => {
      started = true
    }, controller.signal)
    controller.abort()
    await expect(waiting).rejects.toThrow()
    await blocker
    expect(started).toBe(false)
  })
})

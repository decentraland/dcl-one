import { describe, it, expect, vi } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
describe('when a fixture awaits a bounded browser state', () => {
  it('should retain a valid falsy sample without advancing a frame', async () => {
    const next = vi.fn()
    expect(
      await waitForFixture(
        'Count',
        async () => 0,
        (value) => value === 0,
        next,
        () => []
      )
    ).toBe(0)
    expect(next).not.toHaveBeenCalled()
  })
  it('should wait for both the native pseudo state and its expected paint', async () => {
    const values = [
      { active: false, background: 'pressed' },
      { active: true, background: 'normal' },
      { active: true, background: 'pressed' }
    ]
    const read = vi.fn(async () => values.shift()!),
      next = vi.fn(async () => {})
    expect(
      await waitForFixture(
        'Pressed',
        read,
        (v) => v.active && v.background === 'pressed',
        next,
        () => []
      )
    ).toEqual({ active: true, background: 'pressed' })
    expect(next).toHaveBeenCalledTimes(2)
  })
  it('should fail a missing target at its frame limit without an extra pending frame', async () => {
    const read = vi.fn(async () => false),
      next = vi.fn(async () => {})
    await expect(waitForFixture('Target', read, Boolean, next, () => [], 3)).rejects.toThrow(
      'Target did not settle within 3 frames'
    )
    expect(read).toHaveBeenCalledTimes(3)
    expect(next).toHaveBeenCalledTimes(2)
  })
  it('should report a render error before polling a target that cannot appear', async () => {
    const read = vi.fn()
    await expect(
      waitForFixture(
        'Target',
        read,
        Boolean,
        async () => {},
        () => ['Render rejected']
      )
    ).rejects.toThrow('Render rejected')
    expect(read).not.toHaveBeenCalled()
  })
  it('should refuse a valid-looking sample if its evaluation raised a new error', async () => {
    const errors: string[] = []
    await expect(
      waitForFixture(
        'Target',
        async () => {
          errors.push('Render rejected')
          return true
        },
        Boolean,
        async () => {},
        () => errors
      )
    ).rejects.toThrow('Render rejected')
  })
  it('should preserve probe failures without advancing more frames', async () => {
    const error = new Error('Context lost'),
      next = vi.fn()
    await expect(
      waitForFixture(
        'Target',
        async () => {
          throw error
        },
        Boolean,
        next,
        () => []
      )
    ).rejects.toBe(error)
    expect(next).not.toHaveBeenCalled()
  })
  it('should preserve a failed frame scheduler', async () => {
    const error = new Error('Session closed')
    await expect(
      waitForFixture(
        'Target',
        async () => false,
        Boolean,
        async () => {
          throw error
        },
        () => []
      )
    ).rejects.toBe(error)
  })
  it('should allow an old error slice without accepting a new navigation error', async () => {
    const errors = ['Old render failure']
    expect(
      await waitForFixture(
        'Target',
        async () => true,
        Boolean,
        async () => {},
        () => errors.slice(1)
      )
    ).toBe(true)
    errors.push('New render failure')
    await expect(
      waitForFixture(
        'Target',
        async () => true,
        Boolean,
        async () => {},
        () => errors.slice(1)
      )
    ).rejects.toThrow('New render failure')
  })
  it.each([0, -1, 1.5, 121, Infinity, NaN])(
    'should reject an unsupported frame limit %s',
    async (limit) => {
      await expect(
        waitForFixture(
          'Target',
          async () => true,
          Boolean,
          async () => {},
          () => [],
          limit
        )
      ).rejects.toThrow('Fixture frame limit')
    }
  )
})

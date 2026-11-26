import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'

type PaintSample = { active: boolean; background: string }

describe('when a fixture awaits a bounded browser state', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and the first sample is a valid falsy value', () => {
    let next: Mock<() => Promise<unknown>>

    beforeEach(() => {
      next = vi.fn()
    })

    it('should resolve with the falsy sample without advancing a frame', async () => {
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
  })

  describe('and the native pseudo state and its paint settle on different frames', () => {
    let read: Mock<() => Promise<PaintSample>>
    let next: Mock<() => Promise<unknown>>

    beforeEach(() => {
      read = vi
        .fn<() => Promise<PaintSample>>()
        .mockResolvedValueOnce({ active: false, background: 'pressed' })
        .mockResolvedValueOnce({ active: true, background: 'normal' })
        .mockResolvedValueOnce({ active: true, background: 'pressed' })
      next = vi.fn(async () => {})
    })

    it('should wait for both the pseudo state and its expected paint', async () => {
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
  })

  describe('and the target never appears', () => {
    let read: Mock<() => Promise<boolean>>
    let next: Mock<() => Promise<unknown>>

    beforeEach(() => {
      read = vi.fn(async () => false)
      next = vi.fn(async () => {})
    })

    it('should fail at its frame limit without an extra pending frame', async () => {
      await expect(waitForFixture('Target', read, Boolean, next, () => [], 3)).rejects.toThrow(
        'Target did not settle within 3 frames'
      )
      expect(read).toHaveBeenCalledTimes(3)
      expect(next).toHaveBeenCalledTimes(2)
    })
  })

  describe('and a render error was already recorded', () => {
    let read: Mock<() => Promise<unknown>>

    beforeEach(() => {
      read = vi.fn()
    })

    it('should report the render error before polling the target', async () => {
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
  })

  describe('and evaluating a valid-looking sample raises a new error', () => {
    let errors: string[]
    let read: () => Promise<boolean>

    beforeEach(() => {
      errors = []
      read = async () => {
        errors.push('Render rejected')
        return true
      }
    })

    it('should refuse the sample with the new error', async () => {
      await expect(
        waitForFixture(
          'Target',
          read,
          Boolean,
          async () => {},
          () => errors
        )
      ).rejects.toThrow('Render rejected')
    })
  })

  describe('and the probe fails', () => {
    let error: Error
    let next: Mock<() => Promise<unknown>>

    beforeEach(() => {
      error = new Error('Context lost')
      next = vi.fn()
    })

    it('should reject with the probe failure without advancing more frames', async () => {
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
  })

  describe('and the frame scheduler fails', () => {
    let error: Error

    beforeEach(() => {
      error = new Error('Session closed')
    })

    it('should reject with the scheduler failure', async () => {
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
  })

  describe('and the runtime error slice starts after an old error', () => {
    let errors: string[]

    beforeEach(() => {
      errors = ['Old render failure']
    })

    it('should allow the old error', async () => {
      expect(
        await waitForFixture(
          'Target',
          async () => true,
          Boolean,
          async () => {},
          () => errors.slice(1)
        )
      ).toBe(true)
    })

    describe('and a new navigation error is recorded', () => {
      beforeEach(() => {
        errors.push('New render failure')
      })

      it('should reject with the new error', async () => {
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
    })
  })

  describe.each([0, -1, 1.5, 121, Infinity, NaN])('and the frame limit is %s', (limit) => {
    it('should reject the unsupported frame limit', async () => {
      await expect(
        waitForFixture(
          'Target',
          async () => true,
          Boolean,
          async () => {},
          () => [],
          limit
        )
      ).rejects.toThrow('Fixture frame limit must be an integer between 1 and 120')
    })
  })
})

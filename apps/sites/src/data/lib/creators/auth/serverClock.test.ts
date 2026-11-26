// @vitest-environment jsdom
import '../test/setup'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { clockUrl, resetServerClock, resyncServerClock, serverClockOffset, serverNow } from './serverClock'

const NOW = 1_794_700_000_000

function statusResponse(currentTime: unknown, ok = true) {
  return { ok, json: () => Promise.resolve({ currentTime }) }
}

describe('serverClock', () => {
  let fetchMock: ReturnType<typeof vi.fn<(url: string) => Promise<unknown>>>

  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(NOW)
    fetchMock = vi.fn<(url: string) => Promise<unknown>>()
    vi.stubGlobal('fetch', fetchMock)
  })

  afterEach(() => {
    resetServerClock()
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  describe('when no offset was measured', () => {
    it('should sign with the device clock', () => {
      expect(serverNow()).toBe(NOW)
    })
  })

  describe('when the device clock is hours behind the server', () => {
    const behindMs = 9_291_481 // the drift in BUILDER-113: ~2.6 hours

    beforeEach(async () => {
      fetchMock.mockResolvedValue(statusResponse(NOW + behindMs))
      await resyncServerClock()
    })

    it('should sign with the server time', () => {
      expect(serverNow()).toBe(NOW + behindMs)
    })

    describe('and another request is refused within a minute', () => {
      beforeEach(async () => {
        vi.advanceTimersByTime(20_000)
        await resyncServerClock()
      })

      it('should reuse the measurement instead of reading the server time again', () => {
        expect(fetchMock).toHaveBeenCalledTimes(1)
      })
    })

    describe('and another request is refused more than a minute later', () => {
      beforeEach(async () => {
        vi.advanceTimersByTime(61_000)
        await resyncServerClock()
      })

      it('should read the server time again', () => {
        expect(fetchMock).toHaveBeenCalledTimes(2)
      })
    })
  })

  describe('when the device clock is ahead of the server', () => {
    beforeEach(async () => {
      fetchMock.mockResolvedValue(statusResponse(NOW - 3_600_000))
      await resyncServerClock()
    })

    it('should sign with the server time', () => {
      expect(serverNow()).toBe(NOW - 3_600_000)
    })
  })

  describe('when the drift fits in the signed-fetch window', () => {
    beforeEach(async () => {
      fetchMock.mockResolvedValue(statusResponse(NOW + 20_000))
      await resyncServerClock()
    })

    it('should leave the device clock uncorrected', () => {
      expect(serverClockOffset()).toBe(0)
    })
  })

  describe.each([
    ['the status request fails', () => Promise.reject(new TypeError('Failed to fetch'))],
    ['the server answers an error', () => Promise.resolve(statusResponse(NOW + 3_600_000, false))],
    ['the status carries no time', () => Promise.resolve(statusResponse('soon'))],
    ['the status time is implausibly far off', () => Promise.resolve(statusResponse(NOW + 365 * 24 * 3_600_000))]
  ])('when %s', (_name, respond) => {
    beforeEach(async () => {
      fetchMock.mockImplementation(respond)
      await resyncServerClock()
    })

    it('should keep signing with the device clock', () => {
      expect(serverNow()).toBe(NOW)
    })

    describe('and another request is refused within a minute', () => {
      beforeEach(async () => {
        vi.advanceTimersByTime(20_000)
        await resyncServerClock()
      })

      it('should not read the server time again', () => {
        expect(fetchMock).toHaveBeenCalledTimes(1)
      })
    })
  })

  describe('when several refused requests resync at once', () => {
    beforeEach(async () => {
      fetchMock.mockResolvedValue(statusResponse(NOW - 3_600_000))
      await Promise.all([resyncServerClock(), resyncServerClock(), resyncServerClock()])
    })

    it('should read the server time once', () => {
      expect(fetchMock).toHaveBeenCalledTimes(1)
    })
  })
})

describe('when choosing the clock to measure against', () => {
  afterEach(() => {
    delete (window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__
  })

  describe('and the page names no node', () => {
    it("should be the lambdas status of the node behind the page's origin, whose clock checks the signatures", () => {
      expect(clockUrl()).toBe(`${window.location.origin}/lambdas/status`)
    })
  })

  describe('and the page names a node', () => {
    it("should be that node's lambdas status", () => {
      ;(window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__ = 'https://decentraland.zone'
      expect(clockUrl()).toBe('https://decentraland.zone/lambdas/status')
    })
  })
})

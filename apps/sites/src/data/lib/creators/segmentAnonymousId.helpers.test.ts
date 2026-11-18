import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
// @vitest-environment-options {"url":"https://decentraland.org/"}
import { createAnonymousIdResolver } from './segmentAnonymousId.helpers'

const cookieDescriptor = Object.getOwnPropertyDescriptor(Document.prototype, 'cookie')!
const COOKIE_ID = '11111111-1111-4111-8111-111111111111'
const LOCAL_ID = '22222222-2222-4222-8222-222222222222'

describe('when resolving the browser anonymous identity', () => {
  let originalRandomUUID: Crypto['randomUUID']
  let resolver: ReturnType<typeof createAnonymousIdResolver>
  let getSdkId: ReturnType<typeof vi.fn<() => string | undefined>>

  beforeEach(() => {
    originalRandomUUID = crypto.randomUUID
    getSdkId = vi.fn(() => undefined)
    resolver = createAnonymousIdResolver(getSdkId)
  })

  afterEach(() => {
    vi.restoreAllMocks()
    Object.defineProperty(document, 'cookie', cookieDescriptor)
    Object.defineProperty(crypto, 'randomUUID', { value: originalRandomUUID, configurable: true })
    localStorage.clear()
    document.cookie = 'ajs_anonymous_id=; path=/; max-age=0'
    document.cookie = 'ajs_anonymous_id=; domain=.decentraland.org; path=/; max-age=0'
  })

  describe('and there is no identity yet', () => {
    let cookieWrites: ReturnType<typeof vi.spyOn>
    beforeEach(() => {
      cookieWrites = vi.spyOn(document, 'cookie', 'set')
    })
    it('should persist on the writable parent and remove the probe cookie', () => {
      resolver.ensure()
      expect(cookieWrites).toHaveBeenCalledWith(
        expect.stringMatching(/^ajs_anonymous_id=.*; domain=\.decentraland\.org$/)
      )
      expect(cookieWrites).toHaveBeenCalledWith(
        expect.stringMatching(/^__dcl_segment_domain__[a-z0-9-]+=; domain=\.decentraland\.org; path=\/; max-age=0$/)
      )
      expect(document.cookie).not.toContain('__dcl_segment_domain__')
    })
  })

  describe('and only a parent cookie exists', () => {
    beforeEach(() => {
      document.cookie = `ajs_anonymous_id=${COOKIE_ID}; domain=.decentraland.org; path=/`
    })

    it('should reuse it before Segment boots and synchronize localStorage', () => {
      expect(resolver.ensure()).toBe(COOKIE_ID)
      expect(localStorage.getItem('ajs_anonymous_id')).toBe(JSON.stringify(COOKIE_ID))
    })

    describe('and localStorage disagrees', () => {
      beforeEach(() => localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID)))

      it('should use the cookie for beacons', () => {
        expect(resolver.ensure()).toBe(COOKIE_ID)
        expect(localStorage.getItem('ajs_anonymous_id')).toBe(JSON.stringify(COOKIE_ID))
      })
    })

    describe('and localStorage is blocked', () => {
      beforeEach(() => {
        vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
          throw new Error('blocked')
        })
        vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
          throw new Error('blocked')
        })
      })

      it('should still use the cookie for every call', () => {
        expect([resolver.ensure(), resolver.ensure()]).toEqual([COOKIE_ID, COOKIE_ID])
      })
    })

    describe('and the SDK has a newly reset identity', () => {
      beforeEach(() => {
        getSdkId.mockReturnValue('custom-sdk-id')
      })

      it('should honor the loaded SDK and persist its identity in both stores', () => {
        expect(resolver.ensure()).toBe('custom-sdk-id')
        expect(document.cookie).toContain('ajs_anonymous_id=custom-sdk-id')
        expect(localStorage.getItem('ajs_anonymous_id')).toBe('"custom-sdk-id"')
      })
    })

    describe('and reading the SDK fails', () => {
      beforeEach(() => {
        getSdkId.mockImplementation(() => {
          throw new Error('not ready')
        })
      })

      it('should still resolve the cookie', () => expect(resolver.ensure()).toBe(COOKIE_ID))
    })
  })

  describe.each([
    [JSON.stringify(LOCAL_ID), LOCAL_ID],
    [LOCAL_ID, LOCAL_ID],
    ['custom-storage-id', 'custom-storage-id']
  ])('and only localStorage contains %s', (raw, expected) => {
    beforeEach(() => localStorage.setItem('ajs_anonymous_id', raw))

    it('should adopt the stored identity and make it available to other subdomains', () => {
      expect(resolver.ensure()).toBe(expected)
      expect(document.cookie).toContain(`ajs_anonymous_id=${encodeURIComponent(resolver.ensure())}`)
    })
  })

  describe.each(['', '""', 'null', '{}'])('and stores contain an absent or non-string id %s', raw => {
    beforeEach(() => {
      localStorage.setItem('ajs_anonymous_id', raw)
      document.cookie = `ajs_anonymous_id=${encodeURIComponent(raw)}; path=/`
    })

    it('should mint a UUID and persist raw cookie and JSON localStorage values', () => {
      expect(resolver.ensure()).toMatch(/^[0-9a-f-]{36}$/i)
      expect(localStorage.getItem('ajs_anonymous_id')).toBe(JSON.stringify(resolver.ensure()))
      expect(document.cookie).toContain(`ajs_anonymous_id=${resolver.ensure()}`)
    })
  })

  describe.each(['0', '123', '-42', '1.5'])('and a legacy numeric identity %s is stored', raw => {
    beforeEach(() => {
      vi.spyOn(document, 'cookie', 'get').mockReturnValue(`ajs_anonymous_id=${raw}`)
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should preserve the cookie identity as a string and synchronize localStorage', () => {
      expect(resolver.ensure()).toBe(raw)
      expect(localStorage.getItem('ajs_anonymous_id')).toBe(JSON.stringify(raw))
    })
  })

  describe('and the cookie is JSON encoded', () => {
    beforeEach(() => {
      document.cookie = `ajs_anonymous_id=${encodeURIComponent(JSON.stringify(COOKIE_ID))}; path=/`
    })
    it('should decode and reuse the existing id', () => expect(resolver.ensure()).toBe(COOKIE_ID))
  })

  describe.each([
    ['%ZZ', '%ZZ'],
    ['%41%ZZ', 'A%ZZ']
  ])('and cookie percent escapes are only partly valid: %s', (raw, expected) => {
    beforeEach(() => {
      vi.spyOn(document, 'cookie', 'get').mockReturnValue(`ajs_anonymous_id=${raw}`)
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should match js-cookie decoding and keep the cookie identity', () => expect(resolver.ensure()).toBe(expected))
  })

  describe('and a cookie has invalid UTF-8 escapes', () => {
    beforeEach(() => {
      document.cookie = 'ajs_anonymous_id=%FF; path=/'
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should ignore the unreadable cookie like js-cookie', () => expect(resolver.ensure()).toBe(LOCAL_ID))
  })

  describe('and cookie access throws', () => {
    beforeEach(() => {
      vi.spyOn(document, 'cookie', 'get').mockImplementation(() => {
        throw new Error('sandboxed')
      })
      vi.spyOn(document, 'cookie', 'set').mockImplementation(() => {
        throw new Error('sandboxed')
      })
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should retain the localStorage identity', () => expect(resolver.ensure()).toBe(LOCAL_ID))
  })

  describe('and previously persisted identity has been removed from both stores', () => {
    beforeEach(() => {
      localStorage.setItem('ajs_anonymous_id', JSON.stringify('removed-custom-id'))
      resolver.ensure()
      localStorage.clear()
      document.cookie = 'ajs_anonymous_id=; domain=.decentraland.org; path=/; max-age=0'
    })
    it('should mint a new identity instead of reviving a deleted persisted identity', () => {
      expect(resolver.ensure()).toMatch(/^[0-9a-f-]{36}$/i)
    })
  })

  describe('and neither store is usable', () => {
    beforeEach(() => {
      vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
        throw new Error('blocked')
      })
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('blocked')
      })
      vi.spyOn(document, 'cookie', 'set').mockImplementation(() => undefined)
    })
    it('should keep one anonymous identity for every event in the page', () => {
      expect(resolver.ensure()).toBe(resolver.ensure())
    })
  })

  describe('and crypto.randomUUID is unavailable', () => {
    beforeEach(() => {
      Object.defineProperty(crypto, 'randomUUID', { value: undefined, configurable: true })
    })
    it('should generate a UUID v4', () =>
      expect(resolver.ensure()).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i))
  })

  describe.each([
    ['"null"', LOCAL_ID],
    ['"a%22b"', 'a"b']
  ])('and a cookie is quoted: %s', (raw, expected) => {
    beforeEach(() => {
      vi.spyOn(document, 'cookie', 'get').mockReturnValue(`ajs_anonymous_id=${raw}`)
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should match js-cookie quote removal before JSON parsing', () => expect(resolver.ensure()).toBe(expected))
  })

  describe('and the first duplicate cookie has invalid UTF-8', () => {
    beforeEach(() => {
      vi.spyOn(document, 'cookie', 'get').mockReturnValue(`ajs_anonymous_id=%E0%A4; ajs_anonymous_id=${COOKIE_ID}`)
      localStorage.setItem('ajs_anonymous_id', JSON.stringify(LOCAL_ID))
    })
    it('should skip the invalid entry and reuse the next readable cookie', () =>
      expect(resolver.ensure()).toBe(COOKIE_ID))
  })

  describe('and cookies are silently blocked', () => {
    let writes: ReturnType<typeof vi.spyOn>
    let now: ReturnType<typeof vi.spyOn>
    let probeCount: number
    beforeEach(() => {
      writes = vi.spyOn(document, 'cookie', 'set').mockImplementation(() => undefined)
      now = vi.spyOn(Date, 'now').mockReturnValue(0)
      resolver.ensure()
      probeCount = writes.mock.calls.filter(([value]: unknown[]) =>
        String(value).startsWith('__dcl_segment_domain__')
      ).length
    })
    it('should avoid repeated probing for immediate retries', () => {
      resolver.ensure()
      expect(
        writes.mock.calls.filter(([value]: unknown[]) => String(value).startsWith('__dcl_segment_domain__')).length
      ).toBe(probeCount)
    })
    it('should recover after the probe cache expires and cookies become writable', () => {
      writes.mockRestore()
      now.mockReturnValue(6000)
      expect(document.cookie).not.toContain('ajs_anonymous_id')
      resolver.ensure()
      expect(document.cookie).toContain('ajs_anonymous_id=')
    })
  })

  describe('and an old probe cookie remains', () => {
    beforeEach(() => {
      document.cookie = '__dcl_segment_domain__=stale; path=/'
    })
    afterEach(() => {
      document.cookie = '__dcl_segment_domain__=; path=/; max-age=0'
    })
    it('should still write identity on the shared parent domain', () => {
      const writes = vi.spyOn(document, 'cookie', 'set')
      resolver.ensure()
      expect(writes).toHaveBeenCalledWith(expect.stringMatching(/^ajs_anonymous_id=.*; domain=\.decentraland\.org$/))
      expect(writes).toHaveBeenCalledWith(expect.stringMatching(/__dcl_segment_domain__[^=]+=.*max-age=5/))
      expect(writes).toHaveBeenCalledWith(
        expect.stringMatching(/__dcl_segment_domain__[^=]+=; domain=\.decentraland\.org; path=\/; max-age=0/)
      )
    })
  })
})

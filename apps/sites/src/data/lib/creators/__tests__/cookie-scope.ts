import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest'

export function testCookieScope(createResolver: () => { ensure(): string }, domain?: string): void {
  describe(`when creating browser identity on ${window.location.hostname}`, () => {
    let writes: MockInstance<(value: string) => void>
    let id: string
    let identityWrite: string | undefined
    beforeEach(() => {
      writes = vi.spyOn(document, 'cookie', 'set')
      id = createResolver().ensure()
      identityWrite = writes.mock.calls.find(([value]) => String(value).startsWith('ajs_anonymous_id='))?.[0]
    })
    afterEach(() => {
      vi.restoreAllMocks()
      localStorage.clear()
      document.cookie = 'ajs_anonymous_id=; path=/; max-age=0'
      if (domain) document.cookie = `ajs_anonymous_id=; domain=.${domain}; path=/; max-age=0`
    })
    it('should persist on the writable scope without sharing through a public suffix', () => {
      expect(document.cookie).toContain(`ajs_anonymous_id=${id}`)
      if (domain) expect(identityWrite).toContain(`domain=.${domain}`)
      else expect(identityWrite).not.toContain('domain=')
      expect(document.cookie).not.toContain('__dcl_segment_domain__')
    })
  })
}

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fetchIndexedThirdPartySlots } from './indexedThirdPartySlots'

describe('fetchIndexedThirdPartySlots', () => {
  let id: string
  let fetchProvider: ReturnType<typeof vi.fn<typeof fetch>>

  beforeEach(() => {
    id = 'urn:decentraland:matic:collections-thirdparty:provider'
    fetchProvider = vi.fn<typeof fetch>()
    vi.stubGlobal('fetch', fetchProvider)
    vi.stubEnv('CATALYST_URL', 'https://node.example')
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.unstubAllEnvs()
  })

  describe('when Catalyst has indexed the provider', () => {
    beforeEach(() => {
      fetchProvider.mockResolvedValueOnce(Response.json({ id, maxItems: '10', availableSlots: '7' }))
    })

    it('should decode the decimal slots returned by the Catalyst provider route', async () => {
      await expect(fetchIndexedThirdPartySlots(id)).resolves.toEqual({ maxItems: 10, availableSlots: 7 })
      expect(fetchProvider).toHaveBeenCalledWith(
        'https://node.example/lambdas/third-party-integrations/providers/urn%3Adecentraland%3Amatic%3Acollections-thirdparty%3Aprovider',
        expect.objectContaining({ cache: 'no-store' })
      )
    })
  })

  describe('when the index is backfilling', () => {
    beforeEach(() => {
      fetchProvider.mockResolvedValueOnce(new Response(null, { status: 503 }))
    })

    it('should indicate that indexed slots are unavailable', async () => {
      await expect(fetchIndexedThirdPartySlots(id)).resolves.toBeUndefined()
    })
  })

  describe.each([429, 500])('when Catalyst returns HTTP %s', status => {
    beforeEach(() => {
      fetchProvider.mockResolvedValueOnce(new Response(null, { status }))
    })

    it('should indicate that indexed slots are unavailable', async () => {
      await expect(fetchIndexedThirdPartySlots(id)).resolves.toBeUndefined()
    })
  })

  describe('when Catalyst returns an unsafe slot count', () => {
    beforeEach(() => {
      fetchProvider.mockResolvedValueOnce(Response.json({ id, maxItems: '9007199254740993', availableSlots: '7' }))
    })

    it('should allow the caller to use the live count', async () => {
      await expect(fetchIndexedThirdPartySlots(id)).resolves.toBeUndefined()
    })
  })
})

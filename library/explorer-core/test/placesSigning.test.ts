// A signed Places request signs the path the host's gateway forwards (`gatewaySignedPath`), the
// one the engine signs for the same url.
import { afterEach, describe, expect, it, vi } from 'vitest'

const signFetch = vi.fn(async () => ({ 'x-identity-timestamp': '1' }))
vi.mock('@hud-host', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  serviceUrl: () => 'https://gateway.example.test/places',
  gatewaySignedPath: (url: string) => (url.startsWith('https://gateway.example.test/places/') ? '/api/destinations' : null)
}))
vi.mock('../features/editorHost/host/sign', () => ({ signFetch, localSigner: () => vi.fn() }))

afterEach(() => {
  vi.unstubAllGlobals()
  signFetch.mockClear()
})

describe('signed Places requests', () => {
  it("sign the path the host's gateway forwards", async () => {
    const fetch = vi.fn(async () => new Response(JSON.stringify({ ok: true, data: [], total: 0 })))
    vi.stubGlobal('fetch', fetch)
    const { fetchDestinations } = await import('../features/places/placesApi')
    const login = { identity: {}, guest: false } as unknown as Parameters<typeof fetchDestinations>[1]
    await fetchDestinations({ only_favorites: true }, login)
    const url = (fetch.mock.calls[0] as unknown as [string])[0]
    expect(url.startsWith('https://gateway.example.test/places/api/destinations?')).toBe(true)
    expect(signFetch).toHaveBeenCalledWith(login!.identity, 'GET', url, expect.any(Function), {}, '/api/destinations')
  })
})

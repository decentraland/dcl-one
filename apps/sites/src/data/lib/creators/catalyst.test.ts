// @vitest-environment jsdom
import './test/setup'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchCatalystContent, fetchEntitiesByPointers, peerUrl } from './catalyst'

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)

afterEach(() => fetchMock.mockReset())

describe('fetchEntitiesByPointers', () => {
  it('asks the content server for the active entities under the pointers', async () => {
    const entity = { id: 'bafy1', pointers: ['urn:1'] }
    fetchMock.mockResolvedValue(new Response(JSON.stringify([entity]), { status: 200 }))

    await expect(fetchEntitiesByPointers(['urn:1', 'urn:2'])).resolves.toEqual([entity])
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe(`${window.location.origin}/content/entities/active`)
    expect(init.method).toBe('POST')
    expect(JSON.parse(init.body as string)).toEqual({ pointers: ['urn:1', 'urn:2'] })
  })

  it('answers nothing without pointers, without asking the server', async () => {
    await expect(fetchEntitiesByPointers([])).resolves.toEqual([])
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('fails when the server does', async () => {
    fetchMock.mockResolvedValue(new Response('nope', { status: 500 }))
    await expect(fetchEntitiesByPointers(['urn:1'])).rejects.toThrow(/500/)
  })
})

describe('fetchCatalystContent', () => {
  it('downloads a deployed file by hash', async () => {
    fetchMock.mockResolvedValue(new Response('glb-bytes', { status: 200 }))
    const blob = await fetchCatalystContent('Qmhash')
    expect(fetchMock).toHaveBeenCalledWith(`${window.location.origin}/content/contents/Qmhash`)
    await expect(blob.text()).resolves.toBe('glb-bytes')
  })

  it('fails when the file is missing', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 404 }))
    await expect(fetchCatalystContent('Qmmissing')).rejects.toThrow(/404/)
  })
})

describe('fetchBaseWearables', () => {
  const wearable = (id: string, category: string, bodyShapes: string[], data: Record<string, unknown> = {}) => ({
    id,
    data: { category, representations: [{ bodyShapes }], ...data }
  })

  it('reads the base-avatars catalog and drops wearables that hide or replace anything', async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          wearables: [
            wearable('urn:decentraland:off-chain:base-avatars:f_jeans_00', 'lower_body', [
              'urn:decentraland:off-chain:base-avatars:BaseFemale'
            ]),
            wearable(
              'urn:decentraland:off-chain:base-avatars:hat',
              'hat',
              ['urn:decentraland:off-chain:base-avatars:BaseMale'],
              {
                hides: ['hair']
              }
            ),
            wearable(
              'urn:decentraland:off-chain:base-avatars:mask',
              'mask',
              ['urn:decentraland:off-chain:base-avatars:BaseMale'],
              {
                replaces: ['eyewear']
              }
            )
          ]
        }),
        { status: 200 }
      )
    )
    const { fetchBaseWearables } = await import('./catalyst')
    await expect(fetchBaseWearables()).resolves.toEqual([
      {
        urn: 'urn:decentraland:off-chain:base-avatars:f_jeans_00',
        category: 'lower_body',
        bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseFemale'],
        name: 'Jeans'
      }
    ])
    expect(fetchMock.mock.calls[0][0]).toBe(
      `${window.location.origin}/lambdas/collections/wearables?collectionId=urn:decentraland:off-chain:base-avatars`
    )
  })
})

describe('peerUrl', () => {
  describe('when the page is served by this deployment, as on localhost or its own domain', () => {
    afterEach(() => {
      delete (window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__
    })

    it("should be the node behind the page's origin, which serves content and lambdas at its root", () => {
      expect(peerUrl()).toBe(window.location.origin)
      expect(peerUrl('dcl.example')).toBe(window.location.origin)
    })

    it('should be the public node base when the page names one', () => {
      ;(window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__ = 'https://node.example/'
      expect(peerUrl()).toBe('https://node.example')
    })

    it.each(['dcl.dev', 'dcl.live'])('should be the node on a %s page', host => {
      ;(window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__ = `https://${host}`
      expect(peerUrl(host)).toBe(`https://${host}`)
    })
  })

  describe('when the page is on a Decentraland host', () => {
    it("should be that environment's peer", () => {
      expect(peerUrl('decentraland.zone')).toBe('https://peer.decentraland.zone')
    })
  })
})

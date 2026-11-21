// @vitest-environment jsdom
import '../test/setup'
import { rejection } from '../test/rejection'
import { describe, it, expect, afterEach, beforeEach, vi, type Mock } from 'vitest'
import { BuilderServerError, baseUrl, isRefusedRequest, request } from './transport'

vi.mock('../auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '../auth'
import { ADDRESS, jsonResponse, okResponse } from '../test/builderServer'

const signedFetchMock = signedFetch as Mock

beforeEach(() => {
  signedFetchMock.mockReset()
})

describe('request', () => {
  it("answers the envelope's data", async () => {
    signedFetchMock.mockResolvedValue(okResponse({ id: 'a1b2' }))
    await expect(request(ADDRESS, 'GET', '/collections/a1b2')).resolves.toEqual({ id: 'a1b2' })
  })

  it('throws the server error message and status on a failed response', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: false, error: 'Unauthorized' }, false, 401))
    const error = await rejection(request(ADDRESS, 'GET', '/collections/a1b2'))
    expect(error).toBeInstanceOf(BuilderServerError)
    expect(error).toMatchObject({ message: 'Unauthorized', status: 401 })
  })

  it('throws a descriptive error when the response body is not JSON', async () => {
    signedFetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.reject(new Error('Unexpected token <'))
    })
    await expect(request(ADDRESS, 'GET', '/collections/a1b2')).rejects.toThrow(/non-JSON response/)
  })

  it('throws when a successful envelope carries no data', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await expect(request(ADDRESS, 'GET', '/collections/a1b2')).rejects.toThrow('builder-server request failed')
  })

  it('accepts an envelope without data when the endpoint answers none', async () => {
    signedFetchMock.mockResolvedValue(jsonResponse({ ok: true }))
    await expect(request(ADDRESS, 'POST', '/items/i1/files', '', new FormData(), false)).resolves.toBeUndefined()
  })
})

describe('isRefusedRequest', () => {
  it('tells a refused request (4xx) from a transient failure', () => {
    expect(isRefusedRequest(new BuilderServerError('gone', 404))).toBe(true)
    expect(isRefusedRequest(new BuilderServerError('down', 503))).toBe(false)
    expect(isRefusedRequest(new Error('offline'))).toBe(false)
  })
})

describe('baseUrl', () => {
  describe('when the page is served by this deployment, as on localhost or its own domain', () => {
    afterEach(() => {
      delete (window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__
    })

    it("should be the builder-api of the node behind the page's origin", () => {
      expect(baseUrl()).toBe(`${window.location.origin}/builder-api/v1`)
      expect(baseUrl('dcl.example')).toBe(`${window.location.origin}/builder-api/v1`)
    })

    it('should be the builder-api of the public node base when the page names one', () => {
      ;(window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__ = 'https://node.example/'
      expect(baseUrl()).toBe('https://node.example/builder-api/v1')
    })
  })

  describe('when the page is on a Decentraland host', () => {
    it("should be that environment's builder-api", () => {
      expect(baseUrl('decentraland.zone')).toBe('https://builder-api.decentraland.zone/v1')
    })
  })
})

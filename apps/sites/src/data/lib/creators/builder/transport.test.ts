// @vitest-environment jsdom
import '../test/setup'
import { rejection } from '../test/rejection'
import { describe, it, expect, afterEach, beforeEach, vi, type Mock } from 'vitest'
import { BuilderServerError, baseUrl, boundRefusal, isRefusedRequest, request } from './transport'

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

describe('when the node refuses a request at one of its bounds', () => {
  async function refused(
    status: number,
    body: unknown,
    headers: Record<string, string> = {},
    path = '/items/i1/files'
  ) {
    signedFetchMock.mockResolvedValue({ ...jsonResponse(body, false, status), headers: new Headers(headers) })
    return rejection(request(ADDRESS, 'POST', path, '', new FormData(), false))
  }

  it.each([408, 413, 429, 507])("should be the node's own reason for a %i at one of its bounds", async status => {
    const error = await refused(status, { ok: false, data: {}, error: 'The upload is too large' })
    expect(boundRefusal(error)).toEqual({ message: 'The upload is too large' })
  })

  it('should carry the seconds the node asked to wait', async () => {
    const error = await refused(
      429,
      { ok: false, data: {}, error: 'Too many file uploads of this wallet in progress' },
      { 'Retry-After': '30' }
    )
    expect(boundRefusal(error)).toEqual({
      message: 'Too many file uploads of this wallet in progress',
      retryAfterSecs: 30
    })
  })

  it('should be nothing for an answer that is not the envelope, such as a gateway page', async () => {
    signedFetchMock.mockResolvedValue({ ok: false, status: 503, json: () => Promise.reject(new Error('<html>')) })
    expect(boundRefusal(await rejection(request(ADDRESS, 'GET', '/collections/a1b2')))).toBeNull()
  })

  it('should be nothing for an envelope without its reason', async () => {
    expect(boundRefusal(await refused(503, { ok: false, data: {} }))).toBeNull()
  })

  describe('when the node is unavailable', () => {
    it.each(['/items/i1/files', '/items/i1/videos'])(
      "should be the node's own reason for an upload to %s, which only its store can refuse once the item saved",
      async path => {
        const reason = 'The content store is being collected; retry shortly'
        const error = await refused(503, { ok: false, data: {}, error: reason }, {}, path)
        expect(boundRefusal(error)).toEqual({ message: reason })
      }
    )

    it.each(['/items/i1', '/collections/c1'])(
      'should be nothing for any other request to %s, such as one whose creator writes are switched off',
      async path => {
        const error = await refused(
          503,
          { ok: false, data: {}, error: 'creator writes are disabled on this node (CREATE_WRITES_ENABLED=false)' },
          { 'Retry-After': '3600' },
          path
        )
        expect(boundRefusal(error)).toBeNull()
      }
    )
  })

  it('should be nothing for a failure that is no bound of the node', async () => {
    expect(boundRefusal(await refused(500, { ok: false, data: {}, error: 'Internal server error' }))).toBeNull()
  })
})

describe('isRefusedRequest', () => {
  it('tells a refused request (4xx) from a transient failure', () => {
    expect(isRefusedRequest(new BuilderServerError('gone', 404))).toBe(true)
    expect(isRefusedRequest(new BuilderServerError('down', 503))).toBe(false)
    expect(isRefusedRequest(new Error('offline'))).toBe(false)
  })
})

describe('when choosing the builder API', () => {
  describe('and the page is served by this deployment, as on localhost or its own domain', () => {
    afterEach(() => {
      delete (window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__
    })

    it("should be the builder-api of the node behind the page's origin", () => {
      expect(baseUrl()).toBe(`${window.location.origin}/builder-api/v1`)
      expect(baseUrl('dcl.example')).toBe(`${window.location.origin}/builder-api/v1`)
    })

    it('should be the builder-api of the public node base the page names', () => {
      ;(window as Window & { __CATALYST_BASE__?: string }).__CATALYST_BASE__ = 'https://node.example/'
      expect(baseUrl()).toBe('https://node.example/builder-api/v1')
    })
  })

  describe('and the page is on a Decentraland host', () => {
    it("should be that environment's builder-api", () => {
      expect(baseUrl('decentraland.zone')).toBe('https://builder-api.decentraland.zone/v1')
    })
  })
})

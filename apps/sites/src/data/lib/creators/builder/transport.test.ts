import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest'
import { BuilderServerError, isRefusedRequest, request } from './transport'

vi.mock('~/lib/auth', () => ({
  signedFetch: vi.fn()
}))

import { signedFetch } from '~/lib/auth'
import { ADDRESS, jsonResponse, okResponse } from '~/test/builderServer'

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
    const error = await request(ADDRESS, 'GET', '/collections/a1b2').catch((error: unknown) => error)
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

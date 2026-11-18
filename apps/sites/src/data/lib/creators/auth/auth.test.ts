import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach, vi } from 'vitest'
import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts'
import { Authenticator, type AuthIdentity, type AuthLink } from '@dcl/crypto'
import { localStorageStoreIdentity, localStorageClearIdentity } from '@dcl/single-sign-on-client'
import { ProviderType } from '@dcl/schemas'
import { TEST_PRIVATE_KEY, TEST_SIGNER } from '@/data/lib/creators/test/chain'
import { createAuthHeaders, getIdentity, isSocialLogin, logout, signedFetch } from './auth'
import { resetServerClock } from './serverClock'
import { createEphemeralIdentity } from './signer'

vi.mock('decentraland-connect', () => ({
  connection: { disconnect: vi.fn().mockResolvedValue(undefined) }
}))

async function makeStoredIdentity(): Promise<{ address: string; identity: AuthIdentity }> {
  const owner = privateKeyToAccount(generatePrivateKey())
  const ephemeralKey = generatePrivateKey()
  const ephemeral = privateKeyToAccount(ephemeralKey)
  const identity = await Authenticator.initializeAuthChain(
    owner.address,
    {
      address: ephemeral.address,
      publicKey: ephemeral.publicKey,
      privateKey: ephemeralKey
    },
    600,
    message => owner.signMessage({ message })
  )
  const address = owner.address.toLowerCase()
  localStorageStoreIdentity(address, identity)
  return { address, identity }
}

function parseAuthChain(headers: Record<string, string>): AuthLink[] {
  return Object.keys(headers)
    .filter(key => key.startsWith('x-identity-auth-chain-'))
    .sort()
    .map(key => JSON.parse(headers[key]) as AuthLink)
}

describe('auth', () => {
  let address: string

  beforeAll(async () => {
    ;({ address } = await makeStoredIdentity())
  })

  afterAll(() => {
    localStorageClearIdentity(address)
  })

  it('getIdentity returns the stored identity regardless of address casing', () => {
    expect(getIdentity(address)).not.toBeNull()
    expect(getIdentity(address.toUpperCase().replace('0X', '0x'))).not.toBeNull()
    expect(getIdentity('0x0000000000000000000000000000000000000000')).toBeNull()
  })

  it('createAuthHeaders signs the ADR-44 payload and sends the timestamp and metadata it used', async () => {
    const headers = await createAuthHeaders(address, 'GET', `/v1/${address}/collections`)
    const chain = parseAuthChain(headers)
    expect(chain.length).toBeGreaterThanOrEqual(2)
    // The last link carries the signed payload the server rebuilds from method, path and these headers.
    expect(chain[chain.length - 1].payload).toBe(
      `get:/v1/${address}/collections:${headers['x-identity-timestamp']}:${headers['x-identity-metadata']}`
    )
    expect(Number(headers['x-identity-timestamp'])).toBeGreaterThan(0)
    expect(JSON.parse(headers['x-identity-metadata'])).toEqual({})
  })

  it('createAuthHeaders returns no headers without a stored identity', async () => {
    await expect(createAuthHeaders('0x0000000000000000000000000000000000000000', 'GET', '/x')).resolves.toEqual({})
  })

  it('signedFetch signs the full pathname without its query string and fetches the full URL', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true })
    global.fetch = fetchMock

    await signedFetch(address, 'https://builder-api.example/v1', `/${address}/collections?page=1&limit=20`)

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe(`https://builder-api.example/v1/${address}/collections?page=1&limit=20`)
    const headers = init.headers as Record<string, string>
    const chain = parseAuthChain(headers)
    expect(chain[chain.length - 1].payload).toBe(`get:/v1/${address}/collections:${headers['x-identity-timestamp']}:{}`)
  })

  it('signedFetch signs the caller metadata along and sends it verbatim', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true })
    global.fetch = fetchMock

    await signedFetch(
      address,
      'https://marketplace.example',
      '/v1/trades',
      { method: 'POST' },
      { signer: 'dcl:builder', intent: 'dcl:create-trade' }
    )

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    const headers = init.headers as Record<string, string>
    expect(JSON.parse(headers['x-identity-metadata'])).toEqual({ signer: 'dcl:builder', intent: 'dcl:create-trade' })
    const chain = parseAuthChain(headers)
    expect(chain[chain.length - 1].payload).toBe(
      `post:/v1/trades:${headers['x-identity-timestamp']}:${headers['x-identity-metadata']}`.toLowerCase()
    )
  })

  it('logout clears the stored identity so it cannot outlive a sign-out', async () => {
    const { address: other } = await makeStoredIdentity()
    expect(getIdentity(other)).not.toBeNull()

    await logout(other)

    expect(getIdentity(other)).toBeNull()
  })

  it('signedFetch sends no auth headers when there is no address', async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true })
    global.fetch = fetchMock

    await signedFetch(undefined, 'https://builder-api.example/v1', '/collections')

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(Object.keys(init.headers as Record<string, string>)).toHaveLength(0)
  })

  describe('when a signed request is refused as unauthenticated', () => {
    const STATUS_URL = /\/status$/
    const behindMs = 2 * 60 * 60 * 1000

    let fetchMock: ReturnType<typeof vi.fn<(url: string, init?: RequestInit) => Promise<unknown>>>
    let refusedCancel: ReturnType<typeof vi.fn>

    beforeEach(() => {
      refusedCancel = vi.fn().mockResolvedValue(undefined)
      fetchMock = vi.fn<(url: string, init?: RequestInit) => Promise<unknown>>()
      vi.stubGlobal('fetch', fetchMock)
    })

    afterEach(() => {
      resetServerClock()
      vi.unstubAllGlobals()
    })

    describe('and the device clock is hours behind the server', () => {
      let response: Response

      beforeEach(async () => {
        fetchMock.mockImplementation((url: string) => {
          if (STATUS_URL.test(url)) {
            return Promise.resolve({ ok: true, json: () => Promise.resolve({ currentTime: Date.now() + behindMs }) })
          }
          const isRetry = fetchMock.mock.calls.filter(([u]) => !STATUS_URL.test(u)).length > 1
          return Promise.resolve(
            isRetry ? { ok: true, status: 200 } : { ok: false, status: 401, body: { cancel: refusedCancel } }
          )
        })
        response = await signedFetch(address, 'https://builder-api.example/v1', `/${address}/collections`)
      })

      it('should sign the request again with the server time and return the retried response', () => {
        const signedCalls = fetchMock.mock.calls.filter(([u]) => !STATUS_URL.test(u))
        expect(signedCalls).toHaveLength(2)
        const first = Number((signedCalls[0][1]?.headers as Record<string, string>)['x-identity-timestamp'])
        const retried = Number((signedCalls[1][1]?.headers as Record<string, string>)['x-identity-timestamp'])
        expect(retried - first).toBeGreaterThan(behindMs - 60_000)
        expect(response.status).toBe(200)
      })

      it('should release the refused response body', () => {
        expect(refusedCancel).toHaveBeenCalled()
      })
    })

    describe('and it is a write with a body', () => {
      const body = JSON.stringify({ name: 'My collection' })
      const signal = new AbortController().signal

      beforeEach(async () => {
        fetchMock.mockImplementation((url: string) => {
          if (STATUS_URL.test(url)) {
            return Promise.resolve({ ok: true, json: () => Promise.resolve({ currentTime: Date.now() + behindMs }) })
          }
          const isRetry = fetchMock.mock.calls.filter(([u]) => !STATUS_URL.test(u)).length > 1
          return Promise.resolve(isRetry ? { ok: true, status: 200 } : { ok: false, status: 401 })
        })
        await signedFetch(address, 'https://builder-api.example/v1', '/collections', { method: 'PUT', body, signal })
      })

      it('should send the same body and signal again', () => {
        const retried = fetchMock.mock.calls.filter(([u]) => !STATUS_URL.test(u))[1][1]
        expect(retried).toEqual(expect.objectContaining({ method: 'PUT', body, signal }))
      })
    })

    describe('and it was signed before a concurrent refusal corrected the clock', () => {
      let releaseSlowRequest: () => void
      let slow: Promise<Response>
      let signedRequests: number

      beforeEach(async () => {
        signedRequests = 0
        fetchMock.mockImplementation((url: string) => {
          if (STATUS_URL.test(url)) {
            return Promise.resolve({ ok: true, json: () => Promise.resolve({ currentTime: Date.now() + behindMs }) })
          }
          signedRequests++
          if (url.endsWith('/slow') && signedRequests <= 2) {
            // The first request's refusal arrives only after the other request already corrected the clock.
            return new Promise(resolve => {
              releaseSlowRequest = () => resolve({ ok: false, status: 401 })
            })
          }
          return Promise.resolve(signedRequests <= 2 ? { ok: false, status: 401 } : { ok: true, status: 200 })
        })
        slow = signedFetch(address, 'https://builder-api.example/v1', '/slow')
        await signedFetch(address, 'https://builder-api.example/v1', '/fast')
        releaseSlowRequest()
      })

      it('should still sign the late-refused request again with the corrected time', async () => {
        await expect(slow).resolves.toEqual(expect.objectContaining({ status: 200 }))
      })
    })

    describe('and the device clock is in sync with the server', () => {
      let response: Response

      beforeEach(async () => {
        fetchMock.mockImplementation((url: string) =>
          Promise.resolve(
            STATUS_URL.test(url)
              ? { ok: true, json: () => Promise.resolve({ currentTime: Date.now() }) }
              : { ok: false, status: 401 }
          )
        )
        response = await signedFetch(address, 'https://builder-api.example/v1', `/${address}/collections`)
      })

      it('should return the refusal without retrying', () => {
        expect(fetchMock.mock.calls.filter(([u]) => !STATUS_URL.test(u))).toHaveLength(1)
        expect(response.status).toBe(401)
      })
    })

    describe('and the request was not signed', () => {
      beforeEach(async () => {
        fetchMock.mockResolvedValue({ ok: false, status: 401 })
        await signedFetch(undefined, 'https://builder-api.example/v1', '/collections')
      })

      it('should not measure the server clock', () => {
        expect(fetchMock).toHaveBeenCalledTimes(1)
      })
    })
  })

  // The auth chain stores these strings as they are: their format is what servers and other dapps parse.
  it('createEphemeralIdentity answers the checksummed address and the uncompressed public key of its key', () => {
    expect(createEphemeralIdentity(TEST_PRIVATE_KEY)).toEqual({
      address: TEST_SIGNER,
      publicKey:
        '0x044f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa385b6b1b8ead809ca67454d9683fcf2ba03456d6fe2c4abe2b07f0fbdbb2f1c1',
      privateKey: TEST_PRIVATE_KEY
    })
    const fresh = createEphemeralIdentity()
    expect(fresh.privateKey).toMatch(/^0x[0-9a-f]{64}$/)
    expect(fresh.publicKey).toMatch(/^0x04[0-9a-f]{128}$/)
    expect(fresh.address).toMatch(/^0x[0-9a-fA-F]{40}$/)
  })

  it('isSocialLogin is true only for Magic-backed sessions', () => {
    expect(isSocialLogin({ providerType: ProviderType.MAGIC })).toBe(true)
    expect(isSocialLogin({ providerType: ProviderType.MAGIC_TEST })).toBe(true)
    expect(isSocialLogin({ providerType: ProviderType.INJECTED })).toBe(false)
    expect(isSocialLogin({ providerType: ProviderType.WALLET_CONNECT_V2 })).toBe(false)
  })
})

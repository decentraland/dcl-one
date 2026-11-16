import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createEmailIdentity, EMAIL_IDENTITY_LIFETIME_MS } from '../features/auth/guest'
import { authLoginUrl, getStoredLogin, isLoginMethod, rootAddress, storeAccountLogin } from '../features/auth/sso'
import { emailLoginComplete, emailLoginErrorCode, emailLoginStart } from '../features/auth/thirdweb'
import { completeEmailSignIn, startEmailSignIn } from '../shell/emailLogin'

const WALLET = '0x00000000000000000000000000000000000E4A11'

function answer(status: number, body: unknown): Response {
  return new Response(typeof body === 'string' ? body : JSON.stringify(body), { status })
}

describe('email sign-in with thirdweb', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('a refused code and a refused address come back with their codes; a server error with none', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => answer(400, { error: 'Invalid or expired code' })))
    const wrong = await emailLoginComplete('player@example.com', '000000').catch((e: unknown) => e)
    expect(emailLoginErrorCode(wrong)).toBe('incorrect-code')
    // the code survives being carried as text (the bridge scene's replies)
    expect(emailLoginErrorCode(new Error(String(wrong)))).toBe('incorrect-code')

    vi.stubGlobal('fetch', vi.fn(async () => answer(400, { error: 'Invalid email.' })))
    expect(emailLoginErrorCode(await emailLoginStart('nope@example').catch((e: unknown) => e))).toBe('invalid-email')

    vi.stubGlobal('fetch', vi.fn(async () => answer(503, 'unavailable')))
    expect(emailLoginErrorCode(await emailLoginStart('player@example.com').catch((e: unknown) => e))).toBeNull()
  })

  it("checking a code, any refusal is the code's, whatever thirdweb's words", async () => {
    vi.stubGlobal('fetch', vi.fn(async () => answer(401, { message: 'Unauthorized' })))
    expect(emailLoginErrorCode(await emailLoginComplete('player@example.com', '000000').catch((e: unknown) => e))).toBe('incorrect-code')
    // sending a code, a refusal that doesn't name the address is no wrong code
    expect(emailLoginErrorCode(await emailLoginStart('player@example.com').catch((e: unknown) => e))).toBeNull()
  })

  it("a right code whose wallet won't sign from here says so, for the sign-in page to take over", async () => {
    const refused = await createEmailIdentity(
      { complete: async () => ({ token: 't', walletAddress: WALLET }), sign: async () => Promise.reject(new Error('thirdweb 403: forbidden')) },
      'player@example.com',
      '123456'
    ).catch((e: unknown) => e)
    expect(emailLoginErrorCode(refused)).toBe('sign-refused')
    expect(emailLoginErrorCode(new Error(String(refused)))).toBe('sign-refused')
  })

  it('the address never goes into an error', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => answer(400, { error: 'Invalid or expired code' })))
    const e = (await emailLoginComplete('player@example.com', '000000').catch((err: unknown) => err)) as Error
    expect(e.message).not.toContain('player@example.com')
  })

  it('the identity is signed by the wallet the code opens, for 7 days', async () => {
    const before = Date.now()
    const identity = await createEmailIdentity(
      { complete: async () => ({ token: 't', walletAddress: WALLET }), sign: async () => '0xsig' },
      'player@example.com',
      '123456'
    )
    expect(rootAddress(identity)).toBe(WALLET.toLowerCase())
    expect(identity.authChain[1]).toMatchObject({ type: 'ECDSA_EPHEMERAL', signature: '0xsig' })
    const lifetime = new Date(identity.expiration).getTime() - before
    expect(lifetime).toBeGreaterThanOrEqual(EMAIL_IDENTITY_LIFETIME_MS - 1000)
    expect(lifetime).toBeLessThanOrEqual(EMAIL_IDENTITY_LIFETIME_MS + 1000)
  })
})

describe('the web shell side', () => {
  beforeEach(() => localStorage.clear())
  afterEach(() => vi.unstubAllGlobals())

  it('refuses a request that is not an address and a code', async () => {
    await expect(startEmailSignIn({ email: 'not an address' })).rejects.toThrow('bad email request')
    await expect(completeEmailSignIn({ email: 'player@example.com', code: '12ab56' })).rejects.toThrow('bad email request')
  })

  it('checks the code and signs with the wallet it opens, sending no secret', async () => {
    const calls: { url: string; headers: Record<string, string> }[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init: RequestInit) => {
        calls.push({ url, headers: init.headers as Record<string, string> })
        return url.endsWith('/v1/auth/complete') ? answer(200, { token: 'tok', walletAddress: WALLET }) : answer(200, { signature: '0xsig' })
      })
    )
    const { identity, wallet } = await completeEmailSignIn({ email: 'player@example.com', code: '123456' })
    expect(wallet).toEqual({ token: 'tok', walletAddress: WALLET })
    expect(rootAddress(identity)).toBe(WALLET.toLowerCase())
    expect(calls.map((c) => new URL(c.url).pathname)).toEqual(['/v1/auth/complete', '/api/v1/enclave-wallet/sign-message'])
    expect(calls.every((c) => !Object.keys(c.headers).some((h) => /secret/i.test(h)))).toBe(true)
  })

  it("keeps the account as the auth site does, the only one signed in on this origin", () => {
    localStorage.setItem('single-sign-on-0x0000000000000000000000000000000000000abc', '{}')
    const identity = {
      ephemeralIdentity: { address: '0x00000000000000000000000000000000000000e1', publicKey: '0x04', privateKey: '0x01' },
      expiration: new Date(Date.now() + 7 * 86_400_000).toISOString(),
      authChain: [
        { type: 'SIGNER', payload: WALLET.toLowerCase(), signature: '' },
        { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0xsig' }
      ]
    }
    storeAccountLogin(identity)
    expect(localStorage.getItem('single-sign-on-0x0000000000000000000000000000000000000abc')).toBeNull()
    expect(getStoredLogin()?.address).toBe(WALLET.toLowerCase())
  })
})

describe('the sign-in page redirect', () => {
  it('names the method the player picked', () => {
    expect(authLoginUrl('https://example.com/play', [], 'google')).toBe('/auth/login?redirectTo=https%3A%2F%2Fexample.com%2Fplay&loginMethod=google')
    expect(authLoginUrl('https://example.com/play', [])).toBe('/auth/login?redirectTo=https%3A%2F%2Fexample.com%2Fplay')
    expect(isLoginMethod('wallet-connect')).toBe(true)
    expect(isLoginMethod('fortmatic')).toBe(false)
    expect(isLoginMethod('javascript:alert(1)')).toBe(false)
  })
})

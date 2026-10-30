import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AuthIdentity } from '../features/auth/sso'
import { personalSign } from '../shell/sign'
import { leave, visit } from './entryUrl'
import { recoverSigner } from './recoverSigner'

// web3.js docs key; its address is 0x2c7536E3605D9C16a7a3D7b1898e529396a65c23
const KEY = '0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318'
const EPHEMERAL = '0x2c7536E3605D9C16a7a3D7b1898e529396a65c23'

function identity(signer: string, days: number, privateKey = KEY): AuthIdentity {
  const expiration = new Date(Date.now() + days * 86_400_000).toISOString()
  return {
    ephemeralIdentity: { address: EPHEMERAL, publicKey: '0x04', privateKey },
    expiration,
    authChain: [
      { type: 'SIGNER', payload: signer, signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload: `Decentraland Login\nEphemeral address: ${EPHEMERAL}\nExpiration: ${expiration}`, signature: '0xsig' }
    ]
  }
}

const GUEST = '0x1e9a6ddcb5d0b8c2ffd79372e022d63d053ab1bf'
const ACCOUNT = '0x7e5f4552091a69125d5dfcb7b8c2659029395bdf'

vi.mock('../features/auth/guest', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../features/auth/guest')>()),
  webGuestBackend: {},
  createGuestIdentity: vi.fn(async () => identity(GUEST, 30))
}))

async function shell() {
  vi.resetModules()
  return import('../shell/login')
}

describe('shell login requests', () => {
  beforeEach(() => localStorage.clear())

  it('createGuest keeps the guest with its key and hands the app the public identity', async () => {
    const { request } = await shell()
    const login = (await request('createGuest', undefined)) as { address: string; identity: AuthIdentity; guest?: boolean }
    expect(login.address).toBe(GUEST)
    expect(login.guest).toBe(true)
    expect(login.identity.ephemeralIdentity.privateKey).toBeUndefined()
    const stored = JSON.parse(localStorage.getItem('dcl-guest-identity') ?? 'null') as AuthIdentity
    expect(stored.ephemeralIdentity.privateKey).toBe(KEY)

    // it is now the current login, and the shell signs with it
    expect(await request('identity', undefined)).toEqual(login)
    expect(await request('sign', { signer: EPHEMERAL, message: 'Some data' })).toBe(personalSign(KEY, 'Some data'))
  })

  describe("createLocalGuest (the autopilot's guest)", () => {
    type Login = { address: string; identity: AuthIdentity; guest?: boolean } | null
    const ROOT_KEY = 'dcl-local-guest-root-key'

    it('makes one guest with no network call, keeps only its root key, and hands the app the public identity', async () => {
      const fetchSpy = vi.spyOn(globalThis, 'fetch')
      const { createGuestIdentity } = await import('../features/auth/guest')
      vi.mocked(createGuestIdentity).mockClear()
      try {
        const { request } = await shell()
        expect(await request('identity', undefined)).toBeNull()

        const login = (await request('createLocalGuest', undefined)) as NonNullable<Login>
        expect(login.guest).toBe(true)
        expect(login.address).toMatch(/^0x[0-9a-f]{40}$/)
        expect(login.identity.ephemeralIdentity.privateKey).toBeUndefined()
        // no thirdweb, no request of any kind
        expect(fetchSpy).not.toHaveBeenCalled()
        expect(createGuestIdentity).not.toHaveBeenCalled()
        expect(localStorage.getItem('thirdweb_guest_session_id_e1adce863fe287bb6cf0e3fd90bdb77f')).toBeNull()

        // the shell keeps the root key; the identity is held for the page, not stored
        expect(localStorage.getItem(ROOT_KEY)).toMatch(/^0x[0-9a-f]{64}$/)
        expect(localStorage.getItem('dcl-guest-identity')).toBeNull()

        // it is the current login, and the shell signs with its ephemeral key
        expect(await request('identity', undefined)).toEqual(login)
        const { address } = login.identity.ephemeralIdentity
        const signature = (await request('sign', { signer: address, message: 'Some data' })) as string
        expect(recoverSigner('Some data', signature)).toBe(address.toLowerCase())
      } finally {
        fetchSpy.mockRestore()
      }
    })

    it('is the same account on every run: the root key is kept and a fresh identity is signed', async () => {
      const first = (await (await shell()).request('createLocalGuest', undefined)) as NonNullable<Login>
      const key = localStorage.getItem(ROOT_KEY)

      const { request } = await shell() // a new page load: the module state is gone, the storage is not
      expect(await request('identity', undefined)).toBeNull() // no identity is stored between runs
      const again = (await request('createLocalGuest', undefined)) as NonNullable<Login>
      expect(again.address).toBe(first.address)
      expect(again.identity.ephemeralIdentity.address).not.toBe(first.identity.ephemeralIdentity.address)
      expect(again.identity.authChain[0]).toEqual(first.identity.authChain[0])
      expect(recoverSigner(again.identity.authChain[1].payload, again.identity.authChain[1].signature)).toBe(first.address)
      expect(new Date(again.identity.expiration).getTime()).toBeGreaterThan(Date.now() + 29 * 86_400_000)
      expect(localStorage.getItem(ROOT_KEY)).toBe(key)
    })

    it('uses and replaces no account the profile holds: neither a sign-in nor a stored guest', async () => {
      const account = JSON.stringify(identity(ACCOUNT, 60))
      const guest = JSON.stringify(identity(GUEST, 30))
      localStorage.setItem(`single-sign-on-${ACCOUNT}`, account)
      localStorage.setItem('dcl-guest-identity', guest)
      localStorage.setItem('thirdweb_guest_session_id_e1adce863fe287bb6cf0e3fd90bdb77f', 'session')

      const { request } = await shell()
      const login = (await request('createLocalGuest', undefined)) as NonNullable<Login>
      // the run's account is the local guest, not the freshest stored login
      expect(login.guest).toBe(true)
      expect([ACCOUNT.toLowerCase(), GUEST]).not.toContain(login.address)
      expect(await request('identity', undefined)).toEqual(login)
      const { address } = login.identity.ephemeralIdentity
      // signing as it, the only login the shell signs with, forgets nothing
      await request('sign', { signer: address, message: 'hello' })
      expect(localStorage.getItem(`single-sign-on-${ACCOUNT}`)).toBe(account)
      expect(localStorage.getItem('dcl-guest-identity')).toBe(guest)
      expect(localStorage.getItem('thirdweb_guest_session_id_e1adce863fe287bb6cf0e3fd90bdb77f')).toBe('session')

      // the next page load finds the stored login as it was
      expect(await (await shell()).request('identity', undefined)).toMatchObject({ address: ACCOUNT })
    })

    it('keeps its root key when an account later signs in on this profile', async () => {
      const first = (await (await shell()).request('createLocalGuest', undefined)) as NonNullable<Login>
      localStorage.setItem(`single-sign-on-${ACCOUNT}`, JSON.stringify(identity(ACCOUNT, 60)))
      const { request: later } = await shell()
      expect(await later('identity', undefined)).toMatchObject({ address: ACCOUNT })
      await later('sign', { signer: EPHEMERAL, message: 'hello' })
      expect(localStorage.getItem(ROOT_KEY)).toMatch(/^0x[0-9a-f]{64}$/)
      const next = (await (await shell()).request('createLocalGuest', undefined)) as NonNullable<Login>
      expect(next.address).toBe(first.address)
    })

    it('is refused off loopback hosts: no key is made, nothing is stored', async () => {
      const { request } = await shell()
      for (const url of ['https://decentraland.org/', 'https://localhost.example/', 'http://10.0.0.1:8080/']) {
        visit(url)
        try {
          await expect(request('createLocalGuest', undefined)).rejects.toThrow(/loopback/)
          expect(localStorage.getItem(ROOT_KEY)).toBeNull()
          expect(localStorage.getItem('dcl-guest-identity')).toBeNull()
          expect(await request('identity', undefined)).toBeNull()
        } finally {
          leave()
        }
        // and none was kept for the page the harness serves either
        expect(localStorage.getItem(ROOT_KEY)).toBeNull()
      }
    })

    it('is made on any loopback host', async () => {
      for (const url of ['http://127.0.0.1:8080/', 'http://perf.localhost:8080/', 'http://[::1]:8080/']) {
        visit(url)
        try {
          const login = (await (await shell()).request('createLocalGuest', undefined)) as NonNullable<Login>
          expect(login.guest).toBe(true)
        } finally {
          leave()
        }
      }
    })

    it('fails on a kept key that is not a key, rather than make a new account over it', async () => {
      localStorage.setItem(ROOT_KEY, 'not a key')
      await expect((await shell()).request('createLocalGuest', undefined)).rejects.toThrow(/not a key/)
      expect(localStorage.getItem(ROOT_KEY)).toBe('not a key')
      localStorage.setItem(ROOT_KEY, '0x' + 'AB'.repeat(32))
      await expect((await shell()).request('createLocalGuest', undefined)).rejects.toThrow(/not a key/)
    })

    it('survives sign-out like a thirdweb guest does', async () => {
      const { request } = await shell()
      const first = (await request('createLocalGuest', undefined)) as NonNullable<Login>
      await request('logout', undefined)
      expect(localStorage.getItem(ROOT_KEY)).toMatch(/^0x[0-9a-f]{64}$/)
      const again = (await (await shell()).request('createLocalGuest', undefined)) as NonNullable<Login>
      expect(again.address).toBe(first.address)
    })
  })

  it('the guest is the stored login when it is the freshest, and survives logout', async () => {
    localStorage.setItem(`single-sign-on-${ACCOUNT}`, JSON.stringify(identity(ACCOUNT, 1)))
    localStorage.setItem('dcl-guest-identity', JSON.stringify(identity(GUEST, 30)))
    const { request } = await shell()
    expect(await request('identity', undefined)).toMatchObject({ address: GUEST, guest: true })

    await request('logout', undefined)
    expect(localStorage.getItem(`single-sign-on-${ACCOUNT}`)).toBeNull()
    expect(localStorage.getItem('dcl-guest-identity')).not.toBeNull()
  })

  it('signing with an account replaces the guest', async () => {
    localStorage.setItem('dcl-guest-identity', JSON.stringify(identity(GUEST, 1)))
    localStorage.setItem(`single-sign-on-${ACCOUNT}`, JSON.stringify(identity(ACCOUNT, 30)))
    localStorage.setItem(`thirdweb_guest_session_id_e1adce863fe287bb6cf0e3fd90bdb77f`, 'session')
    const { request } = await shell()
    expect(await request('identity', undefined)).toMatchObject({ address: ACCOUNT })
    expect(localStorage.getItem('dcl-guest-identity')).not.toBeNull()

    await request('sign', { signer: EPHEMERAL, message: 'hello' })
    expect(localStorage.getItem('dcl-guest-identity')).toBeNull()
    expect(localStorage.getItem('thirdweb_guest_session_id_e1adce863fe287bb6cf0e3fd90bdb77f')).toBeNull()
  })
})

import { afterEach, describe, expect, it } from 'vitest'
import { clearStoredLogins, getStoredLogin, RELOGIN_MARGIN_MS, resumeGuestLogin, storeAccountLogin, storeGuestLogin, type AuthIdentity } from '../features/auth/sso'
import { GUEST_SESSION_KEY } from '../features/auth/thirdweb'

const ACCOUNT = '0x00000000000000000000000000000000000a11ce'
const GUEST = '0x00000000000000000000000000000000000b0b00'
const HOUR = 60 * 60 * 1000

function identity(signer: string, leftMs: number): AuthIdentity {
  const expiration = new Date(Date.now() + leftMs).toISOString()
  return {
    ephemeralIdentity: { address: '0x00000000000000000000000000000000000e0e0e', publicKey: '0x04', privateKey: '0x01' },
    expiration,
    authChain: [
      { type: 'SIGNER', payload: signer, signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload: `Decentraland Login\nEphemeral address: 0x0e\nExpiration: ${expiration}`, signature: '0x' }
    ]
  }
}

describe('the stored sign-in a run starts with', () => {
  afterEach(() => localStorage.clear())

  it('is used while it has more than a day to run', () => {
    storeAccountLogin(identity(ACCOUNT, RELOGIN_MARGIN_MS + HOUR))
    expect(getStoredLogin()?.address).toBe(ACCOUNT)
  })

  it('is forgotten with a day or less to run, so signing in asks for a new one', () => {
    storeAccountLogin(identity(ACCOUNT, RELOGIN_MARGIN_MS - HOUR))
    expect(getStoredLogin()).toBeNull()
    expect(localStorage.getItem(`single-sign-on-${ACCOUNT}`)).toBeNull()
  })

  it("a guest's: the identity goes, its wallet session stays for PLAY AS GUEST", () => {
    localStorage.setItem(GUEST_SESSION_KEY, 'session')
    storeGuestLogin(identity(GUEST, 2 * HOUR))
    expect(getStoredLogin()).toBeNull()
    expect(localStorage.getItem('dcl-guest-identity')).toBeNull()
    expect(localStorage.getItem(GUEST_SESSION_KEY)).toBe('session')
  })

  it('a fresh guest is still found when an older account sign-in is about to run out', () => {
    storeAccountLogin(identity(ACCOUNT, HOUR))
    storeGuestLogin(identity(GUEST, 30 * 24 * HOUR))
    expect(getStoredLogin()).toMatchObject({ address: GUEST, guest: true })
  })
})

// unity-explorer's guest: a sign-out ends its session but keeps the guest, and PLAY AS GUEST brings it back
describe('a guest that signs out', () => {
  afterEach(() => localStorage.clear())

  it('is kept, but is not the stored sign-in a run starts with', () => {
    storeAccountLogin(identity(ACCOUNT, 7 * 24 * HOUR))
    storeGuestLogin(identity(GUEST, 30 * 24 * HOUR))
    clearStoredLogins()
    expect(getStoredLogin()).toBeNull()
    expect(localStorage.getItem(`single-sign-on-${ACCOUNT}`)).toBeNull()
    expect(localStorage.getItem('dcl-guest-identity')).not.toBeNull()
  })

  it('comes back with PLAY AS GUEST, and is then the stored sign-in again', () => {
    storeGuestLogin(identity(GUEST, 30 * 24 * HOUR))
    clearStoredLogins()
    expect(resumeGuestLogin()).toMatchObject({ address: GUEST, guest: true })
    expect(getStoredLogin()).toMatchObject({ address: GUEST, guest: true })
  })

  it('is not brought back with a day or less to run: PLAY AS GUEST signs its wallet in again', () => {
    storeGuestLogin(identity(GUEST, 2 * HOUR))
    clearStoredLogins()
    expect(resumeGuestLogin()).toBeNull()
  })

  it('is signed in again on its own once a new guest sign-in is kept', () => {
    storeGuestLogin(identity(GUEST, 30 * 24 * HOUR))
    clearStoredLogins()
    storeGuestLogin(identity(GUEST, 30 * 24 * HOUR))
    expect(getStoredLogin()).toMatchObject({ address: GUEST, guest: true })
  })

  it('an account signing out with no guest kept marks none', () => {
    storeAccountLogin(identity(ACCOUNT, 7 * 24 * HOUR))
    clearStoredLogins()
    expect(localStorage.getItem('dcl-guest-signed-out')).toBeNull()
  })
})

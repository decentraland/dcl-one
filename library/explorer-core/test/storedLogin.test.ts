import { afterEach, describe, expect, it } from 'vitest'
import { getStoredLogin, RELOGIN_MARGIN_MS, storeAccountLogin, storeGuestLogin, type AuthIdentity } from '../features/auth/sso'
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

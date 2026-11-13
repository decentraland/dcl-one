// DOMAIN: email sign-in through the session: the code is checked before anything signs in, and the
// account it opens then signs in like any other (the lobby).
import { describe, expect, it } from 'vitest'
import { act, waitFor } from '@testing-library/react'
import type { AuthIdentity } from '@explorer-core/features/auth/sso'
import { FakeDriver, renderSession } from '../harness'

const IDENTITY: AuthIdentity = {
  ephemeralIdentity: { address: '0x00000000000000000000000000000000000000e1', publicKey: '0x04' },
  expiration: new Date(Date.now() + 3_600_000).toISOString(),
  authChain: [
    { type: 'SIGNER', payload: '0x00000000000000000000000000000000000e4a11', signature: '' },
    { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0xsig' }
  ]
}

class EmailDriver extends FakeDriver {
  readonly emailCalls: string[] = []
  verify: (email: string, code: string) => Promise<AuthIdentity> = async () => IDENTITY
  async emailSendCode(email: string): Promise<void> {
    this.emailCalls.push(`send ${email}`)
  }
  emailVerify(email: string, code: string): Promise<AuthIdentity> {
    this.emailCalls.push(`verify ${email} ${code}`)
    return this.verify(email, code)
  }
  async loginEmail(identity: AuthIdentity): Promise<void> {
    this.calls.push('loginEmail')
    this.emailCalls.push(`login ${identity.authChain[0].payload}`)
  }
}

describe('email sign-in session', () => {
  it('sends the code, checks it, then signs in with the account it opens and goes to the lobby', async () => {
    const driver = new EmailDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    expect(h.session().login.emailAvailable).toBe(true)

    await act(() => h.session().login.sendEmailCode('player@example.com'))
    await act(() => h.session().login.verifyEmailCode('player@example.com', '123456'))
    await waitFor(() => expect(h.session().phase).toBe('lobby'))
    await waitFor(() => expect(driver.calls).toContain('loginEmail'))
    expect(driver.emailCalls).toEqual(['send player@example.com', 'verify player@example.com 123456', 'login 0x00000000000000000000000000000000000e4a11'])
  })

  it("holds a checked code's sign-in a moment, so its screen shows Success before Verifying", async () => {
    const driver = new EmailDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    await act(() => h.session().login.verifyEmailCode('player@example.com', '123456'))
    expect(h.session().phase).toBe('login')
    expect(h.session().login.emailQueued).toBe(true)
    expect(driver.calls).not.toContain('loginEmail')
    await waitFor(() => expect(driver.calls).toContain('loginEmail'))
  })

  it('a refused code signs nothing in, and a check aborted on the way (Back) comes to nothing', async () => {
    const driver = new EmailDriver()
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))

    driver.verify = async () => Promise.reject(new Error('[incorrect-code] thirdweb 400'))
    await expect(h.session().login.verifyEmailCode('player@example.com', '000000')).rejects.toThrow('incorrect-code')

    driver.verify = async () => IDENTITY
    // what the page keeps of an account; the aborted check lets it go
    localStorage.setItem('single-sign-on-0x00000000000000000000000000000000000e4a11', '{}')
    const abort = new AbortController()
    const check = h.session().login.verifyEmailCode('player@example.com', '123456', abort.signal)
    abort.abort()
    await act(() => check)
    expect(h.session().phase).toBe('login')
    expect(driver.calls).not.toContain('loginEmail')
    expect(localStorage.getItem('single-sign-on-0x00000000000000000000000000000000000e4a11')).toBeNull()
  })
})

import { describe, expect, it } from 'vitest'
import { startupScreen } from '../features/startup/startupScreen'

describe('startupScreen', () => {
  it('shows the splash while the session is read', () => {
    expect(startupScreen({ status: 'loading', authPending: false, error: null })).toBe('splash')
  })

  it('a visitor with no session chooses guest or an account', () => {
    expect(startupScreen({ status: 'sign-in-or-guest', authPending: false, error: null })).toBe('guestOrSignUp')
    // a failed guest sign-in stays here, with its Connection Error
    expect(startupScreen({ status: 'sign-in-or-guest', authPending: false, error: 'thirdweb 500' })).toBe('guestOrSignUp')
  })

  it('a stored sign-in goes in under the splash, and only a failure shows Welcome back', () => {
    expect(startupScreen({ status: 'reuse-login-or-new', authPending: false, error: null })).toBe('splash')
    expect(startupScreen({ status: 'reuse-login-or-new', authPending: false, error: 'Login failed' })).toBe('existingAccount')
  })

  it("the desktop browser sign-in shows its own panel", () => {
    expect(startupScreen({ status: 'sign-in-or-guest', authPending: true, error: null })).toBe('browserSignIn')
  })
})

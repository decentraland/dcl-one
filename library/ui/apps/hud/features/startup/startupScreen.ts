// Which startup screen the sign-in state shows, from launch until a sign-in is submitted. The screens still
// to come (login selection, the email code, verifying) take their place here.
import type { LoginFlow } from '../session/useEngineSession'

export type StartupScreen =
  /** The animated logo while the session is read, and while a stored sign-in goes straight in. */
  | 'splash'
  /** Play as guest, or log in or sign up. */
  | 'guestOrSignUp'
  /** The desktop browser sign-in in flight, with its Cancel. */
  | 'browserSignIn'
  /** A stored sign-in that failed: "Welcome back" with Try again or a different account. */
  | 'existingAccount'

export function startupScreen(flow: Pick<LoginFlow, 'status' | 'authPending' | 'error'>): StartupScreen {
  if (flow.status === 'sign-in-or-guest' && !flow.authPending) return 'guestOrSignUp'
  if (flow.status === 'loading' || (flow.status === 'reuse-login-or-new' && flow.error == null)) return 'splash'
  return flow.authPending ? 'browserSignIn' : 'existingAccount'
}

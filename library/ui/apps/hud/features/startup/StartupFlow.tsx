// The screens from launch until a sign-in is submitted (startupScreen.ts picks one from the sign-in
// state; the player moves between the sign-in screens here). They share one frame: the drifting logo
// pattern on the purple vignette, laid out on a 1920x1080 canvas scaled to fit, with the footer in its
// corner. A sign-in that fails shows Connection Error over whichever screen it was started on.
import { useEffect, useRef, useState } from 'react'
import { LogoPattern } from '../../design'
import type { LoginMethod } from '@explorer-core/features/auth/sso'
import type { LoginFlow } from '../session/useEngineSession'
import { LoadingAndLogin } from '../login/LoadingAndLogin'
import { GuestOrSignUp } from './GuestOrSignUp'
import { LoginSelection } from './LoginSelection'
import { StartupFooter } from './StartupFooter'
import { StartupSplash } from './StartupSplash'
import { VerificationOtp } from './VerificationOtp'
import { showConnectionError } from './connectionError'
import { startupScreen } from './startupScreen'
import type { LoginMusic } from './useLoginMusic'
import styles from './StartupFlow.module.css'

type View =
  | { screen: 'guestOrSignUp' }
  | { screen: 'loginSelection'; email: string; emailError: boolean }
  | { screen: 'verificationOtp'; email: string }

export function StartupFlow({ flow, music }: { flow: LoginFlow; music: LoginMusic }): React.JSX.Element {
  const [view, setView] = useState<View>({ screen: 'guestOrSignUp' })
  // A failed guest sign-in gets the generic Connection Error; a failed account sign-in (the desktop
  // browser sign-in) keeps what went wrong, e.g. that no app on the computer opens Decentraland's
  // sign-in links.
  const attempt = useRef<'guest' | 'account' | null>(null)
  const { error } = flow
  useEffect(() => {
    if (error == null) return
    const abort = new AbortController()
    showConnectionError(attempt.current === 'account' ? error : undefined, abort.signal)
    return () => abort.abort()
  }, [error])

  const screen = startupScreen(flow)
  if (screen === 'splash') return <StartupSplash />
  // the screens not ported yet keep today's panel
  if (screen === 'existingAccount') return <LoadingAndLogin flow={flow} />

  const toMethods = (email = '', emailError = false): void => setView({ screen: 'loginSelection', email, emailError })
  const signInWith = (method: LoginMethod): void => {
    attempt.current = 'account'
    flow.startWithAccount(method)
  }

  // the desktop browser sign-in, under way: the methods screen waits with a spinner and Cancel
  const current: View = screen === 'browserSignIn' && view.screen !== 'loginSelection' ? { screen: 'loginSelection', email: '', emailError: false } : view
  return (
    <div className={styles.root}>
      <LogoPattern />
      <div className={styles.canvas}>
        {current.screen === 'guestOrSignUp' && (
          <GuestOrSignUp
            flow={flow}
            onGuest={() => {
              attempt.current = 'guest'
              flow.exploreAsGuest()
            }}
            onLogInOrSignUp={() => toMethods()}
          />
        )}
        {current.screen === 'loginSelection' && (
          <LoginSelection
            flow={flow}
            initialEmail={current.email}
            initialEmailError={current.emailError}
            inProgress={screen === 'browserSignIn'}
            onBack={() => setView({ screen: 'guestOrSignUp' })}
            onEmailSent={(email) => setView({ screen: 'verificationOtp', email })}
            onMethod={signInWith}
            onCancel={flow.cancelLogin}
          />
        )}
        {current.screen === 'verificationOtp' && (
          <VerificationOtp
            flow={flow}
            email={current.email}
            onBack={() => toMethods(current.email)}
            onInvalidEmail={() => toMethods(current.email, true)}
          />
        )}
        <StartupFooter music={music} />
      </div>
    </div>
  )
}

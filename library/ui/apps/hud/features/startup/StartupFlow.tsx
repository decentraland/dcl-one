// The screens from launch until a sign-in is submitted (startupScreen.ts picks one from the sign-in
// state; the player moves between the sign-in screens here). They share one frame: the drifting logo
// pattern on the purple vignette, laid out on a 1920x1080 canvas scaled to fit, with the footer in its
// corner. A sign-in that fails shows Connection Error over whichever screen it was started on.
import { useCallback, useEffect, useRef, useState } from 'react'
import { LogoPattern } from '../../design'
import { EngineViewport } from '../engine/EngineViewport'
import { useSession } from '../session/SessionContext'
import { NewAccountLook, type LookChoice } from './NewAccountLook'
import { NewAccountName } from './NewAccountName'
import { PRESETS } from './presets'
import { Verifying } from './Verifying'
import type { LoginMethod } from '@explorer-core/features/auth/sso'
import { VERIFY_TIMED_OUT, type LoginFlow } from '../session/useEngineSession'
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

export function StartupFlow({
  flow,
  music
}: {
  flow: LoginFlow
  music: LoginMusic
}): React.JSX.Element {
  const session = useSession()
  const [view, setView] = useState<View>({ screen: 'guestOrSignUp' })
  // a new account's two steps, and the look it picked (worn by its avatar, deployed with its name)
  const [step, setStep] = useState<'look' | 'name'>('look')
  const [look, setLook] = useState<LookChoice>({ body: 'a', slot: 0 })
  const { showLook } = session.welcome
  // a new account starts on its look, body type A's first: one that signed out from its name screen
  // and signs in again (a new guest, say) starts over
  const inWelcome = session.phase === 'welcome'
  useEffect(() => {
    if (inWelcome) return
    setStep('look')
    setLook({ body: 'a', slot: 0 })
  }, [inWelcome])
  const wear = useCallback(
    (choice: LookChoice) => {
      setLook(choice)
      showLook(
        (choice.body === 'a' ? 0 : 4) + choice.slot + 1,
        PRESETS[choice.body][choice.slot].look
      )
    },
    [showLook]
  )
  // A failed guest sign-in gets the generic Connection Error; a failed account sign-in (the desktop
  // browser sign-in) keeps what went wrong, e.g. that no app on the computer opens Decentraland's
  // sign-in links.
  const attempt = useRef<'guest' | 'account' | null>(null)
  const { error } = flow
  useEffect(() => {
    if (error == null) return
    const abort = new AbortController()
    showConnectionError(attempt.current === 'account' && error !== VERIFY_TIMED_OUT ? error : undefined, abort.signal)
    return () => abort.abort()
  }, [error])

  // a sign-in that failed starts the screens over
  useEffect(() => {
    if (error != null) setView({ screen: 'guestOrSignUp' })
  }, [error])
  // a new account's save that failed: Connection Error, on its name screen to try again
  const welcomeError = session.welcome.error
  useEffect(() => {
    if (welcomeError != null) showConnectionError()
  }, [welcomeError])

  // Signed in: Verifying while the account loads (and an existing account's terms are answered for
  // it), then a new account's look and name; anyone else goes on to the lobby.
  if (session.phase === 'welcome') {
    const welcome = session.welcome
    if (welcome.pending || !welcome.newProfile)
      return (
        <StartupFrame music={music}>
          <Verifying onCancel={flow.cancelSignIn} />
        </StartupFrame>
      )
    return (
      <StartupFrame
        music={music}
        stage={<EngineViewport region="signin" report={session.setEngineViewport} />}
        stageReady={session.signinStageReady}
      >
        {step === 'look' ? (
          <NewAccountLook
            initial={look}
            onShow={wear}
            onContinue={() => setStep('name')}
            onBack={session.logout}
          />
        ) : (
          <NewAccountName
            email={welcome.email}
            saving={welcome.saving}
            onJumpIn={(name, newsletter) => welcome.accept(name, newsletter)}
            onBack={session.logout}
            onLegal={welcome.openLegal}
          />
        )}
      </StartupFrame>
    )
  }

  const screen = startupScreen(flow)
  if (screen === 'splash') return <StartupSplash />
  const toMethods = (email = '', emailError = false): void =>
    setView({ screen: 'loginSelection', email, emailError })
  const signInWith = (method: LoginMethod): void => {
    attempt.current = 'account'
    flow.startWithAccount(method)
  }

  // the desktop browser sign-in, under way: the methods screen waits with a spinner and Cancel
  const current: View =
    screen === 'browserSignIn' && view.screen !== 'loginSelection'
      ? { screen: 'loginSelection', email: '', emailError: false }
      : view
  return (
    <StartupFrame music={music}>
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
    </StartupFrame>
  )
}

/** The sign-in screens' frame: the pattern on the vignette and the footer, on the 1920x1080 canvas.
 *  `stage` is a new account's avatar: the engine draws the same background over the whole screen, with
 *  the avatar on its podium in the stage's column, and once it has (`stageReady`) this background
 *  fades out to show it. */
function StartupFrame({
  music,
  stage,
  stageReady = false,
  children
}: {
  music: LoginMusic
  stage?: React.ReactNode
  stageReady?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div className={styles.root}>
      <div className={`${styles.backdrop} ${stage != null && stageReady ? styles.backdropAway : ''}`.trim()}>
        <LogoPattern />
      </div>
      <div className={styles.canvas}>
        {stage != null && <div className={styles.stage}>{stage}</div>}
        {children}
        <StartupFooter music={music} />
      </div>
    </div>
  )
}

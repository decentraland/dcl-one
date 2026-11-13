// The first screen with no stored session: play as a
// guest, or log in or sign up. A guest gets the name and terms page next (WelcomePage); an account
// goes to the sign-in page. A guest sign-in that fails shows Connection Error.
//
// Laid out on the startup screens' 1920x1080 canvas, scaled to fit.
import { useEffect, useRef } from 'react'
import { Button, showDialog } from '../../design'
import { prefetchPlaces } from '@explorer-core/features/places/placesApi'
import type { LoginFlow } from '../session/useEngineSession'
import { pendingLabel } from '../login/LoadingAndLogin'
import { canExit, exitApplication } from '../gate/exitApplication'
import logo from '../../assets/loading/logo-icon.webp'
import avatars from '../../assets/startup/avatars.webp'
import styles from './GuestOrSignUp.module.css'

export const CONNECTION_ERROR = {
  title: 'Connection Error',
  body: "We couldn't connect to the server. Please check your internet connection and try again later."
}

export function GuestOrSignUp({ flow }: { flow: LoginFlow }): React.JSX.Element {
  // the lobby's places, warmed while the visitor chooses
  useEffect(() => {
    prefetchPlaces()
  }, [])

  // A failed guest sign-in gets Connection Error; a failed account sign-in (the
  // desktop browser sign-in) keeps what went wrong, e.g. that no app on the computer opens
  // Decentraland's sign-in links. Retry closes the popup and leaves both buttons to try again;
  // Exit leaves, where the page can.
  const attempt = useRef<'guest' | 'account' | null>(null)
  const { error } = flow
  useEffect(() => {
    if (error == null) return
    const abort = new AbortController()
    void showDialog({
      title: CONNECTION_ERROR.title,
      body: attempt.current === 'account' ? error : CONNECTION_ERROR.body,
      actions: [...(canExit() ? [{ id: 'exit', label: 'Exit', variant: 'secondary' as const }] : []), { id: 'retry', label: 'Retry' }],
      actionsEqual: true,
      signal: abort.signal
    }).then((choice) => {
      if (choice === 'exit') exitApplication()
    })
    return () => abort.abort()
  }, [error])

  // the click is taken: the engine's boot progress shows on the button until it runs
  const queued = flow.acceptQueued
  const locked = flow.busy || queued

  return (
    <div className={styles.canvas}>
      <img className={styles.logo} src={logo} alt="Decentraland" />
      <h1 className={styles.title}>Welcome to Decentraland!</h1>
      <p className={styles.description}>Discover a world shaped by its community!</p>
      <Button
        size="start"
        className={styles.playAsGuest}
        disabled={locked}
        onClick={() => {
          attempt.current = 'guest'
          flow.exploreAsGuest()
        }}
      >
        {queued && !flow.engineReady ? pendingLabel(flow) : 'Play as guest'}
      </Button>
      <Button
        size="start"
        variant="darkOutline"
        className={styles.logInOrSignUp}
        disabled={locked}
        onClick={() => {
          attempt.current = 'account'
          flow.startWithAccount()
        }}
      >
        Login or signup
      </Button>
      <img className={styles.avatars} src={avatars} alt="" draggable={false} />
    </div>
  )
}

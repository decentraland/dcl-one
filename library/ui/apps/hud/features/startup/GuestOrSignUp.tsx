// The first screen with no stored session: play as a guest, or log in or sign up. A guest gets the
// name and terms page next (WelcomePage); an account picks how to sign in (LoginSelection).
//
// Laid out on the startup screens' 1920x1080 canvas, scaled to fit.
import { useEffect } from 'react'
import { Button } from '../../design'
import { prefetchPlaces } from '@explorer-core/features/places/placesApi'
import type { LoginFlow } from '../session/useEngineSession'
import { pendingLabel } from '../login/LoadingAndLogin'
import logo from '../../assets/loading/logo-icon.webp'
import avatars from '../../assets/startup/avatars.webp'
import styles from './GuestOrSignUp.module.css'

export function GuestOrSignUp({
  flow,
  onGuest,
  onLogInOrSignUp
}: {
  flow: LoginFlow
  onGuest: () => void
  onLogInOrSignUp: () => void
}): React.JSX.Element {
  // the lobby's places, warmed while the visitor chooses
  useEffect(() => {
    prefetchPlaces()
  }, [])

  // the click is taken: the engine's boot progress shows on the button until it runs
  const queued = flow.acceptQueued
  const locked = flow.busy || queued

  return (
    <div className={styles.canvas}>
      <img className={styles.logo} src={logo} alt="Decentraland" />
      <h1 className={styles.title}>Welcome to Decentraland!</h1>
      <p className={styles.description}>Discover a world shaped by its community!</p>
      <Button size="start" className={styles.playAsGuest} disabled={locked} onClick={onGuest}>
        {queued && !flow.engineReady ? pendingLabel(flow) : 'Play as guest'}
      </Button>
      <Button size="start" variant="darkOutline" className={styles.logInOrSignUp} disabled={locked} onClick={onLogInOrSignUp}>
        Login or signup
      </Button>
      <img className={styles.avatars} src={avatars} alt="" draggable={false} />
    </div>
  )
}

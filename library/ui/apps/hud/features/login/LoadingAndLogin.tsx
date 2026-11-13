// React port of scene/src/ui-classes/loading-and-login/LoadingAndLogin.tsx, for the startup screen
// not yet built in features/startup: a stored sign-in that failed shows the "welcome back" screen,
// whose "Jump in" hands it straight to the engine.

import { useEffect, useState } from 'react'
import { Button } from '../../design'
import { prefetchPlaces } from '@explorer-core/features/places/placesApi'
import type { LoginFlow } from '../session/useEngineSession'
import styles from './LoadingAndLogin.module.css'

// Pending-CTA label: surface the real boot progress while the WASM downloads (the long phase),
// e.g. "DOWNLOADING… 47%", then a generic "STARTING…" for the quick compile/GPU tail. The percent
// is the overall weighted progress (download is weighted ~80%), matching the boot bar
// (features/engine/BootProgress, over every page while the engine boots).
export function pendingLabel(flow: LoginFlow): string {
  if (flow.loadStep === 'download') return `DOWNLOADING… ${Math.round(flow.loadProgress)}%`
  return 'STARTING…'
}

// Circled arrow on the primary CTA (Figma "JUMP IN" button): white disc + brand arrow.
function ArrowIcon(): React.JSX.Element {
  return (
    <span className={styles.arrow} aria-hidden="true">
      <svg viewBox="0 0 28 28" width="22" height="22">
        <circle cx="14" cy="14" r="14" fill="#fff" />
        <path
          d="M12.5 9l5 5-5 5M9 14h8"
          stroke="var(--brand)"
          strokeWidth="2.4"
          fill="none"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  )
}

// The previous account's avatar: a static full-body snapshot on a CSS gold podium. (The live 3D
// avatar can't render pre-login — the engine has no spawned avatar yet — so we use the snapshot.)
function LoginAvatar({ body }: { body: string }): React.JSX.Element | null {
  const [failed, setFailed] = useState(false)
  if (failed) return null
  return (
    <div className={styles.avatarWrap}>
      <div className={styles.disc} />
      <div className={styles.glow} />
      <img
        className={styles.avatar}
        src={body}
        alt=""
        draggable={false}
        onError={() => setFailed(true)}
      />
    </div>
  )
}

const COPY = { title: 'Welcome back!', subtitle: 'Ready to explore?' }

export function LoadingAndLogin({ flow }: { flow: LoginFlow }): React.JSX.Element {
  // Warm the default places list now, while the user is on the login screen, so the post-jump-in
  // picker renders with data instead of a spinner.
  useEffect(() => {
    prefetchPlaces()
  }, [])
  const profile = flow.profile
  return (
    <div className={styles.root}>
      <div className={styles.watermark} />
      <Panel flow={flow} name={profile.name} />
      {profile.body != null && <LoginAvatar body={profile.body} />}
    </div>
  )
}

function Panel({ flow, name }: { flow: LoginFlow; name?: string }): React.JSX.Element {
  const copy = COPY
  const title = name != null && name !== '' ? `Welcome back ${name}` : copy.title
  // The engine boots in the background while this screen is up. Keep the engine-driven CTAs disabled
  // (showing a starting state) until it can take the login command, so a click never lands in a
  // silent wait. The auth-redirect buttons don't touch the engine, so they stay enabled.
  const enginePending = !flow.engineReady

  return (
    <div className={styles.panel}>
      <div className={styles.logo} />
      <h1 className={styles.title}>{title}</h1>
      {copy.subtitle && <p className={styles.subtitle}>{copy.subtitle}</p>}
      {flow.error && <p className={styles.error}>{flow.error}</p>}

      <div className={styles.buttons}>
        <>
          <Button
            variant="primary"
            size="lg"
            className={styles.cta}
            onClick={flow.jumpIn}
            disabled={flow.busy || enginePending}
          >
            <span className={styles.label}>
              {enginePending
                ? pendingLabel(flow)
                : flow.profileFetchFailed
                  ? 'TRY AGAIN'
                  : 'JUMP INTO DECENTRALAND'}
            </span>
            {!enginePending && <ArrowIcon />}
          </Button>
          <Button
            variant="secondary"
            size="lg"
            className={styles.ctaSecondary}
            onClick={flow.useDifferentAccount}
            disabled={flow.busy}
          >
            <span className={styles.label}>USE A DIFFERENT ACCOUNT</span>
          </Button>
          {flow.profileFetchFailed && (
            <div className={styles.resetBox}>
              <p className={styles.resetText}>
                If the problem persists you can continue with a fresh default profile. This
                permanently replaces your account&apos;s current avatar and profile.
              </p>
              <Button
                variant="secondary"
                size="lg"
                className={`${styles.ctaSecondary} ${styles.ctaDanger}`}
                onClick={flow.resetProfileAndJumpIn}
                disabled={flow.busy || enginePending}
              >
                <span className={styles.label}>RESET PROFILE & JUMP IN</span>
              </Button>
            </div>
          )}
        </>
      </div>
    </div>
  )
}

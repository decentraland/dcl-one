// Full-screen crash surface: the engine panics, the engine stops responding, or react-web itself
// throws. Sits above everything (the popup layer, login, loading) and is deliberately NOT a popup —
// it renders in the ErrorBoundary fallback and in embedded mode, where PopupHost isn't mounted, so it
// mounts its own scrim and focus trap. Fed by useEngineSession's fatalError or the
// ErrorBoundary. A world-not-found is not a crash and does not come here — see openRealmError.

import { useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { ModalShell } from '../../design'
import { useFocusTrap } from '../../lib/useFocusTrap'
import { lockInput } from '@explorer-core/lib/inputLock'
import type { CrashSource } from '@explorer-core/features/error/fatalError'
import styles from './CrashModal.module.css'
import { Button, ModalBackdrop } from '@dcl/one-ui'

const COPY: Record<CrashSource, { title: string; subtitle: string }> = {
  launch: { title: 'Something went wrong', subtitle: "The 3D engine couldn't start. Reloading often fixes it." },
  runtime: { title: 'The world crashed', subtitle: 'The 3D engine stopped unexpectedly.' },
  react: { title: 'Something went wrong', subtitle: 'The app hit an unexpected error.' }
}

export function CrashModal({
  error,
  onReload,
  onDismiss
}: {
  error: { message: string; source: CrashSource }
  onReload: () => void
  /** Provided for a dismissable crash (the runtime watchdog can false-positive) — a boot/react crash
   *  is fatal and has no way out but Reload. */
  onDismiss?: () => void
}): React.JSX.Element {
  const { title, subtitle } = COPY[error.source]
  const scrimRef = useRef<HTMLDivElement>(null)

  // Freeze HUD input while this is up: every global hotkey, the popup layer's Escape and its focus trap
  // stand down, so whatever was open when the crash landed stays open, untouched and screenshottable —
  // see inputLock. Declared before the trap below: hook effects/cleanups run in declaration order, so
  // on unmount the lock releases first — otherwise our own cleanup would see itself still "locked" and
  // skip restoring focus to the opener.
  useEffect(lockInput, [])

  useFocusTrap(scrimRef, true) // self-contained: no PopupHost here (ErrorBoundary fallback / embedded)

  // No Escape-to-dismiss here, unlike every popup: Escape is reflex in-world (release pointer lock,
  // close a menu), and a stray press would wipe the panic text before it's read. A dismissable crash
  // closes on Dismiss or a scrim click; a fatal boot/react crash has no onDismiss at all.

  // The popup layer's scrim (the shared ModalBackdrop: DPI scale, entrance) at --z-fatal, above the whole
  // popup layer + login, portaled to <body>. ModalShell is just the card; this component owns the focus
  // trap and the dismissal (what PopupHost does for popups, but here without depending on it).
  return createPortal(
    <ModalBackdrop ref={scrimRef} className={styles.scrim} tabIndex={-1} onClick={onDismiss ?? undefined}>
      <ModalShell
        // Alert-style dialog: centered header with title-scale type, centered footer buttons.
        header={
          <div className={styles.head}>
            <h2 className={styles.title}>{title}</h2>
            <p className={styles.subtitle}>{subtitle}</p>
          </div>
        }
        role="alertdialog"
        ariaLabel={title}
        closeButton={false}
        actionsAlign="center"
        actions={
          <>
            {onDismiss && (
              <Button appearance="hud" variant="ghost" className={styles.btn} onClick={onDismiss}>
                Dismiss
              </Button>
            )}
            <Button appearance="hud" variant="primary" className={styles.btn} onClick={onReload}>
              Reload
            </Button>
          </>
        }
      >
        {/* The panic text itself — what a bug report needs. */}
        <pre className={styles.detail}>{error.message}</pre>
      </ModalShell>
    </ModalBackdrop>,
    document.body
  )
}

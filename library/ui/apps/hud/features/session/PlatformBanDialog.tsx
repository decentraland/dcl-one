// The platform ban: the ban period and reason when the record carries them, then where to get
// help. Blocking before the world (no way past it but signing out); dismissible after a refused
// in-world travel.

import { useEffect, useState } from 'react'
import { isNativeHud, siteUrl } from '@hud-host'
import type { PlatformBanNotice } from '@explorer-core/engine/generated'
import { Button, ModalShell, openPopup } from '../../design'
import type { SessionPhase } from './useEngineSession'

export const PLATFORM_BAN_TITLE = "You're banned from Decentraland"

/** Time left until `expiresAt`, rounded up: hours under a day, else days. */
export function banPeriod(expiresAt: string, now: number): string {
  const end = Date.parse(expiresAt)
  if (Number.isNaN(end)) return expiresAt
  const left = end - now
  if (left <= 0) return 'expired'
  const hours = Math.ceil(left / 3_600_000)
  if (hours < 24) return hours <= 1 ? '1 hour' : `${hours} hours`
  const days = Math.ceil(left / 86_400_000)
  return days === 1 ? '1 day' : `${days} days`
}

/** The lines shown under the title. */
export function banLines(ban: PlatformBanNotice, now: number): string[] {
  const lines: string[] = []
  if (ban.expiresAt != null && ban.expiresAt !== '') lines.push(`Ban period: ${banPeriod(ban.expiresAt, now)}.`)
  if (ban.reason !== '') lines.push(`Reason: ${ban.reason}.`)
  if (ban.customMessage != null && ban.customMessage !== '') lines.push(ban.customMessage)
  lines.push('Contact Support for more information.')
  return lines
}

/** The ban lines, worded again each minute while the ban has a period. */
function BanLines({ ban }: { ban: PlatformBanNotice }): React.JSX.Element {
  const [now, setNow] = useState(Date.now)
  useEffect(() => {
    if (ban.expiresAt == null || ban.expiresAt === '') return
    const id = setInterval(() => setNow(Date.now()), 60_000)
    return () => clearInterval(id)
  }, [ban.expiresAt])
  return (
    <>
      {banLines(ban, now).map((line) => (
        <p key={line}>{line}</p>
      ))}
    </>
  )
}

const helpUrl = (): string => siteUrl('/help/')

const openHelpPage = (): void => {
  window.open(helpUrl(), '_blank', 'noopener')
}

/** Opens the notice. With `signOut` it blocks: Escape and the scrim do nothing. `openSupport`
 *  replaces opening the help page in a new tab. */
export function openPlatformBanDialog(
  ban: PlatformBanNotice,
  options: { signOut?: () => void; onClose?: () => void; openSupport?: () => void }
): () => void {
  const signOut = options.signOut
  const openSupport = options.openSupport ?? openHelpPage
  const close = openPopup(
    (dismiss) => (
      <ModalShell
        title={PLATFORM_BAN_TITLE}
        width={460}
        role="alertdialog"
        ariaLabel={PLATFORM_BAN_TITLE}
        closeButton={false}
        actionsAlign="center"
        actions={
          <>
            <Button variant="secondary" onClick={openSupport}>
              Contact Support
            </Button>
            {signOut != null ? (
              <Button
                onClick={() => {
                  close()
                  signOut()
                }}
              >
                Sign Out
              </Button>
            ) : (
              <Button onClick={dismiss}>OK</Button>
            )}
          </>
        }
      >
        <BanLines ban={ban} />
      </ModalShell>
    ),
    signOut != null ? { backdropClickCloses: false, confirmClose: () => false, onClose: options.onClose } : { onClose: options.onClose }
  )
  return close
}

const NO_RECORD: PlatformBanNotice = { banned: true, reason: '', customMessage: null, expiresAt: null }

/** Opens the platform-ban notices. Blocking once a ban is known before the first world entry since
 *  sign-in, and kept open until sign-out or a not-banned notice; dismissible after a refused travel
 *  otherwise. A refused travel under the blocking notice is dismissed at once; nothing opens over
 *  sign-in. */
export function usePlatformBanNotices(input: {
  phase: SessionPhase
  notice: PlatformBanNotice | null
  travelRefused: boolean
  dismissTravelRefused: () => void
  signOut: () => void
  openExternalUrl: (url: string) => Promise<boolean>
}): void {
  const { phase, notice, travelRefused } = input
  // the native HUD opens the help page through `openExternalUrl`
  const openSupport = isNativeHud() ? () => void input.openExternalUrl(helpUrl()) : undefined
  // tells an in-world travel's `entering` from the first entry since sign-in
  const [reachedWorld, setReachedWorld] = useState(false)
  useEffect(() => {
    if (phase === 'login') setReachedWorld(false)
    else if (phase === 'world') setReachedWorld(true)
  }, [phase])
  const beforeWorld = phase !== 'login' && phase !== 'world' && !reachedWorld
  const [held, setHeld] = useState(false)
  useEffect(() => {
    if (notice == null || phase === 'login') setHeld(false)
    else if (beforeWorld) setHeld(true)
  }, [notice, phase, beforeWorld])
  const blocking = notice != null && phase !== 'login' && (beforeWorld || held)
  const blockingKey = blocking ? JSON.stringify(notice) : null
  useEffect(() => {
    if (blockingKey == null || notice == null) return
    return openPlatformBanDialog(notice, { signOut: input.signOut, openSupport })
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-open only when the notice changes
  }, [blockingKey])

  useEffect(() => {
    if (blocking && travelRefused) input.dismissTravelRefused()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- dismissTravelRefused is stable
  }, [blocking, travelRefused])
  const refusedNotice = travelRefused && !blocking && phase !== 'login'
  useEffect(() => {
    if (!refusedNotice) return
    return openPlatformBanDialog(notice ?? NO_RECORD, { onClose: input.dismissTravelRefused, openSupport })
    // eslint-disable-next-line react-hooks/exhaustive-deps -- dismissTravelRefused is stable
  }, [refusedNotice])
}

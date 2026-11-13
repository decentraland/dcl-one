// The platform ban: the ban period and reason when the record carries them, then where to get
// help. Blocking before the world (no way past it but signing out); dismissible after a refused
// in-world travel, where the player stays put.

import { siteUrl } from '@hud-host'
import type { PlatformBanNotice } from '@explorer-core/engine/generated'
import { Button, ModalShell, openPopup } from '../../design'

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

const openSupport = (): void => {
  window.open(siteUrl('/help/'), '_blank', 'noopener')
}

/** Opens the notice. With `signOut` it blocks: Escape and the scrim do nothing. */
export function openPlatformBanDialog(ban: PlatformBanNotice, options: { signOut?: () => void; onClose?: () => void }): () => void {
  const signOut = options.signOut
  const lines = banLines(ban, Date.now())
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
        {lines.map((line) => (
          <p key={line}>{line}</p>
        ))}
      </ModalShell>
    ),
    signOut != null ? { backdropClickCloses: false, confirmClose: () => false, onClose: options.onClose } : { onClose: options.onClose }
  )
  return close
}

// "Graphics lowered": the message Auto Graphics raises each time it settles on lower graphics for
// the session. It is large on purpose, since the picture just changed under the player, and it
// closes itself after a while.
import { useEffect } from 'react'
import type { AutoGraphicsNotice } from '@explorer-core/engine/generated'
import { Button } from '../../design'
import styles from './AutoGraphicsBanner.module.css'

export const AUTO_GRAPHICS_TITLE = 'Graphics lowered to keep things smooth'
export const AUTO_GRAPHICS_REASSURANCE = "Your saved settings haven't changed. You can turn off Auto Graphics in Settings."
/** How long the banner stays when the player does nothing. */
export const AUTO_GRAPHICS_DISMISS_MS = 15_000

/** What changed, such as "Shadows off · Resolution 70% · Bloom off": only the parts that apply. */
export function autoGraphicsSummary(notice: AutoGraphicsNotice): string {
  const parts: string[] = []
  if (notice.shadowsOff) parts.push('Shadows off')
  if (notice.renderScalePercent != null) parts.push(`Resolution ${notice.renderScalePercent}%`)
  if (notice.bloomOff) parts.push('Bloom off')
  return parts.join(' · ')
}

interface Props {
  /** The notice with an id that goes up with each one, so the same content shows again. */
  notice: (AutoGraphicsNotice & { id: number }) | null
  dismiss: () => void
  settingsOpen: boolean
  openSettings: () => void
}

export function AutoGraphicsBanner({ notice, dismiss, settingsOpen, openSettings }: Props): React.JSX.Element | null {
  const id = notice?.id
  useEffect(() => {
    if (id == null) return
    const timer = setTimeout(dismiss, AUTO_GRAPHICS_DISMISS_MS)
    return () => clearTimeout(timer)
  }, [id, dismiss])
  if (notice == null) return null
  const summary = autoGraphicsSummary(notice)
  return (
    <div className={styles.host}>
      <div className={styles.banner} role="status" aria-live="polite" aria-label="Graphics lowered">
        <strong className={styles.title}>{AUTO_GRAPHICS_TITLE}</strong>
        {summary !== '' && <p className={styles.summary}>{summary}</p>}
        <p className={styles.note}>{AUTO_GRAPHICS_REASSURANCE}</p>
        <div className={styles.actions}>
          <Button
            variant="secondary"
            size="md"
            onClick={() => {
              if (!settingsOpen) openSettings()
              dismiss()
            }}
          >
            Settings
          </Button>
          <Button variant="primary" size="md" onClick={dismiss}>
            OK
          </Button>
        </div>
      </div>
    </div>
  )
}

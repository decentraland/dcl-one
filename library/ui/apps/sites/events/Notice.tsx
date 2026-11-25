import type { ReactNode } from 'react'
import { GeneralInfoButton, GeneralWarning, GeneralWarningError } from '../../../foundations/icons'
import styles from './Notice.module.css'

// TODO(figma-ds): the design system's alert is not in library/ui yet; the Events pages share this one,
// drawn as the What's On design's <Alert> (standard variant: tinted surface, dark ink, leading icon).

type NoticeTone = 'error' | 'warning' | 'info' | 'success'

const ICONS = {
  error: GeneralWarning,
  warning: GeneralWarningError,
  info: GeneralInfoButton,
  success: GeneralInfoButton
} as const

/**
 * An Events message in its own box: an error interrupts (role `alert`), the other tones wait their turn
 * (role `status`). Line breaks in a text child are kept, so a note can follow its lead on the next line.
 */
function Notice({ tone = 'info', children }: { tone?: NoticeTone; children: ReactNode }) {
  const Icon = ICONS[tone]
  return (
    <p className={styles.notice} data-tone={tone} role={tone === 'error' ? 'alert' : 'status'}>
      <Icon className={styles.icon} size={22} aria-hidden="true" />
      <span className={styles.text}>{children}</span>
    </p>
  )
}

export { Notice, type NoticeTone }

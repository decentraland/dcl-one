import { useEffect, useRef, useState, type ReactNode } from 'react'
import styles from './CopyFeedback.module.css'

export type CopyFeedbackProps = {
  value: string
  children: (copy: () => void) => ReactNode
  /** A value beside the control that the success message replaces. Omit for a tooltip. */
  inlineValue?: ReactNode
  copiedLabel?: string
  successAnnouncement?: string
  errorAnnouncement?: string
}

const TOOLTIP_MS = 1600
const INLINE_MS = 2000

/** Copies text from the supplied control and displays a transient success message beside it. */
export function CopyFeedback({
  value,
  children,
  inlineValue,
  copiedLabel = 'Copied',
  successAnnouncement = copiedLabel,
  errorAnnouncement = 'Could not copy'
}: CopyFeedbackProps) {
  const inline = inlineValue !== undefined
  const latest = useRef(0)
  const [feedback, setFeedback] = useState<{ attempt: number; succeeded: boolean } | null>(null)

  useEffect(() => {
    if (!feedback) return
    const timeout = setTimeout(() => setFeedback(null), inline ? INLINE_MS : TOOLTIP_MS)
    return () => clearTimeout(timeout)
  }, [feedback, inline])

  const copy = async () => {
    const attempt = ++latest.current
    setFeedback(null)
    let succeeded = false
    try {
      await navigator.clipboard.writeText(value)
      succeeded = true
    } catch {
      // The live region can report unsupported or refused clipboard writes.
    }
    if (attempt === latest.current) setFeedback({ attempt, succeeded })
  }

  return (
    <span className={inline ? `${styles.root} ${styles.inline}` : styles.root}>
      {inline && (feedback?.succeeded ? (
        <span key={feedback.attempt} className={styles.inlineSuccess} aria-hidden="true">
          {copiedLabel}
        </span>
      ) : inlineValue)}
      {children(() => void copy())}
      {!inline && feedback?.succeeded ? (
        <span key={feedback.attempt} className={styles.tooltip} aria-hidden="true">
          {copiedLabel}
        </span>
      ) : null}
      <span className={styles.status} role="status">
        {feedback ? (
          <span key={feedback.attempt}>{feedback.succeeded ? successAnnouncement : errorAnnouncement}</span>
        ) : null}
      </span>
    </span>
  )
}

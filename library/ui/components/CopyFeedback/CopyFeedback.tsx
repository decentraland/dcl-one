import { useEffect, useRef, useState, type ReactNode } from 'react'
import styles from './CopyFeedback.module.css'

export type CopyFeedbackProps = {
  value: string
  children: (copy: () => void) => ReactNode
  copiedLabel?: string
  successAnnouncement?: string
  errorAnnouncement?: string
}

const FEEDBACK_MS = 1600

/** Copies text from the supplied control and displays a transient success message beside it. */
export function CopyFeedback({
  value,
  children,
  copiedLabel = 'Copied',
  successAnnouncement = copiedLabel,
  errorAnnouncement = 'Could not copy'
}: CopyFeedbackProps) {
  const latest = useRef(0)
  const [feedback, setFeedback] = useState<{ attempt: number; succeeded: boolean } | null>(null)

  useEffect(() => {
    if (!feedback) return
    const timeout = setTimeout(() => setFeedback(null), FEEDBACK_MS)
    return () => clearTimeout(timeout)
  }, [feedback])

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
    <span className={styles.root}>
      {children(() => void copy())}
      {feedback?.succeeded ? (
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

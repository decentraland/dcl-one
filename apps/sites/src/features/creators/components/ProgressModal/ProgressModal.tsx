import { Modal } from '~/components/Modal'
import styles from './ProgressModal.module.css'

type Props = {
  title: string
  heading: string
  description: string
  /** The line above the bar, e.g. "Uploading files…". */
  label: string
  done: number
  total: number
  /** Without it the work can't be interrupted: no ✕, Escape or scrim close. */
  onClose?: () => void
  testId: string
}

/** A step that runs on its own: heading, copy and a counted progress bar. */
export function ProgressModal({ title, heading, description, label, done, total, onClose, testId }: Props) {
  return (
    <Modal
      title={title}
      onClose={onClose ?? (() => undefined)}
      closeDisabled={!onClose}
      hideClose={!onClose}
      testId={testId}
    >
      <div className={styles.wrap} role="status" aria-live="polite">
        <h3 className={styles.heading}>{heading}</h3>
        <p className={styles.description}>{description}</p>
        <p className={styles.label}>{label}</p>
        <div className={styles.row}>
          <progress
            className={styles.bar}
            value={done}
            max={total}
            aria-label={label}
            data-active={done < total || undefined}
            data-testid={`${testId}-bar`}
          />
          <span className={styles.count} data-testid={`${testId}-count`}>
            {done}/{total}
          </span>
        </div>
      </div>
    </Modal>
  )
}

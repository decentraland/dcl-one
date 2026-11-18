import { useEffect, type ReactNode } from 'react'
import { Button } from '../Button'
import { Confetti } from '../Confetti'
import { WarningIcon } from '../Icons'
import { Modal } from '../Modal'
import { cx } from '@/data/lib/creators/cx'
import { useNotifications } from '@/data/lib/creators/notifications'
import styles from './ConfirmModal.module.css'
import shared from '../../styles/shared.module.css'

type Action = {
  label: string
  onClick: () => void
  testId: string
}

type Props = {
  title: string
  description: ReactNode
  /** Illustration shown instead of the warning glyph. */
  art?: { src: string }
  /** Glyph shown instead of the warning one, for confirmations that aren't warnings. */
  icon?: ReactNode
  /** Failure copy of a rejected confirm, shown as an error toast so the dialog keeps its shape. */
  error?: string | null
  /** Extra content under the description, e.g. an error's technical details. */
  children?: ReactNode
  /** While the confirm action is in flight: blocks closing, disables cancel, spins the confirm button. */
  busy?: boolean
  showClose?: boolean
  /** Rains the purchase confetti over the dialog: for a celebration, never a confirmation. */
  celebrate?: boolean
  onClose: () => void
  cancel?: Action
  confirm?: Action
  /** A wider dialog, for an illustrated notice with longer copy. */
  size?: 'default' | 'medium'
  /** Sizes the two actions to their labels' column and pushes them to the edges, instead of filling the row. */
  spreadActions?: boolean
  /** Keeps a lone action full width, for a dialog that hides Cancel only while its request is open. */
  fillActions?: boolean
  testId: string
}

/** Centered dialog with a glyph or illustration, a heading, copy and up to two footer actions. */
export function ConfirmModal({
  title,
  description,
  art,
  icon,
  error,
  children,
  busy = false,
  showClose = false,
  celebrate = false,
  onClose,
  cancel,
  confirm,
  size = 'default',
  spreadActions = false,
  fillActions = false,
  testId
}: Props) {
  const showToast = useNotifications(state => state.showToast)
  useEffect(() => {
    if (error) showToast(error, { type: 'error' })
  }, [error, showToast])

  return (
    <Modal
      title={title}
      size={size}
      onClose={onClose}
      closeDisabled={busy}
      hideTitle
      showClose={showClose}
      testId={testId}
    >
      {celebrate && <Confetti />}
      <div className={styles.wrap} data-art={art ? 'image' : 'icon'}>
        {art ? (
          <img className={styles.art} src={art.src} alt="" />
        ) : (
          <div className={styles.iconWrap} aria-hidden>
            {icon ?? <WarningIcon />}
          </div>
        )}
        <h3 className={styles.heading} data-testid={`${testId}-title`}>
          {title}
        </h3>
        <div className={styles.body}>
          <p className={styles.text} data-testid={`${testId}-description`}>
            {description}
          </p>
          {children}
        </div>
        <div
          className={cx(shared.modalActions, styles.actions)}
          data-single={(!cancel || !confirm) && !fillActions ? '' : undefined}
          data-spread={spreadActions && cancel && confirm ? '' : undefined}
        >
          {cancel && (
            <Button
              type="button"
              variant="secondary"
              disabled={busy}
              data-testid={cancel.testId}
              onClick={cancel.onClick}
            >
              {cancel.label}
            </Button>
          )}
          {confirm && (
            <Button type="button" loading={busy} data-testid={confirm.testId} onClick={confirm.onClick}>
              {confirm.label}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  )
}

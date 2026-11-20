import type { RefObject } from 'react'

import { Button, Modal } from '@dcl/one-ui'

import styles from './LeaveCreditsDialog.module.css'

type LeaveCreditsDialogProps = {
  labels: {
    confirmTitle: string
    confirmDescription: string
    close: string
    cancel: string
    confirm: string
  }
  /** While the opt-out is in flight the dialog cannot be dismissed. */
  pending: boolean
  error: string | undefined
  onConfirm: () => void
  onClose: () => void
  /** Where focus goes back to on close; Safari does not focus a clicked button, so it is passed, not read. */
  returnFocusRef: RefObject<HTMLElement | null>
}

/**
 * The confirmation before leaving the credits program: the shared Modal, so focus stays inside and the
 * page behind is inert. Escape, Close, Cancel and a click on the backdrop dismiss it, except while
 * leaving is in flight; the page does not scroll behind it, and focus goes back to Leave Program.
 */
export function LeaveCreditsDialog({
  labels,
  pending,
  error,
  onConfirm,
  onClose,
  returnFocusRef
}: LeaveCreditsDialogProps) {
  return (
    <Modal
      title={labels.confirmTitle}
      description={labels.confirmDescription}
      closeLabel={labels.close}
      dismissible={!pending}
      onClose={onClose}
      returnFocusRef={returnFocusRef}
      actions={
        <>
          <Button variant="secondary" size="lg" onClick={onClose} disabled={pending}>
            {labels.cancel}
          </Button>
          <Button size="lg" onClick={onConfirm} loading={pending}>
            {labels.confirm}
          </Button>
        </>
      }
    >
      {error ? (
        <p className={styles.error} role="alert">
          {error}
        </p>
      ) : null}
    </Modal>
  )
}

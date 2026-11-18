import type { ReactNode } from 'react'
import { CheckIcon, OpenInNewIcon } from '@/features/creators/components/Icons'
import successArt from '@/features/creators/assets/sale-success.png'
import { Button } from '@/features/creators/components/Button'
import { Modal } from '@/features/creators/components/Modal'
import { useTranslation } from '@/data/lib/creators/intl'
import styles from './SuccessModal.module.css'

type Props = {
  title: string
  description: ReactNode
  /** A secondary line under the description. */
  note?: ReactNode
  art?: string
  /** A way on from the success, shown before DONE. */
  action?: { label: string; onClick: () => void; testId: string; external?: boolean }
  onDone: () => void
  testId?: string
}

/** The celebratory close of an action: illustration, heading, one line, DONE. */
export function SuccessModal({
  title,
  description,
  note,
  art = successArt,
  action,
  onDone,
  testId = 'success-modal'
}: Props) {
  const { t } = useTranslation()
  return (
    <Modal title={title} onClose={onDone} hideTitle size="medium" testId={`${testId}-modal`}>
      <div className={styles.wrap}>
        <img className={styles.art} src={art} alt="" />
        <h3 className={styles.heading} data-testid={`${testId}-title`}>
          {title}
        </h3>
        <div className={styles.body}>
          <p className={styles.text} data-testid={`${testId}-description`}>
            {description}
          </p>
          {note}
        </div>
        <div className={styles.actions}>
          {action && (
            <Button type="button" variant="secondary" data-testid={action.testId} onClick={action.onClick}>
              {action.label}
              {action.external && <OpenInNewIcon fontSize="small" aria-hidden />}
            </Button>
          )}
          <Button type="button" data-testid={`${testId}-done`} onClick={onDone}>
            {t('success_modal.done')}
            <CheckIcon fontSize="small" />
          </Button>
        </div>
      </div>
    </Modal>
  )
}

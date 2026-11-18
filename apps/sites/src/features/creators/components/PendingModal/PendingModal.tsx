import { Modal } from '~/components/Modal'
import { StepIndicator } from '~/components/StepIndicator'
import { useTranslation } from '~/intl'
import styles from './PendingModal.module.css'

type Props = {
  label: string
  /** A multi-signature action shows its heading and a stepper above the spinner. */
  title?: string
  steps?: { labels: string[]; current: number }
  /** Offered only while the wallet prompt is still open: once signed, the action can't be taken back. */
  onCancel?: () => void
  testId: string
}

/** A whole-dialog spinner with one line of status copy, for wallet confirmations and pending transactions. */
export function PendingModal({ label, title, steps, onCancel, testId }: Props) {
  const { t } = useTranslation()
  return (
    <Modal title={title ?? label} onClose={() => undefined} closeDisabled hideTitle testId={testId}>
      <div
        className={styles.wrap}
        role="status"
        aria-live="polite"
        data-with-steps={steps ? '' : undefined}
        data-signature={title && !steps ? '' : undefined}
      >
        {title && <h3 className={styles.title}>{title}</h3>}
        {steps && (
          <div className={styles.stepper}>
            <StepIndicator
              current={steps.current}
              total={steps.labels.length}
              labels={steps.labels}
              testId={`${testId}-step`}
            />
          </div>
        )}
        <span className={styles.bigSpinner} aria-hidden />
        <p className={styles.label} data-testid={`${testId}-label`}>
          {label}
        </p>
        {onCancel && (
          <button className={styles.cancelLink} type="button" data-testid={`${testId}-cancel`} onClick={onCancel}>
            {t('sell_item_modal.cancel')}
          </button>
        )}
      </div>
    </Modal>
  )
}

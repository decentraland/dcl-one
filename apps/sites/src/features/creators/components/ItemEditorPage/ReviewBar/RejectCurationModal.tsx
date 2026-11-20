import { useMemo, useState } from 'react'
import { CloseIcon as RemoveIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '../../Button'
import { Modal } from '../../Modal'
import { MultiSelect } from '../../MultiSelect'
import { useRejectCuration } from '../../../hooks/useCuration'
import type { Collection } from '@/data/lib/creators/collections'
import type { CollectionCuration } from '@/data/lib/creators/curation'
import { REJECT_REASON_CODES, type RejectReasonCode } from '@/data/lib/creators/events'
import { useNotifications } from '@/data/lib/creators/notifications'
import shared from '../../../styles/shared.module.css'
import styles from './RejectCurationModal.module.css'

export const REJECTION_MESSAGE_MAX_LENGTH = 3000

type Props = {
  collection: Collection
  curation: CollectionCuration | null
  address: string
  onClose: () => void
  /** After the rejection is saved, once the modal closed. */
  onRejected?: () => void
}

/** The committee's rejection: one or more fixed reasons, required, plus optional details, both shown to the creator. */
export function RejectCurationModal({ collection, curation, address, onClose, onRejected }: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const reject = useRejectCuration(address)
  const [reasons, setReasons] = useState<RejectReasonCode[]>([])
  const [message, setMessage] = useState('')
  const options = useMemo(
    () => REJECT_REASON_CODES.map(code => ({ value: code, label: t(`reject_reason.${code}`) })),
    [t]
  )
  const trimmed = message.trim()
  const canSubmit = reasons.length > 0

  function remove(code: RejectReasonCode) {
    setReasons(current => current.filter(reason => reason !== code))
  }

  function submit(event: React.FormEvent) {
    event.preventDefault()
    if (!canSubmit) return
    reject.mutate(
      {
        collection,
        curation,
        decision: { rejectionReasons: reasons, ...(trimmed ? { rejectionMessage: trimmed } : {}) }
      },
      {
        onSuccess: () => {
          showToast(t('reject_curation_modal.success', { collection: collection.name }))
          onClose()
          onRejected?.()
        }
      }
    )
  }

  return (
    <Modal
      title={t('reject_curation_modal.title')}
      onClose={onClose}
      closeDisabled={reject.isPending}
      testId="reject-curation-modal"
    >
      <form className={styles.form} onSubmit={submit}>
        <h3 className={styles.heading}>{t('reject_curation_modal.heading')}</h3>
        <div className={styles.field}>
          <span className={styles.label} id="reject-reasons-label">
            {t('reject_curation_modal.reasons')}
          </span>
          <MultiSelect<RejectReasonCode>
            values={reasons}
            options={options}
            onChange={setReasons}
            placeholder={t('reject_curation_modal.reasons_placeholder')}
            summary={t('reject_curation_modal.reasons_selected', { count: reasons.length })}
            disabled={reject.isPending}
            ariaLabel={t('reject_curation_modal.reasons')}
            testId="reject-reasons"
          />
          {reasons.length > 0 && (
            <ul className={styles.pills} data-testid="reject-reason-pills">
              {reasons.map(code => (
                <li className={styles.pill} key={code} data-testid={`reject-reason-pill-${code}`}>
                  {t(`reject_reason.${code}`)}
                  <button
                    className={styles.pillRemove}
                    type="button"
                    aria-label={t('reject_curation_modal.remove_reason', { reason: t(`reject_reason.${code}`) })}
                    disabled={reject.isPending}
                    data-testid={`reject-reason-pill-${code}-remove`}
                    onClick={() => remove(code)}
                  >
                    <RemoveIcon fontSize="inherit" />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <fieldset className={styles.details}>
          <legend className={styles.detailsLegend}>{t('reject_curation_modal.message')}</legend>
          <textarea
            className={styles.textArea}
            value={message}
            maxLength={REJECTION_MESSAGE_MAX_LENGTH}
            placeholder={t('reject_curation_modal.message_placeholder')}
            aria-label={t('reject_curation_modal.message')}
            disabled={reject.isPending}
            data-testid="rejection-message"
            onChange={event => setMessage(event.target.value)}
          />
        </fieldset>
        <span className={styles.charCount}>
          {message.length}/{REJECTION_MESSAGE_MAX_LENGTH}
        </span>
        {reject.isError && (
          <p className={styles.error} data-testid="reject-curation-error">
            {t('reject_curation_modal.error')}
          </p>
        )}
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            disabled={reject.isPending}
            onClick={onClose}
            data-testid="reject-curation-cancel"
          >
            {t('reject_curation_modal.cancel')}
          </Button>
          <Button type="submit" loading={reject.isPending} disabled={!canSubmit} data-testid="reject-curation-confirm">
            {t('reject_curation_modal.confirm')}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

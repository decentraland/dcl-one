import uploadErrorArt from '../../../assets/modal-error.png'
import { ConfirmModal } from '../../ConfirmModal'
import { useTranslation } from '@/data/lib/creators/intl'
import type { UploadFailureReason } from '@/data/lib/creators/uploadItems'
import type { BoundRefusal } from '@/data/lib/creators/builder'
import { useRefusalText } from '../../../hooks/useRefusalText'

type Props = {
  reason: UploadFailureReason
  /** The node's own words for the failure, shown instead of the reason's description. */
  refusal: BoundRefusal | null
  onCancel: () => void
  onRetry: () => void
}

/**
 * Upload failure feedback. Generic, oversized and duplicate failures offer TRY AGAIN (reopens the details modal with
 * the failed items); locked, published and taken-URN failures, which the form can't fix, only GOT IT.
 */
export function UploadErrorModal({ reason, refusal, onCancel, onRetry }: Props) {
  const { t } = useTranslation()
  const refusalText = useRefusalText()
  const canRetry = reason === 'generic' || reason === 'too_big' || reason === 'duplicate_urn'

  return (
    <ConfirmModal
      title={t(`add_items_modal.error.title_${reason}`)}
      description={refusalText(refusal) ?? t(`add_items_modal.error.description_${reason}`)}
      art={{ src: uploadErrorArt }}
      onClose={onCancel}
      cancel={
        canRetry ? { label: t('add_items_modal.cancel'), onClick: onCancel, testId: 'upload-error-cancel' } : undefined
      }
      confirm={
        canRetry
          ? { label: t('add_items_modal.error.try_again'), onClick: onRetry, testId: 'upload-error-retry' }
          : { label: t('add_items_modal.error.got_it'), onClick: onCancel, testId: 'upload-error-dismiss' }
      }
      testId="upload-error-modal"
    />
  )
}

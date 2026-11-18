import errorArt from '~/assets/modal-error.png'
import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'
import { type PublishFailureReason } from '~/lib/publishCollection'

type Props = {
  reason: PublishFailureReason
  onCancel: () => void
  onRetry: () => void
}

// Reasons with dedicated copy; the rest share the generic message.
const DESCRIBED_REASONS: PublishFailureReason[] = [
  'insufficient_credits',
  'locked',
  'unsynced',
  'fee_mismatch',
  'tos_failed'
]
// Retrying can never succeed once the collection is locked or its items drifted.
const UNRETRIABLE_REASONS: PublishFailureReason[] = ['locked', 'unsynced']

/** "We couldn't publish your collection" — TRY AGAIN returns to the wizard with everything kept. */
export function PublishErrorModal({ reason, onCancel, onRetry }: Props) {
  const { t } = useTranslation()
  const description =
    reason === 'wallet'
      ? 'wallet_error.wrong_account'
      : `publish_collection_modal.error.description_${DESCRIBED_REASONS.includes(reason) ? reason : 'generic'}`
  const canRetry = !UNRETRIABLE_REASONS.includes(reason)

  return (
    <ConfirmModal
      title={t('publish_collection_modal.error.title')}
      description={t(description)}
      art={{ src: errorArt }}
      onClose={onCancel}
      cancel={{ label: t('publish_collection_modal.error.cancel'), onClick: onCancel, testId: 'publish-error-cancel' }}
      confirm={
        canRetry
          ? { label: t('publish_collection_modal.error.try_again'), onClick: onRetry, testId: 'publish-error-retry' }
          : undefined
      }
      testId="publish-error-modal"
    />
  )
}

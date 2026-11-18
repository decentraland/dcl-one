import { useState } from 'react'
import { useTranslation } from '~/intl'
import { ConfirmModal } from '~/components/ConfirmModal'
import { PendingModal } from '~/components/PendingModal'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useReviewThirdParty } from '~/hooks/useLinkedReview'
import { isSocialLogin, type Session } from '~/lib/auth'
import { useNotifications } from '~/lib/notifications'
import { type ThirdParty } from '~/lib/thirdParty'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

type Props = {
  session: Session
  thirdParty: ThirdParty
  /** True enables the third party again, false disables it (every collection under it included). */
  approved: boolean
  onClose: () => void
  onDone?: () => void
}

type Phase = 'confirm' | 'signing' | 'pending'

/** Confirm, then the wallet prompt and pending screens; success is a toast. */
export function ThirdPartyStatusFlow({ session, thirdParty, approved, onClose, onDone }: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const review = useReviewThirdParty(session)
  const [phase, setPhase] = useState<Phase>('confirm')
  const action = approved ? 'enable' : 'disable'
  useBeforeUnloadGuard(phase !== 'confirm' || review.isPending)
  const awaitingWallet = review.isPending && !isSocialLogin(session)

  function submit() {
    review.reset()
    if (!isSocialLogin(session)) setPhase('signing')
    review.mutate(
      { thirdParty, approved, onSigned: () => setPhase('pending') },
      {
        onSuccess: () => {
          showToast(t(`item_editor.linked_review.${action}.success`, { name: thirdParty.name }))
          onClose()
          onDone?.()
        },
        onError: error => {
          setPhase('confirm')
          if (isWalletRejection(error)) review.reset()
        }
      }
    )
  }

  if (phase === 'signing') {
    return (
      <PendingModal
        title={t('approval_flow.signature_title')}
        label={t(`item_editor.linked_review.${action}.signing`)}
        onCancel={() => setPhase('confirm')}
        testId="review-third-party-signing"
      />
    )
  }
  if (phase === 'pending') {
    return <PendingModal label={t(`item_editor.linked_review.${action}.pending`)} testId="review-third-party-pending" />
  }
  return (
    <ConfirmModal
      title={t(`item_editor.linked_review.${action}.title`, { name: thirdParty.name })}
      description={t(`item_editor.linked_review.${action}.description`)}
      error={
        review.isError
          ? t(
              isWalletStateError(review.error)
                ? 'wallet_error.wrong_account'
                : `item_editor.linked_review.${action}.error`
            )
          : null
      }
      busy={review.isPending && !awaitingWallet}
      onClose={review.isPending ? () => undefined : onClose}
      fillActions
      cancel={
        review.isPending
          ? undefined
          : { label: t('item_editor.review.cancel'), onClick: onClose, testId: 'review-third-party-cancel' }
      }
      confirm={{
        label: t(`item_editor.linked_review.${action}.confirm`),
        onClick: awaitingWallet ? () => setPhase('signing') : submit,
        testId: 'review-third-party-confirm'
      }}
      testId={`review-third-party-${action}`}
    />
  )
}

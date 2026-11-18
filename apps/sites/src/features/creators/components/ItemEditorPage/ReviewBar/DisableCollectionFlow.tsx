import { useState } from 'react'
import { useTranslation } from '~/intl'
import { ConfirmModal } from '~/components/ConfirmModal'
import { PendingModal } from '~/components/PendingModal'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useDisableCollection } from '~/hooks/useCuration'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { useNotifications } from '~/lib/notifications'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

type Props = {
  session: Session
  collection: Collection
  onClose: () => void
  onDisabled?: () => void
}

type Phase = 'confirm' | 'signing' | 'pending'

/** Confirm, then the same wallet prompt and pending screens as the approval flow; success is a toast. */
export function DisableCollectionFlow({ session, collection, onClose, onDisabled }: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const disable = useDisableCollection(session)
  const [phase, setPhase] = useState<Phase>('confirm')
  useBeforeUnloadGuard(phase !== 'confirm' || disable.isPending)
  // Backed out of the wallet prompt, with the request still open there.
  const awaitingWallet = disable.isPending && !isSocialLogin(session)

  function submit() {
    disable.reset()
    // Custodial wallets sign without a prompt to wait on: the confirm button just spins.
    if (!isSocialLogin(session)) setPhase('signing')
    disable.mutate(
      { collection, onSigned: () => setPhase('pending') },
      {
        onSuccess: () => {
          showToast(t('item_editor.review.disable.success', { collection: collection.name }))
          onClose()
          onDisabled?.()
        },
        onError: error => {
          setPhase('confirm')
          // Dismissing the wallet prompt is the curator changing their mind, not a failure.
          if (isWalletRejection(error)) disable.reset()
        }
      }
    )
  }

  if (phase === 'signing') {
    return (
      <PendingModal
        title={t('approval_flow.signature_title')}
        label={t('item_editor.review.disable.signing')}
        // Only hides the prompt: the request stays open in the wallet, and Disable shows it again.
        onCancel={() => setPhase('confirm')}
        testId="review-disable-signing"
      />
    )
  }
  if (phase === 'pending') {
    return <PendingModal label={t('item_editor.review.disable.pending')} testId="review-disable-pending" />
  }
  return (
    <ConfirmModal
      title={t('item_editor.review.disable.title', { collection: collection.name })}
      description={t('item_editor.review.disable.description')}
      error={
        disable.isError
          ? t(isWalletStateError(disable.error) ? 'wallet_error.wrong_account' : 'item_editor.review.disable.error')
          : null
      }
      busy={disable.isPending && !awaitingWallet}
      // Closing would lose track of the open request; a second Disable would revert and cost gas.
      onClose={disable.isPending ? () => undefined : onClose}
      fillActions
      cancel={
        disable.isPending
          ? undefined
          : { label: t('item_editor.review.cancel'), onClick: onClose, testId: 'review-disable-cancel' }
      }
      confirm={{
        label: t('item_editor.review.disable.confirm'),
        onClick: awaitingWallet ? () => setPhase('signing') : submit,
        testId: 'review-disable-confirm'
      }}
      testId="review-disable"
    />
  )
}

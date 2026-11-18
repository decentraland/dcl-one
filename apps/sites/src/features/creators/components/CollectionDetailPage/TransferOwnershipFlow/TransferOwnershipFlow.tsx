import { useState } from 'react'
import { useTranslation } from '~/intl'
import { useDiscardGuard } from '~/hooks/useDiscardGuard'
import { useTransferCollectionOwnership } from '~/hooks/useTransferCollectionOwnership'
import { useFriends } from '~/hooks/useSales'
import { shortenAddress } from '~/lib/address'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { matchesCollectionName } from '~/lib/collectionOwnership'
import { toSellItemError, type SellFailureReason } from '~/lib/sales'
import { PendingModal } from '~/components/PendingModal'
import { SuccessModal } from '~/components/SuccessModal'
import { SaleErrorModal } from '../SellItemFlow/SaleErrorModal'
import { TransferOwnershipModal, type TransferStep } from './TransferOwnershipModal'

type View = 'form' | 'saving' | 'success' | 'error'
type Phase = 'confirm' | 'pending'

type Props = {
  collection: Collection
  session: Session
  /** The transfer is done and acknowledged: the collection is no longer the signer's. */
  onDone: () => void
  onClose: () => void
}

/**
 * Hand the collection to another wallet. The form asks for the new owner and the collection's name as
 * confirmation, then a second step shows the chosen wallet once more before anything is sent.
 * TRANSFER OWNERSHIP sends `transferCreatorship` in one transaction: web3 wallets get the
 * whole-dialog "confirm in your wallet" status, custodial ones just see the button spin. There is no
 * backing out once sent: the wallet prompt can still sign an irreversible transfer, so the dialog stays
 * locked until it settles. Closing never asks: two fields are nothing to lose.
 */
export function TransferOwnershipFlow({ collection, session, onDone, onClose }: Props) {
  const { t } = useTranslation()
  const social = isSocialLogin(session)
  const friends = useFriends(session, true)

  const [view, setView] = useState<View>('form')
  const [phase, setPhase] = useState<Phase>('confirm')
  const [step, setStep] = useState<TransferStep>('details')
  const [newOwner, setNewOwner] = useState('')
  const [confirmation, setConfirmation] = useState('')
  const [reason, setReason] = useState<SellFailureReason | null>(null)

  const transfer = useTransferCollectionOwnership(session)
  const ready = newOwner !== '' && matchesCollectionName(collection, confirmation)
  const busy = transfer.isPending
  // Only for the busy lock and browser back; nothing to discard, so the close is never questioned.
  const { requestClose } = useDiscardGuard({
    state: view === 'saving' || busy ? 'busy' : 'done',
    dirty: false,
    pending: transfer.isPending,
    onClose
  })

  function submit() {
    setPhase('confirm')
    if (!social) setView('saving')
    transfer.mutate(
      { collection, newOwner, onSigned: () => setPhase('pending') },
      {
        onSuccess: () => setView('success'),
        onError: cause => {
          const failure = toSellItemError(cause).reason
          // Dismissing the wallet prompt is the creator changing their mind, not a failure to report.
          if (failure === 'rejected') return setView('form')
          setReason(failure)
          setView('error')
        }
      }
    )
  }

  switch (view) {
    case 'form':
      return (
        <TransferOwnershipModal
          collection={collection}
          step={step}
          newOwner={newOwner}
          confirmation={confirmation}
          friends={friends.data}
          isLoadingFriends={friends.isLoading}
          busy={busy}
          canContinue={ready && !busy}
          onStep={setStep}
          onNewOwnerChange={setNewOwner}
          onConfirmationChange={setConfirmation}
          onSubmit={submit}
          onClose={requestClose}
        />
      )
    case 'saving':
      return (
        <PendingModal
          label={phase === 'confirm' ? t('sell_item_modal.confirm_in_wallet') : t('transfer_ownership_modal.saving')}
          testId="transfer-ownership-pending"
        />
      )
    case 'success':
      return (
        <SuccessModal
          title={t('transfer_ownership_modal.success_title')}
          description={t('transfer_ownership_modal.success_description', {
            name: collection.name,
            owner: shortenAddress(transfer.data?.owner ?? newOwner)
          })}
          onDone={onDone}
        />
      )
    case 'error':
      return reason ? (
        <SaleErrorModal
          stage="transfer"
          reason={reason}
          onCancel={onClose}
          onRetry={() => {
            setReason(null)
            setView('form')
          }}
        />
      ) : null
  }
}

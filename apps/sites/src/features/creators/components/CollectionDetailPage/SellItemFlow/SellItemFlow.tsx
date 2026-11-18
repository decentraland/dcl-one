import { useMemo, useRef, useState } from 'react'
import { useTranslation } from '~/intl'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useSellItem } from '~/hooks/useSales'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { getMaticChainId } from '~/lib/publishCollection'
import { isSellingOnOlderMarketplace, toSellItemError, type SellFailureReason } from '~/lib/sales'
import { PendingModal } from '~/components/PendingModal'
import { type EnableSalesReason } from './EnableSalesModal'
import { SaleErrorModal, type SaleStage } from './SaleErrorModal'
import { SuccessModal } from '~/components/SuccessModal'
import { DEFAULT_SELL_VALUES, SellItemModal, type SellFormValues, type SellSubmission } from './SellItemModal'
import { useEnableSalesStep, type Phase } from './useEnableSalesStep'

type View = 'form' | 'selling' | 'success' | 'error'

type Props = {
  item: Item
  collection: Collection
  session: Session
  /** The item was edited after its approval: buyers get the approved version until the changes are approved. */
  hasPendingChanges?: boolean
  onClose: () => void
}

/**
 * Put an item on sale: enable sales on the collection the first time (one transaction), then sign the
 * item's order. Web3 wallets get a whole-dialog "confirm in your wallet" status with a way out while the
 * prompt is open; custodial (social login) wallets sign silently, so their submit button just spins.
 */
export function SellItemFlow({ item, collection, session, hasPendingChanges = false, onClose }: Props) {
  const { t } = useTranslation()
  const social = isSocialLogin(session)
  const reason = useMemo<EnableSalesReason>(
    () => (isSellingOnOlderMarketplace(collection, getMaticChainId()) ? 'marketplace-upgrade' : 'first-sale'),
    [collection]
  )

  const [view, setView] = useState<View>('form')
  const [phase, setPhase] = useState<Phase>('confirm')
  const [error, setError] = useState<{ stage: SaleStage; reason: SellFailureReason } | null>(null)
  const [values, setValues] = useState<SellFormValues>(DEFAULT_SELL_VALUES)
  // Bumped when the creator backs out of a wallet prompt, so that attempt's outcome is ignored.
  const attempt = useRef(0)

  const enableStep = useEnableSalesStep({
    collection,
    session,
    reason,
    onClose,
    onFailed: failure => {
      setError({ stage: 'enable', reason: failure })
      setView('error')
    }
  })
  const sell = useSellItem(session)
  useBeforeUnloadGuard(enableStep.isPending || sell.isPending)

  function fail(cause: unknown) {
    const failure = toSellItemError(cause).reason
    // Dismissing the wallet prompt is the creator changing their mind, not a failure to report.
    if (failure === 'rejected') {
      setView('form')
      return
    }
    setError({ stage: 'sell', reason: failure })
    setView('error')
  }

  function submitSell(formValues: SellFormValues, submission: SellSubmission) {
    setValues(formValues)
    const id = ++attempt.current
    setPhase('confirm')
    if (!social) setView('selling')
    sell.mutate(
      { collection, item, ...submission, onSigned: () => attempt.current === id && setPhase('pending') },
      {
        onSuccess: () => attempt.current === id && setView('success'),
        onError: cause => attempt.current === id && fail(cause)
      }
    )
  }

  function backOut() {
    attempt.current++
    setView('form')
  }

  // Until sales are enabled, the Enable Sales step stands in for the form (a failed enable still shows its error).
  if (enableStep.modal && view !== 'error') return enableStep.modal

  switch (view) {
    case 'form':
      return (
        <SellItemModal
          item={item}
          session={session}
          initialValues={values}
          hasPendingChanges={hasPendingChanges}
          busy={social && sell.isPending}
          onSubmit={submitSell}
          onClose={onClose}
        />
      )
    case 'selling':
      return (
        <PendingModal
          label={phase === 'confirm' ? t('sell_item_modal.confirm_in_wallet') : t('sell_item_modal.selling')}
          onCancel={phase === 'confirm' ? backOut : undefined}
          testId="sell-item-pending"
        />
      )
    case 'success':
      return (
        <SuccessModal
          title={t('sell_item_modal.success.title')}
          description={t('sell_item_modal.success.description')}
          onDone={onClose}
        />
      )
    case 'error':
      return error ? (
        <SaleErrorModal
          stage={error.stage}
          reason={error.reason}
          onCancel={onClose}
          onRetry={() => {
            setError(null)
            // A failed enable left the Enable Sales step on its dialog, which takes over again.
            setView('form')
          }}
        />
      ) : null
  }
}

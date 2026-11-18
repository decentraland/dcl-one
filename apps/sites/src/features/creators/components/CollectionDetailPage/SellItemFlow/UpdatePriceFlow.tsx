import { useRef, useState } from 'react'
import { useTranslation } from '~/intl'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useSellItem, useUpdatePrice } from '~/hooks/useSales'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { type ItemListing } from '~/lib/listings'
import { toSellItemError, type ListingTerms, type PricedSale, type SellFailureReason } from '~/lib/sales'
import { PendingModal } from '~/components/PendingModal'
import { type PriceFormValues } from './PriceField'
import { SaleErrorModal, type SaleStage } from './SaleErrorModal'
import { SuccessModal } from '~/components/SuccessModal'
import { UpdatePriceModal } from './UpdatePriceModal'
import { useEnableSalesStep } from './useEnableSalesStep'

type View = 'form' | 'signing' | 'storing' | 'success' | 'error'
// The two wallet prompts of a price change, then the order being stored.
type Step = 'cancel' | 'sign'

type Props = {
  item: Item
  collection: Collection
  listing: ItemListing & { tradeId: string }
  session: Session
  onClose: () => void
}

/**
 * Change a listing's price or currency: the current order is cancelled on chain and a new one is signed
 * with the same beneficiary and expiration. Web3 wallets see the two signatures as steps; social login signs
 * silently and only sees "Updating price". Once the old order is gone, a retry re-lists directly.
 * The new order is signed on the current marketplace, so if that one cannot mint the collection yet
 * (the listing is on an older one), sales are enabled for it first.
 */
export function UpdatePriceFlow({ item, collection, listing, session, onClose }: Props) {
  const { t } = useTranslation()
  const social = isSocialLogin(session)

  const [view, setView] = useState<View>('form')
  const [step, setStep] = useState<Step>('cancel')
  const [stage, setStage] = useState<SaleStage>('update')
  const [reason, setReason] = useState<SellFailureReason>('generic')
  const [values, setValues] = useState<PriceFormValues | undefined>()
  // Set once the old listing is cancelled: from then on only the new order is missing. A ref, because
  // the mutation callbacks close over the render they were created in and nothing renders it.
  const terms = useRef<ListingTerms | null>(null)
  const attempt = useRef(0)

  // A live listing means the collection already sells, just not on the marketplace new orders are signed on.
  const enableStep = useEnableSalesStep({
    collection,
    session,
    reason: 'marketplace-upgrade',
    onClose,
    onFailed: failure => {
      setStage('enable')
      setReason(failure)
      setView('error')
    }
  })
  const update = useUpdatePrice(session)
  const sell = useSellItem(session)
  useBeforeUnloadGuard(enableStep.isPending || update.isPending || sell.isPending)

  function fail(cause: unknown) {
    const failure = toSellItemError(cause).reason
    // Rejecting the first prompt leaves the listing untouched, so the form comes back. Rejecting the
    // second one happens after the old order is already cancelled: the item is off sale, hence the error view.
    if (failure === 'rejected' && !terms.current) {
      setView('form')
      return
    }
    setStage('update')
    setReason(failure)
    setView('error')
  }

  function submit(formValues: PriceFormValues, price: PricedSale) {
    setValues(formValues)
    const id = ++attempt.current
    const guard =
      <T,>(fn: (arg: T) => void) =>
      (arg: T) =>
        attempt.current === id && fn(arg)
    if (terms.current) {
      // The old order is already gone: only the new signature is left.
      setStep('sign')
      if (!social) setView('signing')
      sell.mutate(
        {
          collection,
          item,
          price,
          ...terms.current,
          onSigned: guard<void>(() => setView('storing'))
        },
        { onSuccess: guard(() => setView('success')), onError: guard(fail) }
      )
      return
    }
    setStep('cancel')
    setView(social ? 'storing' : 'signing')
    update.mutate(
      {
        collection,
        item,
        tradeId: listing.tradeId,
        price,
        onSigned: guard(signed => {
          if (signed === 'cancel') setStep('sign')
          else setView('storing')
        }),
        onCancelled: guard((cancelled: ListingTerms) => {
          terms.current = cancelled
        })
      },
      { onSuccess: guard(() => setView('success')), onError: guard(fail) }
    )
  }

  function backOut() {
    attempt.current++
    setView('form')
  }

  const steps = {
    labels: [t('sell_item_modal.update_price.step_remove'), t('sell_item_modal.update_price.step_confirm')],
    current: step === 'cancel' ? 1 : 2
  }

  // Until sales are enabled, the Enable Sales step stands in for the form (a failed enable still shows its error).
  if (enableStep.modal && view !== 'error') return enableStep.modal

  switch (view) {
    case 'form':
      return (
        <UpdatePriceModal item={item} listing={listing} initialValues={values} onSubmit={submit} onClose={onClose} />
      )
    case 'signing':
      return (
        <PendingModal
          title={t('sell_item_modal.update_price.two_signatures')}
          steps={steps}
          label={t(
            step === 'cancel'
              ? 'sell_item_modal.update_price.confirm_removal'
              : 'sell_item_modal.update_price.confirm_new'
          )}
          // Only the first prompt can be walked away from: once the cancellation is signed, the listing is going.
          onCancel={step === 'cancel' ? backOut : undefined}
          testId="update-price-signing"
        />
      )
    case 'storing':
      return <PendingModal label={t('sell_item_modal.update_price.pending')} testId="update-price-pending" />
    case 'success':
      return (
        <SuccessModal
          title={t('sell_item_modal.update_price.success_title')}
          description={t('sell_item_modal.update_price.success_description')}
          onDone={onClose}
        />
      )
    case 'error':
      return <SaleErrorModal stage={stage} reason={reason} onCancel={onClose} onRetry={() => setView('form')} />
  }
}

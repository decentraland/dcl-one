import { useRef, useState } from 'react'
import { useTranslation } from '~/intl'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useRemoveListing } from '~/hooks/useSales'
import { isSocialLogin, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { type ItemListing } from '~/lib/listings'
import { toSellItemError, type SellFailureReason } from '~/lib/sales'
import { ConfirmModal } from '~/components/ConfirmModal'
import { PendingModal } from '~/components/PendingModal'
import { SaleErrorModal } from './SaleErrorModal'
import { SuccessModal } from '~/components/SuccessModal'

type View = 'confirm' | 'pending' | 'success' | 'error'
type Phase = 'confirm' | 'mining'

type Props = {
  item: Item
  collection: Collection
  listing: ItemListing
  session: Session
  onClose: () => void
}

/** Take an item off sale: confirm, cancel its order on chain (one transaction), celebrate. */
export function RemoveListingFlow({ item, collection, listing, session, onClose }: Props) {
  const { t } = useTranslation()
  const social = isSocialLogin(session)

  const [view, setView] = useState<View>('confirm')
  const [phase, setPhase] = useState<Phase>('confirm')
  const [reason, setReason] = useState<SellFailureReason>('generic')
  const attempt = useRef(0)

  const remove = useRemoveListing(session)
  useBeforeUnloadGuard(remove.isPending)

  function start() {
    const id = ++attempt.current
    setPhase('confirm')
    if (!social) setView('pending')
    remove.mutate(
      { collection, item, listing, onSigned: () => attempt.current === id && setPhase('mining') },
      {
        onSuccess: () => attempt.current === id && setView('success'),
        onError: cause => {
          if (attempt.current !== id) return
          const failure = toSellItemError(cause).reason
          if (failure === 'rejected') {
            setView('confirm')
            return
          }
          setReason(failure)
          setView('error')
        }
      }
    )
  }

  function backOut() {
    attempt.current++
    setView('confirm')
  }

  switch (view) {
    case 'confirm':
      return (
        <ConfirmModal
          title={t('sell_item_modal.remove.title')}
          description={t('sell_item_modal.remove.description', { name: item.name })}
          busy={social && remove.isPending}
          onClose={onClose}
          cancel={{ label: t('sell_item_modal.cancel'), onClick: onClose, testId: 'remove-listing-cancel' }}
          confirm={{ label: t('sell_item_modal.remove.confirm'), onClick: start, testId: 'remove-listing-confirm' }}
          testId="remove-listing-modal"
        />
      )
    case 'pending':
      return (
        <PendingModal
          label={t(phase === 'confirm' ? 'sell_item_modal.confirm_in_wallet' : 'sell_item_modal.remove.pending')}
          onCancel={phase === 'confirm' ? backOut : undefined}
          testId="remove-listing-pending"
        />
      )
    case 'success':
      return (
        <SuccessModal
          title={t('sell_item_modal.remove.success_title')}
          description={t('sell_item_modal.remove.success_description')}
          onDone={onClose}
        />
      )
    case 'error':
      return <SaleErrorModal stage="remove" reason={reason} onCancel={onClose} onRetry={() => setView('confirm')} />
  }
}

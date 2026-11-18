import { useRef, useState } from 'react'
import errorArt from '~/assets/modal-error.png'
import { useTranslation } from '~/intl'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { endOutcome, useDiscountInFlight, useEndDiscount } from '~/hooks/useCollectionDiscounts'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { discountPct, toDiscountError, type CollectionDiscount, type DiscountFailureReason } from '~/lib/coupons'
import { ConfirmModal } from '~/components/ConfirmModal'
import { PendingModal } from '~/components/PendingModal'
import { SuccessModal } from '~/components/SuccessModal'

type View = 'confirm' | 'pending' | 'success' | 'error' | 'slow'

type Props = {
  discount: CollectionDiscount
  collection: Collection
  session: Session
  onClose: () => void
}

/** End a discount early: confirm, cancel the coupon from the wallet on Polygon (it pays the gas), celebrate. */
export function EndDiscountFlow({ discount, collection, session, onClose }: Props) {
  const { t } = useTranslation()
  const end = useEndDiscount(session)
  const { ending } = useDiscountInFlight({ discountId: discount.id })
  const [failure, setFailure] = useState<DiscountFailureReason>('generic')
  const [view, setView] = useState<View>('confirm')
  const [phase, setPhase] = useState<'confirm' | 'mining'>('confirm')
  const attempt = useRef(0)
  // This dialog's own request whose wallet prompt the creator backed out of: the prompt stays open in the wallet,
  // so End shows that request again rather than sending a second paid one.
  const backedOut = useRef<number | null>(null)
  const [heldOpen, setHeldOpen] = useState(false)
  // An attempt from a dialog the creator closed counts too: it may still be mining, or have already settled.
  const outcome = endOutcome(session.address, discount.id)
  useBeforeUnloadGuard(ending)

  function hold(id: number | null) {
    backedOut.current = id
    setHeldOpen(id !== null)
  }

  function start() {
    if (backedOut.current !== null && ending) {
      attempt.current = backedOut.current
      hold(null)
      setPhase('confirm')
      setView('pending')
      return
    }
    // A request from a dialog the creator closed may still be open: a second one would be a second paid cancellation.
    if (ending || outcome) return
    const id = ++attempt.current
    setPhase('confirm')
    setView('pending')
    end.mutate(
      {
        discount,
        collection,
        // Signed even after backing out: only one request is ever open, so it is the one to show mining.
        onSigned: () => {
          attempt.current = id
          hold(null)
          setPhase('mining')
          setView('pending')
        }
      },
      {
        onSuccess: () => setView('success'),
        onError: cause => {
          // A request backed out of still owns the confirmation, so its failure is explained there too.
          const backedOutOf = backedOut.current === id
          if (backedOutOf) hold(null)
          if (attempt.current !== id && !backedOutOf) return
          const reason = toDiscountError(cause).reason
          if (reason === 'rejected') {
            setView('confirm')
            return
          }
          // Sent, just not mined yet: offering a retry would pay for a second cancellation.
          if (reason === 'pending') {
            setView('slow')
            return
          }
          setFailure(reason)
          setView('error')
        }
      }
    )
  }

  // Never offer End again for a discount whose cancellation is already sent or done, whichever dialog sent it.
  const shown: View | 'in_flight' =
    view !== 'confirm'
      ? view
      : outcome === 'ended'
        ? 'success'
        : ending
          ? heldOpen
            ? view
            : 'in_flight'
          : outcome === 'pending'
            ? 'slow'
            : view

  switch (shown) {
    case 'in_flight':
      return (
        <PendingModal
          label={t('collection_discount.end.in_flight')}
          onCancel={onClose}
          testId="end-discount-in-flight"
        />
      )
    case 'confirm':
      return (
        <ConfirmModal
          title={t('collection_discount.end.title')}
          description={t(
            discount.collections.length > 1
              ? 'collection_discount.end.description_multi'
              : 'collection_discount.end.description',
            { pct: discountPct(discount), others: discount.collections.length - 1 }
          )}
          onClose={onClose}
          cancel={{ label: t('collection_discount.cancel'), onClick: onClose, testId: 'end-discount-cancel' }}
          confirm={{ label: t('collection_discount.end.confirm'), onClick: start, testId: 'end-discount-confirm' }}
          testId="end-discount-modal"
        />
      )
    case 'pending':
      return (
        <PendingModal
          label={t(phase === 'confirm' ? 'collection_discount.confirm_in_wallet' : 'collection_discount.end.pending')}
          onCancel={
            phase === 'confirm'
              ? () => {
                  hold(attempt.current)
                  attempt.current++
                  setView('confirm')
                }
              : undefined
          }
          testId="end-discount-pending"
        />
      )
    case 'success':
      return (
        <SuccessModal
          title={t('collection_discount.end.success_title')}
          description={t('collection_discount.end.success_description')}
          onDone={onClose}
        />
      )
    case 'slow':
      return (
        <ConfirmModal
          title={t('collection_discount.end.slow_title')}
          description={t('collection_discount.end.slow_description')}
          onClose={onClose}
          confirm={{ label: t('collection_discount.end.got_it'), onClick: onClose, testId: 'end-discount-slow-close' }}
          testId="end-discount-slow-modal"
        />
      )
    case 'error':
      return (
        <ConfirmModal
          title={t('collection_discount.end.error_title')}
          description={t(
            failure === 'wallet'
              ? 'collection_discount.end.error_description_wallet'
              : 'collection_discount.end.error_description'
          )}
          art={{ src: errorArt }}
          onClose={onClose}
          cancel={{ label: t('collection_discount.cancel'), onClick: onClose, testId: 'end-discount-error-cancel' }}
          confirm={{
            label: t('collection_discount.error.try_again'),
            onClick: () => setView('confirm'),
            testId: 'end-discount-error-retry'
          }}
          testId="end-discount-error-modal"
        />
      )
  }
}

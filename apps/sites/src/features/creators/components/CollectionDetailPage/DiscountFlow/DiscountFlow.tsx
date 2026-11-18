import { useRef, useState } from 'react'
import errorArt from '@/features/creators/assets/modal-error.png'
import { useIntl } from 'react-intl'
import { useTranslation } from '@/data/lib/creators/intl'
import { useBeforeUnloadGuard } from '@/features/creators/hooks/useBeforeUnloadGuard'
import { lastCreatedDiscount, useCreateDiscount, useDiscountInFlight } from '@/features/creators/hooks/useCollectionDiscounts'
import { isSocialLogin, type Session } from '@/data/lib/creators/auth'
import { type Collection } from '@/data/lib/creators/collections'
import {
  DiscountError,
  discountPct,
  isDiscountCapped,
  toDiscountError,
  type DiscountFailureReason
} from '@/data/lib/creators/coupons'
import { acknowledgeUnconfirmedCreate, unconfirmedCreateOf } from '@/data/lib/creators/discountAttempts'
import { type SaleableCollection } from '@/data/lib/creators/saleableCollection'
import { ConfirmModal } from '@/features/creators/components/ConfirmModal'
import { PendingModal } from '@/features/creators/components/PendingModal'
import { SuccessModal } from '@/features/creators/components/SuccessModal'
import { CreatorSaleModal, type SaleTerms } from '../CreatorSaleModal'
import sell from '../SellItemFlow/SellItemModal.module.css'
import styles from './DiscountFlow.module.css'

type View = 'form' | 'settling' | 'success' | 'error' | 'unconfirmed'

type Props = {
  collection: Collection
  session: Session
  /** The collection as the discount modal reads it. */
  sale: SaleableCollection
  onClose: () => void
}

/**
 * Create a discount over the collection with the Shop's own modal: one wallet signature, no transaction and no fee.
 * Around it, what the Shop's modal doesn't do: settle a coupon signed earlier whose storing got no answer before
 * anything new is signed, and wait on a create still in flight from a dialog the creator closed.
 */
export function DiscountFlow({ collection, session, sale, onClose }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const social = isSocialLogin(session)
  const create = useCreateDiscount(session)
  const { creating } = useDiscountInFlight({ collectionId: collection.id })
  // A coupon signed earlier, here or before a reload, whose storing was never confirmed: settle it before the form.
  const [view, setView] = useState<View>(() =>
    unconfirmedCreateOf(session.address, collection.id) ? 'unconfirmed' : 'form'
  )
  const [failure, setFailure] = useState<DiscountFailureReason>('generic')
  // A create from a dialog the creator closed counts too: while it posts, and once it went through.
  useBeforeUnloadGuard(creating)
  const createdAtOpen = useRef(lastCreatedDiscount(collection.id))
  const createdMeanwhile = lastCreatedDiscount(collection.id) !== createdAtOpen.current

  function sign(terms: SaleTerms, onSigned: () => void) {
    // Another tab may have left a signed coupon unconfirmed since this opened: settle that one first.
    if (unconfirmedCreateOf(session.address, collection.id)) {
      setView('unconfirmed')
      return Promise.reject(new DiscountError('unconfirmed', 'An unconfirmed discount needs settling first'))
    }
    return create.mutateAsync({ collection, ...terms, discountedCount: sale.listedCount, onSigned })
  }

  /** Stores the coupon already signed (or finds it stored); never asks for a new signature. */
  function retryUnconfirmed() {
    const pending = unconfirmedCreateOf(session.address, collection.id)
    if (!pending) {
      setView('form')
      return
    }
    const { payload } = pending
    setView('settling')
    create.mutate(
      {
        collection,
        resume: payload,
        discountPct: discountPct(payload),
        startsAtMs: payload.checks.effective,
        endsAtMs: payload.checks.expiration,
        uses: isDiscountCapped(payload) ? payload.checks.uses : undefined,
        discountedCount: sale.listedCount
      },
      {
        onSuccess: () => setView('success'),
        onError: cause => {
          const reason = toDiscountError(cause).reason
          if (reason === 'unconfirmed') {
            setView('unconfirmed')
            return
          }
          setFailure(reason)
          setView('error')
        }
      }
    )
  }

  /** Lets the creator sign a new one on their word: the unconfirmed one stays recorded and may still be on sale. */
  function discardUnconfirmed() {
    const pending = unconfirmedCreateOf(session.address, collection.id)
    if (pending) acknowledgeUnconfirmedCreate(session.address, pending.payload.signature)
    setView(unconfirmedCreateOf(session.address, collection.id) ? 'unconfirmed' : 'form')
  }

  const unconfirmed = unconfirmedCreateOf(session.address, collection.id)
  // A create from a dialog the creator closed is still posting: wait on it, then show how it went. This dialog's
  // own create is shown by the modal itself.
  // And once a closed dialog's create ends with its storing unconfirmed, settle that coupon rather than show the form.
  const unconfirmedNow = view === 'form' && !creating && !!unconfirmedCreateOf(session.address, collection.id)
  const shown: View | 'in_flight' =
    (view === 'form' || view === 'unconfirmed') && createdMeanwhile && !create.isSuccess
      ? 'success'
      : (view === 'form' || view === 'unconfirmed') && creating && !create.isPending
        ? 'in_flight'
        : unconfirmedNow
          ? 'unconfirmed'
          : view

  switch (shown) {
    case 'in_flight':
      return (
        <PendingModal
          label={t('collection_discount.in_flight')}
          onCancel={onClose}
          testId="create-discount-in-flight"
        />
      )
    case 'form':
      return (
        <CreatorSaleModal
          session={session}
          collection={sale}
          onSubmit={sign}
          onUnconfirmed={() => setView('unconfirmed')}
          onClose={onClose}
        />
      )
    case 'settling':
      return <PendingModal label={t('collection_discount.saving')} testId="create-discount-pending" />
    case 'success':
      return (
        <SuccessModal
          title={t('collection_discount.success.title')}
          description={t(
            social ? 'collection_discount.success.description_social' : 'collection_discount.success.description'
          )}
          onDone={onClose}
        />
      )
    case 'unconfirmed':
      return (
        <ConfirmModal
          title={t('collection_discount.unconfirmed.title')}
          description={t('collection_discount.unconfirmed.description')}
          art={{ src: errorArt }}
          onClose={onClose}
          cancel={{ label: t('collection_discount.close'), onClick: onClose, testId: 'discount-unconfirmed-close' }}
          confirm={{
            label: t('collection_discount.unconfirmed.check'),
            onClick: retryUnconfirmed,
            testId: 'discount-unconfirmed-retry'
          }}
          testId="discount-unconfirmed-modal"
        >
          {unconfirmed && (
            <div className={sell.note} data-testid="discount-unconfirmed-terms">
              <p>
                {t('collection_discount.unconfirmed.terms', {
                  pct: discountPct(unconfirmed.payload),
                  date: intl.formatDate(unconfirmed.payload.checks.expiration, { month: 'short', day: 'numeric' })
                })}{' '}
                <button
                  className={styles.textButton}
                  type="button"
                  data-testid="discount-unconfirmed-discard"
                  onClick={discardUnconfirmed}
                >
                  {t('collection_discount.unconfirmed.discard')}
                </button>{' '}
                {t(
                  social
                    ? 'collection_discount.unconfirmed.discard_hint_social'
                    : 'collection_discount.unconfirmed.discard_hint'
                )}
              </p>
            </div>
          )}
        </ConfirmModal>
      )
    case 'error':
      return (
        <ConfirmModal
          title={t('collection_discount.error.title')}
          description={t(failure === 'wallet' ? 'wallet_error.wrong_account' : 'collection_discount.error.description')}
          art={{ src: errorArt }}
          onClose={onClose}
          cancel={{ label: t('collection_discount.cancel'), onClick: onClose, testId: 'discount-error-cancel' }}
          confirm={{
            label: t('collection_discount.error.try_again'),
            onClick: retryUnconfirmed,
            testId: 'discount-error-retry'
          }}
          testId="discount-error-modal"
        />
      )
  }
}

import { useMemo } from 'react'
import { ChevronLeftIcon } from '@/features/creators/components/Icons'
import { useIntl } from '@/data/lib/creators/intl'
import { config } from '@/data/lib/creators/config'
import { useTranslation } from '@/data/lib/creators/intl'
import { useManaBalance } from '@/features/creators/hooks/useBalances'
import { type Session } from '@/data/lib/creators/auth'
import { type Collection } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { openExternal } from '@/data/lib/creators/navigation'
import { formatMana, weiToUsdCents } from '@/data/lib/creators/publishFee'
import { getLinkedPublishQuote, type LinkedSlots } from '@/data/lib/creators/publishLinkedItems'
import { type ThirdPartyPrice } from '@/data/lib/creators/thirdPartyRegistry'
import { type ThirdParty } from '@/data/lib/creators/thirdParty'
import { Button } from '@/features/creators/components/Button'
import { Checkbox } from '@/features/creators/components/Checkbox'
import { PaymentMethodCard } from '../../PublishCollectionModal/PaymentMethodCard'
import styles from '@/features/creators/components/CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'
import shared from '@/features/creators/styles/shared.module.css'

const SUMMARY_COLUMNS = 'minmax(0, 2fr) minmax(80px, 1fr) minmax(90px, 1fr) minmax(120px, 1fr)'

type Props = {
  collection: Collection
  thirdParty: ThirdParty
  session: Session
  itemsToPublish: Item[]
  itemsToPush: Item[]
  isProgrammatic: boolean
  price: ThirdPartyPrice | undefined
  priceError: boolean
  onRetryPrice: () => void
  slots: LinkedSlots | undefined
  slotsError: boolean
  /** The fee went up since it was shown: Submit now confirms the new one. */
  feeChanged: boolean
  onRetrySlots: () => void
  /** An earlier attempt sent a payment: it is confirmed first, and only what is still missing is charged. */
  resuming: boolean
  /** A run is starting or running: Submit must not start a second one. */
  busy: boolean
  accepted: boolean
  onAcceptedChange: (accepted: boolean) => void
  onBack: () => void
  onSubmit: () => void
}

/** Last step: the slots to buy and their MANA price, the wallet's balance, the terms, and the submit itself. */
export function LinkedPaymentStep({
  collection,
  thirdParty,
  session,
  itemsToPublish,
  itemsToPush,
  isProgrammatic,
  price,
  priceError,
  onRetryPrice,
  slots,
  slotsError,
  feeChanged,
  onRetrySlots,
  resuming,
  busy,
  accepted,
  onAcceptedChange,
  onBack,
  onSubmit
}: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const mana = useManaBalance(session.address)

  const quote = useMemo(
    () =>
      price && slots
        ? getLinkedPublishQuote(itemsToPublish.length, slots, price, {
            isProgrammatic,
            published: thirdParty.published
          })
        : null,
    [price, slots, itemsToPublish.length, isProgrammatic, thirdParty.published]
  )
  const slotsToBuy = quote?.slotsToBuy ?? null
  const fee = quote?.fee ?? null
  const isRegistered = thirdParty.published || !!slots?.registered
  const balance = mana.data ?? 0n
  const hasEnough = !!fee && balance >= fee.maxManaWei
  const isFree = !!fee && fee.manaWei === 0n
  const usd = (wei: bigint) => intl.formatNumber(weiToUsdCents(wei) / 100, { style: 'currency', currency: 'USD' })
  const loadError = priceError || slotsError
  const canSubmit = !busy && accepted && !loadError && !!fee && (isFree || resuming || (hasEnough && mana.isSuccess))

  const openLink = (url: string) => (event: React.MouseEvent) => {
    event.preventDefault()
    openExternal(url)
  }

  return (
    <div className={styles.step} data-testid="publish-linked-payment-step">
      <div className={styles.fields}>
        <div>
          <h3 className={styles.heading}>{t('publish_linked_modal.payment_step.title')}</h3>
          <p className={styles.text}>
            {t('publish_linked_modal.payment_step.description', {
              count: itemsToPublish.length,
              pushed: itemsToPush.length,
              name: collection.name
            })}
          </p>
        </div>

        {loadError ? (
          <p className={styles.inlineNote} data-testid="publish-linked-fee-error">
            {t('publish_linked_modal.payment_step.price_error')}
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() => {
                if (priceError) onRetryPrice()
                if (slotsError) onRetrySlots()
              }}
            >
              {t('publish_collection_modal.payment_step.retry')}
            </Button>
          </p>
        ) : !fee || slotsToBuy === null ? (
          <p className={styles.inlineNote} data-testid="publish-linked-fee-loading">
            <span className={shared.spinner} aria-hidden />
          </p>
        ) : (
          <>
            <div className={styles.summaryTable}>
              <div className={styles.tableHeader} style={{ gridTemplateColumns: SUMMARY_COLUMNS }}>
                <span>{t('publish_collection_modal.payment_step.columns.collection')}</span>
                <span>{t('publish_collection_modal.payment_step.columns.items')}</span>
                <span>{t('publish_linked_modal.payment_step.columns.slots')}</span>
                <span>{t('publish_collection_modal.payment_step.columns.total_fee')}</span>
              </div>
              <div
                className={styles.summaryRow}
                style={{ gridTemplateColumns: SUMMARY_COLUMNS }}
                data-testid="publish-linked-summary"
              >
                <div className={styles.summaryName}>
                  <span title={collection.name}>{collection.name}</span>
                </div>
                <span data-testid="publish-linked-summary-items">
                  {t('publish_collection_modal.payment_step.items_count', { count: itemsToPublish.length })}
                </span>
                <span data-testid="publish-linked-summary-slots">{slotsToBuy}</span>
                <span className={styles.summaryTotal} data-testid="publish-linked-summary-total">
                  {/* USD only, like the standard publish: the MANA it is paid in shows in the payment method below. */}
                  {isFree ? t('publish_linked_modal.payment_step.free') : usd(fee.usdWei)}
                </span>
              </div>
            </div>
            {resuming && (
              <p className={styles.inlineNote} data-testid="publish-linked-resuming">
                {t('publish_linked_modal.payment_step.resuming')}
              </p>
            )}
            {feeChanged && (
              <p className={styles.inlineNote} data-testid="publish-linked-fee-changed">
                {t('publish_linked_modal.payment_step.fee_changed')}
              </p>
            )}
            <p className={styles.text} data-testid="publish-linked-fee-note">
              {isProgrammatic
                ? isRegistered
                  ? t('publish_linked_modal.payment_step.programmatic_published')
                  : t('publish_linked_modal.payment_step.programmatic_first', { slots: price!.programmaticMinSlots })
                : t('publish_linked_modal.payment_step.standard_note', {
                    price: usd(price!.itemSlotUsdWei),
                    available: slots?.available ?? 0
                  })}
            </p>
            {!isFree && (
              <div className={styles.paymentMethods}>
                <h4 className={styles.lead}>{t('publish_collection_modal.payment_step.payment_method')}</h4>
                <PaymentMethodCard
                  method="mana"
                  price={formatMana(fee.manaWei)}
                  balance={formatMana(balance)}
                  hasEnough={hasEnough}
                  getMoreUrl={config.get('ACCOUNT_URL')}
                  selected
                  showCheckbox={false}
                  compactBuy={false}
                  disabled={false}
                  onSelect={() => undefined}
                />
              </div>
            )}
          </>
        )}

        <Checkbox checked={accepted} onChange={onAcceptedChange} testId="publish-linked-terms-accept">
          {intl.formatMessage(
            { id: 'publish_linked_modal.payment_step.checkbox' },
            {
              terms: chunks => (
                <a href={config.get('TERMS_OF_USE_URL')} onClick={openLink(config.get('TERMS_OF_USE_URL'))}>
                  {chunks}
                </a>
              ),
              policy: chunks => (
                <a href={config.get('CONTENT_POLICY_URL')} onClick={openLink(config.get('CONTENT_POLICY_URL'))}>
                  {chunks}
                </a>
              )
            }
          )}
        </Checkbox>
      </div>
      <div className={styles.footer}>
        <Button type="button" variant="secondary" data-testid="publish-back" onClick={onBack}>
          <ChevronLeftIcon fontSize="small" />
          {t('publish_collection_modal.back')}
        </Button>
        <Button type="button" disabled={!canSubmit} data-testid="publish-linked-submit" onClick={onSubmit}>
          {t(
            resuming
              ? 'publish_linked_modal.payment_step.submit_resume'
              : isFree
                ? 'publish_linked_modal.payment_step.submit_free'
                : 'publish_linked_modal.payment_step.submit'
          )}
        </Button>
      </div>
    </div>
  )
}

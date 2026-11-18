import { useEffect, useMemo, useState } from 'react'
import { ChevronLeftIcon } from '@/features/creators/components/Icons'
import { useIntl } from '@/data/lib/creators/intl'
import { config } from '@/data/lib/creators/config'
import { useTranslation } from '@/data/lib/creators/intl'
import { useCreditsBalance, useManaBalance } from '@/features/creators/hooks/useBalances'
import { useFeatureFlag } from '@/features/creators/hooks/useFeatureFlag'
import { useApproveMana, usePublishCollection, useRarities } from '@/features/creators/hooks/usePublishCollection'
import { useBeforeUnloadGuard } from '@/features/creators/hooks/useBeforeUnloadGuard'
import { track } from '@/data/lib/creators/analytics'
import { type Session } from '@/data/lib/creators/auth'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import { type Collection } from '@/data/lib/creators/collections'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { type Item } from '@/data/lib/creators/items'
import { createCreditsCheckout } from '@/data/lib/creators/credits'
import { type PackSelection, type PackTotals } from '@/data/lib/creators/creditPacks'
import { saveTopUpResume } from '@/data/lib/creators/creditsTopUp'
import { openExternal, redirectExternal } from '@/data/lib/creators/navigation'
import { verifyPublicationFee } from '@/data/lib/creators/feeVerification'
import {
  canPayWith,
  fetchCollectionManagerAllowance,
  getAvailablePaymentMethods,
  getManaApprovalNeeded,
  getMaticChainId,
  toPublishError,
  type PaymentMethod,
  type PublishCollectionError,
  type PublishFailureReason,
  type PublishResult
} from '@/data/lib/creators/publishCollection'
import { formatCredits, formatMana, getPublicationFee } from '@/data/lib/creators/publishFee'
import { Button } from '@/features/creators/components/Button'
import { InfoTooltip } from '@/features/creators/components/Tooltip'
import { Checkbox } from '@/features/creators/components/Checkbox'
import { ThumbnailMosaic } from '@/features/creators/components/ThumbnailMosaic'
import { CurrencyAmount } from '@/features/creators/components/CurrencyAmount'
import { BuyCreditsModal } from './BuyCreditsModal'
import { PaymentMethodCard } from './PaymentMethodCard'
import publishModal from './PublishCollectionModal.module.css'
import shared from '@/features/creators/styles/shared.module.css'
import method from './PaymentMethodCard.module.css'

const SUMMARY_COLUMNS = 'minmax(0, 2fr) 1fr 1fr 1fr'
// Anchors keep their href for a11y; navigation goes through the host-facing helper.
const openLink = (url: string) => (event: React.MouseEvent) => {
  event.preventDefault()
  openExternal(url)
}

type Status = 'idle' | 'checking' | 'approving' | 'confirming'

type Props = {
  collection: Collection
  items: Item[]
  session: Session
  paymentMethod: PaymentMethod | null
  onPaymentMethodChange: (method: PaymentMethod) => void
  /** The contact email confirmed in step 1, recorded with the ToS acceptance. */
  email: string
  accepted: boolean
  onAcceptedChange: (accepted: boolean) => void
  onBusyChange: (busy: boolean) => void
  onBack: () => void
  onPublished: (result: PublishResult) => void
  onFailed: (error: PublishCollectionError) => void
}

/** Step 3: fee summary, payment method, terms — and the publish transaction itself. */
export function PaymentStep({
  collection,
  items,
  session,
  paymentMethod,
  onPaymentMethodChange,
  email,
  accepted,
  onAcceptedChange,
  onBusyChange,
  onBack,
  onPublished,
  onFailed
}: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const { address } = session

  const rarities = useRarities(address)
  const credits = useCreditsBalance(address)
  const mana = useManaBalance(address)
  const fee = useMemo(() => getPublicationFee(rarities.data ?? [], items.length), [rarities.data, items.length])

  const creditsFlag = useFeatureFlag(FeatureFlag.SHOP_CREDITS_FOR_COLLECTIONS_FEE)
  const methods = useMemo(
    () => getAvailablePaymentMethods(mana.data, creditsFlag.enabled),
    [mana.data, creditsFlag.enabled]
  )
  const approve = useApproveMana(session)
  const publish = usePublishCollection(session)

  const [status, setStatus] = useState<Status>('idle')
  const [approveFailure, setApproveFailure] = useState<PublishFailureReason | null>(null)
  const [isBuyingCredits, setBuyingCredits] = useState(false)
  const isSubmitting = status !== 'idle'

  useEffect(() => {
    onBusyChange(isSubmitting)
  }, [isSubmitting, onBusyChange])
  // A wallet prompt is pending: leaving the page now would orphan the transaction.
  useBeforeUnloadGuard(isSubmitting)

  // A lone method is the selection; a vanished one (balance refetch) falls back to the first available.
  // The still-unread flag would offer a list the creator never sees, so nothing is selected until it lands.
  useEffect(() => {
    if (creditsFlag.isLoading) return
    if (paymentMethod && !methods.includes(paymentMethod)) onPaymentMethodChange(methods[0])
    else if (!paymentMethod && methods.length === 1) onPaymentMethodChange(methods[0])
  }, [methods, paymentMethod, onPaymentMethodChange, creditsFlag.isLoading])

  const balances = { credits: credits.data?.credits ?? 0, manaWei: mana.data ?? 0n }
  const creditsShortfall = Math.max(0, (fee?.total.credits ?? 0) - balances.credits)
  const selectedIsPayable =
    !!fee && !!paymentMethod && methods.includes(paymentMethod) && canPayWith(paymentMethod, fee, balances)
  const canSubmit = selectedIsPayable && accepted && !isSubmitting && !credits.isLoading && !creditsFlag.isLoading

  async function handleSubmit() {
    if (!fee || !paymentMethod || !canSubmit) return
    setApproveFailure(null)
    // The quote is builder-server's word; the contract's own price decides whether a wallet prompt is warranted.
    setStatus('checking')
    let onChainTotal: bigint
    try {
      onChainTotal = await verifyPublicationFee(fee, items, getMaticChainId())
    } catch (error) {
      setStatus('idle')
      void rarities.refetch()
      onFailed(toPublishError(error))
      return
    }
    // The contract pulls its own price, not the quote, and the allowance is read from the chain right now.
    const approval =
      paymentMethod === 'mana'
        ? await getManaApprovalNeeded(onChainTotal, () => fetchCollectionManagerAllowance(address))
        : null
    if (approval !== null) {
      setStatus('approving')
      try {
        await approve.mutateAsync(approval)
      } catch (error) {
        setStatus('idle')
        const { reason } = toPublishError(error)
        if (reason !== 'rejected') setApproveFailure(reason)
        return
      }
    }
    setStatus('confirming')
    publish.mutate(
      { collection, items, paymentMethod, fee, email },
      {
        onSuccess: onPublished,
        onError: error => {
          setStatus('idle')
          const publishError = toPublishError(error)
          // A wallet rejection is the creator changing their mind, not a failure to report.
          if (publishError.reason !== 'rejected') onFailed(publishError)
        }
      }
    )
  }

  function openBuyCredits() {
    track('Open buy credits', { collectionId: collection.id, shortfall: creditsShortfall })
    setBuyingCredits(true)
  }

  // Leaves for Stripe's hosted page; the hand-off record brings the creator back to this step.
  async function buyCredits(selection: PackSelection, totals: PackTotals) {
    const checkout = await createCreditsCheckout(address, selection)
    // Blocked storage only costs the wizard's reopening; the credits still land, so the checkout goes on.
    const resumeSaved = saveTopUpResume({
      collectionId: collection.id,
      orderId: checkout.orderId,
      paymentMethod,
      termsAccepted: accepted,
      email
    })
    track('Start credits checkout', {
      collectionId: collection.id,
      orderId: checkout.orderId,
      packId: selection.packId,
      quantity: selection.quantity,
      credits: totals.credits,
      usd: totals.usd,
      shortfall: creditsShortfall,
      resumeSaved
    })
    redirectExternal(checkout.url)
  }

  const thumbnails = useMemo(
    () =>
      items
        .slice(0, 4)
        .map(item => ({ url: getContentsStorageUrl(item.contents[item.thumbnail]), rarity: item.rarity })),
    [items]
  )

  return (
    <div className={publishModal.step} data-testid="publish-payment-step">
      <div
        className={publishModal.fields}
        data-busy={isSubmitting || undefined}
        aria-busy={isSubmitting || undefined}
        {...(isSubmitting ? { inert: '' } : {})}
      >
        <div>
          <h3 className={publishModal.heading}>{t('publish_collection_modal.payment_step.title')}</h3>
          <p className={publishModal.text}>
            {intl.formatMessage(
              { id: 'publish_collection_modal.payment_step.description' },
              {
                count: items.length,
                name: collection.name,
                fee: chunks => (
                  <span className={publishModal.feeTerm}>
                    {chunks}
                    <InfoTooltip
                      content={
                        <>
                          <strong>{t('publish_collection_modal.payment_step.fee_tooltip_title')}</strong>
                          {'\n'}
                          {t('publish_collection_modal.payment_step.fee_tooltip_body')}
                        </>
                      }
                      testId="publish-fee-tooltip"
                    />
                  </span>
                )
              }
            )}
          </p>
        </div>

        <div className={publishModal.summaryTable}>
          <div className={publishModal.tableHeader} style={{ gridTemplateColumns: SUMMARY_COLUMNS }}>
            <span>{t('publish_collection_modal.payment_step.columns.collection')}</span>
            <span>{t('publish_collection_modal.payment_step.columns.items')}</span>
            <span>{t('publish_collection_modal.payment_step.columns.fee_per_item')}</span>
            <span>{t('publish_collection_modal.payment_step.columns.total_fee')}</span>
          </div>
          <div
            className={publishModal.summaryRow}
            style={{ gridTemplateColumns: SUMMARY_COLUMNS }}
            data-testid="publish-summary"
          >
            <div className={publishModal.summaryName}>
              <span className={publishModal.mosaicFrame} aria-hidden>
                <ThumbnailMosaic thumbnails={thumbnails} testId="publish-summary-mosaic" />
              </span>
              <span title={collection.name}>{collection.name}</span>
            </div>
            <span data-testid="publish-summary-items">
              {t('publish_collection_modal.payment_step.items_count', { count: items.length })}
            </span>
            <span data-testid="publish-summary-fee-per-item">
              {fee ? <CurrencyAmount currency="credits">{formatCredits(fee.perItem.credits)}</CurrencyAmount> : '—'}
            </span>
            <span className={publishModal.summaryTotal} data-testid="publish-summary-total">
              {fee ? <CurrencyAmount currency="credits">{formatCredits(fee.total.credits)}</CurrencyAmount> : '—'}
            </span>
          </div>
        </div>

        <div className={publishModal.paymentMethods}>
          <h4 className={publishModal.lead}>{t('publish_collection_modal.payment_step.payment_method')}</h4>
          {rarities.isError ? (
            <p className={publishModal.inlineNote} data-testid="publish-fee-error">
              {t('publish_collection_modal.payment_step.fee_error')}
              <Button type="button" variant="secondary" size="sm" onClick={() => void rarities.refetch()}>
                {t('publish_collection_modal.payment_step.retry')}
              </Button>
            </p>
          ) : !fee || creditsFlag.isLoading ? (
            <p className={publishModal.inlineNote} data-testid="publish-fee-loading">
              <span className={shared.spinner} aria-hidden />
            </p>
          ) : (
            <div className={method.methods} data-testid="payment-methods" data-disabled={isSubmitting || undefined}>
              {methods.map(method => (
                <PaymentMethodCard
                  key={method}
                  method={method}
                  price={method === 'credits' ? formatCredits(fee.total.credits) : formatMana(fee.total.manaWei)}
                  balance={method === 'credits' ? formatCredits(balances.credits) : formatMana(balances.manaWei)}
                  note={
                    method === 'mana'
                      ? t('publish_collection_modal.payment_step.rate', {
                          mana: formatMana(fee.manaPerCredit)
                        })
                      : undefined
                  }
                  hasEnough={canPayWith(method, fee, balances)}
                  getMoreUrl={method === 'credits' ? `${config.get('SHOP_URL')}/credits` : config.get('ACCOUNT_URL')}
                  onGetMore={method === 'credits' ? openBuyCredits : undefined}
                  selected={paymentMethod === method}
                  showCheckbox={methods.length > 1}
                  compactBuy={methods.length > 1}
                  disabled={isSubmitting}
                  onSelect={() => onPaymentMethodChange(method)}
                />
              ))}
            </div>
          )}
          {approveFailure && (
            <p className={publishModal.errorText} data-testid="publish-approve-error">
              {t(
                approveFailure === 'wallet'
                  ? 'wallet_error.wrong_account'
                  : 'publish_collection_modal.payment_step.approve_error'
              )}
            </p>
          )}
        </div>

        <Checkbox checked={accepted} disabled={isSubmitting} onChange={onAcceptedChange} testId="publish-terms-accept">
          {intl.formatMessage(
            { id: 'publish_collection_modal.payment_step.checkbox' },
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
      <div className={publishModal.footer}>
        <Button type="button" variant="secondary" disabled={isSubmitting} data-testid="publish-back" onClick={onBack}>
          <ChevronLeftIcon fontSize="small" />
          {t('publish_collection_modal.back')}
        </Button>
        <Button
          type="button"
          loading={isSubmitting}
          disabled={!canSubmit}
          data-testid="publish-submit"
          onClick={() => void handleSubmit()}
        >
          {t('publish_collection_modal.payment_step.submit')}
        </Button>
      </div>
      {isBuyingCredits && (
        <BuyCreditsModal
          balance={balances.credits}
          shortfall={creditsShortfall}
          onCancel={() => setBuyingCredits(false)}
          onBuy={buyCredits}
        />
      )}
    </div>
  )
}

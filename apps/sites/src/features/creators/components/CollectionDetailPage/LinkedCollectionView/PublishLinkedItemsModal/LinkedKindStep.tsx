import { ChevronLeftIcon, ChevronRightIcon } from '@/features/creators/components/Icons'
import { useIntl } from '@/data/lib/creators/intl'
import { useTranslation } from '@/data/lib/creators/intl'
import { weiToUsdCents } from '@/data/lib/creators/publishFee'
import { type ThirdPartyPrice } from '@/data/lib/creators/thirdPartyRegistry'
import { Button } from '@/features/creators/components/Button'
import publishModal from '@/features/creators/components/CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'
import publishRow from './LinkedPublishRow.module.css'

type Props = {
  value: boolean
  price: ThirdPartyPrice | undefined
  priceError: boolean
  onRetryPrice: () => void
  onChange: (isProgrammatic: boolean) => void
  onBack: () => void
  onConfirm: () => void
}

/** First publish only: standard (a fee per item) or programmatic (one fee for a pack of slots, then free). */
export function LinkedKindStep({ value, price, priceError, onRetryPrice, onChange, onBack, onConfirm }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const usd = (wei: bigint) => intl.formatNumber(weiToUsdCents(wei) / 100, { style: 'currency', currency: 'USD' })

  return (
    <div className={publishModal.step} data-testid="publish-linked-kind-step">
      <h3 className={publishModal.heading}>{t('publish_linked_modal.kind_step.title')}</h3>
      <p className={publishModal.text}>{t('publish_linked_modal.kind_step.description')}</p>
      {priceError ? (
        <p className={publishModal.inlineNote} data-testid="publish-linked-price-error">
          {t('publish_linked_modal.payment_step.price_error')}
          <Button type="button" variant="secondary" size="sm" onClick={onRetryPrice}>
            {t('publish_collection_modal.payment_step.retry')}
          </Button>
        </p>
      ) : (
        <div className={publishRow.kindCards} role="radiogroup" aria-label={t('publish_linked_modal.kind_step.title')}>
          {[false, true].map(programmatic => (
            <button
              className={publishRow.kindCard}
              key={String(programmatic)}
              type="button"
              role="radio"
              aria-checked={value === programmatic}
              data-selected={value === programmatic || undefined}
              data-testid={`publish-linked-kind-${programmatic ? 'programmatic' : 'standard'}`}
              onClick={() => onChange(programmatic)}
            >
              <span className={publishRow.kindTitle}>
                {t(`linked_collection.kind.${programmatic ? 'programmatic' : 'standard'}`)}
              </span>
              <span className={publishRow.kindText}>
                {t(`publish_linked_modal.kind_step.${programmatic ? 'programmatic' : 'standard'}_description`)}
              </span>
              <span
                className={publishRow.kindPrice}
                data-testid={`publish-linked-kind-${programmatic ? 'programmatic' : 'standard'}-price`}
              >
                {price
                  ? programmatic
                    ? t('publish_linked_modal.kind_step.programmatic_price', {
                        price: usd(price.itemSlotUsdWei * BigInt(price.programmaticMinSlots)),
                        slots: price.programmaticMinSlots
                      })
                    : t('publish_linked_modal.kind_step.standard_price', { price: usd(price.itemSlotUsdWei) })
                  : '…'}
              </span>
            </button>
          ))}
        </div>
      )}
      <div className={publishModal.footer}>
        <Button type="button" variant="secondary" data-testid="publish-back" onClick={onBack}>
          <ChevronLeftIcon fontSize="small" />
          {t('publish_collection_modal.back')}
        </Button>
        <Button type="button" disabled={!price} data-testid="publish-linked-kind-confirm" onClick={onConfirm}>
          {t('publish_linked_modal.kind_step.confirm')}
          <ChevronRightIcon fontSize="small" />
        </Button>
      </div>
    </div>
  )
}

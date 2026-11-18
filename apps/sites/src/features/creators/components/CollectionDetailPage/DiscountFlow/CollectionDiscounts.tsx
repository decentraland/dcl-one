import { useIntl } from 'react-intl'
import { useTranslation } from '~/intl'
import { discountPct, isDiscountCapped, liveDiscountStatus, type CollectionDiscount } from '~/lib/coupons'
import { Button } from '~/components/Button'
import { SaleTag } from '../SaleTag'
import styles from './DiscountFlow.module.css'

type Props = {
  discounts: CollectionDiscount[]
  /** The same clock the list was filtered with, so a discount's status and its presence agree. */
  now: number
  /** Omitted when the creator can't end a discount early (a social login can't pay the Polygon gas). */
  onEnd?: (discount: CollectionDiscount) => void
}

/** The collection's running and scheduled discounts, each with a way to end it early when the creator can. */
export function CollectionDiscounts({ discounts, now, onEnd }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  if (discounts.length === 0) return null
  const date = (ms: number) => intl.formatDate(ms, { month: 'short', day: 'numeric' })
  return (
    <ul
      className={styles.discounts}
      aria-label={t('collection_discount.list_label')}
      data-testid="collection-discounts"
    >
      {discounts.map(discount => {
        const scheduled = liveDiscountStatus(discount, now) === 'scheduled'
        return (
          <li
            className={styles.discountChip}
            key={discount.id}
            data-status={scheduled ? 'scheduled' : 'active'}
            data-testid="collection-discount"
          >
            <div className={styles.discountTerms}>
              <SaleTag pct={discountPct(discount)} testId="discount-sale-tag" />
              <span>
                {scheduled
                  ? t('collection_discount.chip.starts', { date: date(discount.checks.effective) })
                  : t('collection_discount.chip.ends', { date: date(discount.checks.expiration) })}
              </span>
              {isDiscountCapped(discount) && (
                <span>
                  {t('collection_discount.chip.uses', { used: discount.state?.uses ?? 0, total: discount.checks.uses })}
                </span>
              )}
              {discount.ending && <span data-testid="discount-ending">{t('collection_discount.chip.ending')}</span>}
              {discount.collections.length > 1 && (
                <span data-testid="discount-other-collections">
                  {t('collection_discount.chip.others', { count: discount.collections.length - 1 })}
                </span>
              )}
            </div>
            {onEnd && (
              <Button
                type="button"
                variant="secondary"
                size="sm"
                data-testid="end-discount"
                // Its cancellation is already on its way: a second one would only cost gas.
                disabled={discount.ending}
                onClick={() => onEnd(discount)}
              >
                {t('collection_discount.end.action')}
              </Button>
            )}
          </li>
        )
      })}
    </ul>
  )
}

import { useMemo, useState, type FormEvent } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import type { Item } from '@/data/lib/creators/items'
import type { ItemListing } from '@/data/lib/creators/listings'
import { isSamePrice, listingToSalePrice, toPricedSale, type PricedSale } from '@/data/lib/creators/sales'
import { Button } from '../../Button'
import { Modal } from '../../Modal'
import { PriceField, type PriceFormValues } from './PriceField'
import { SellItemCard } from './SellItemCard'
import styles from './SellItemModal.module.css'

type Props = {
  item: Item
  /** The listing being re-priced: its currency is the default, and the same price can't be "updated" to itself. */
  listing: ItemListing
  /** Restores a previous attempt's form. */
  initialValues?: PriceFormValues
  onSubmit: (values: PriceFormValues, price: PricedSale) => void
  onClose: () => void
}

/** Edit Price: the new amount and currency; beneficiary and expiration carry over from the listing. */
export function UpdatePriceModal({ item, listing, initialValues, onSubmit, onClose }: Props) {
  const { t } = useTranslation()
  const [values, setValues] = useState<PriceFormValues>(initialValues ?? { currency: listing.currency, amount: '' })
  const current = useMemo(() => listingToSalePrice(listing), [listing])
  const price = useMemo(() => toPricedSale(values.currency, values.amount), [values])
  const canSubmit = price !== null && !isSamePrice(price, current)

  function handleSubmit(event: FormEvent) {
    event.preventDefault()
    if (price !== null && canSubmit) onSubmit(values, price)
  }

  return (
    <Modal title={t('sell_item_modal.update_price.title')} compact onClose={onClose} testId="update-price-modal">
      <hr className={styles.divider} />
      <form className={styles.form} onSubmit={handleSubmit} data-testid="update-price-form">
        <div className={styles.fields}>
          <SellItemCard item={item} showAvailability />
          <div className={styles.field}>
            <PriceField
              label={t('sell_item_modal.update_price.price_label')}
              values={values}
              onChange={setValues}
              testId="update-price"
            />
          </div>
        </div>
        <div className={styles.footer}>
          <Button type="button" variant="secondary" data-testid="update-price-cancel" onClick={onClose}>
            {t('sell_item_modal.cancel')}
          </Button>
          <Button type="submit" disabled={!canSubmit} data-testid="update-price-submit">
            {t('sell_item_modal.update_price.submit')}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

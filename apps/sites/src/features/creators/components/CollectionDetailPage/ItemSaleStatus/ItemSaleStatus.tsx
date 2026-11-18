import { useTranslation } from '@/data/lib/creators/intl'
import type { ItemSales } from '@/data/lib/creators/items'
import type { ItemListing } from '@/data/lib/creators/listings'
import { openExternal } from '@/data/lib/creators/navigation'
import { PriceTagIcon, CheckIcon, OpenInNewIcon } from '../../Icons'
import { cx } from '@/data/lib/creators/cx'
import { Button } from '../../Button'
import styles from './ItemSaleStatus.module.css'

const EMPTY = '—'

type Props = {
  sales: ItemSales | undefined
  /** The item's primary listing: `null` when it has none, `undefined` while listings are still loading. */
  listing: ItemListing | null | undefined
  /** Whether the collection has been approved at least once; until then the "Put on sale" CTA is disabled. */
  canSell?: boolean
  /** Absent for viewers who may not sell (only the owner can), who get a dash instead of the CTA. */
  onPutOnSale?: () => void
  /** The item's page in the Shop; the ON SALE pill links there. */
  shopUrl?: string
}

/** Sold out / on sale pill, or the "Put on sale" CTA (a dash for those who may not sell); nothing while listings load. */
export function ItemSaleStatus({ sales, listing, canSell = false, onPutOnSale, shopUrl }: Props) {
  const { t } = useTranslation()

  if (sales && sales.minted >= sales.maxSupply) {
    return (
      <span className={styles.pill} data-testid="item-sale-status" data-status="sold_out">
        {t('collection_detail_page.sale_status.sold_out')}
      </span>
    )
  }
  if (listing === undefined) return null
  if (listing) {
    if (!shopUrl) {
      return (
        <span className={styles.pill} data-testid="item-sale-status" data-status="on_sale">
          <CheckIcon aria-hidden />
          {t('collection_detail_page.sale_status.on_sale')}
          <span className={styles.dot} aria-hidden />
        </span>
      )
    }
    return (
      <a
        className={cx(styles.pill, styles.pillLink)}
        href={shopUrl}
        title={t('collection_detail_page.actions.view_in_shop')}
        data-testid="item-sale-status"
        data-status="on_sale"
        onClick={event => {
          event.preventDefault()
          openExternal(shopUrl)
        }}
      >
        <CheckIcon aria-hidden />
        {t('collection_detail_page.sale_status.on_sale')}
        {/* The dot gives way to the external-link glyph on hover/focus, so the pill reads as a link. */}
        <span className={styles.trailing} aria-hidden>
          <span className={styles.dot} />
          <OpenInNewIcon />
        </span>
      </a>
    )
  }
  if (!onPutOnSale)
    return (
      <span data-testid="item-sale-status" data-status="not_on_sale">
        {EMPTY}
      </span>
    )
  return (
    <Button
      className={styles.putOnSale}
      data-testid="item-sale-status"
      data-status="not_on_sale"
      type="button"
      disabled={!canSell}
      title={canSell ? undefined : t('collection_detail_page.sale_status.awaiting_approval')}
      onClick={onPutOnSale}
    >
      <PriceTagIcon />
      {t('collection_detail_page.sale_status.put_on_sale')}
    </Button>
  )
}

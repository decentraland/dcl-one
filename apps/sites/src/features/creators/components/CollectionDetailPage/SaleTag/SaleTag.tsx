import { useIntl } from 'react-intl'
import { useTranslation } from '~/intl'
import { cx } from '~/lib/cx'
import styles from './SaleTag.module.css'

type Props = {
  /** Percent off; a non-whole one shows up to two decimals in the viewer's locale. Zero falls back to the plain word. */
  pct: number
  className?: string
  testId?: string
}

/** How much a discount takes off, drawn like the Shop's tag so it reads the same in both. */
export function SaleTag({ pct, className, testId }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  return (
    <span className={cx(styles.root, className)} data-testid={testId}>
      {/* Decorative: the number beside it carries the meaning. */}
      <span aria-hidden>🔥</span>
      <b className={styles.pct}>
        {pct > 0
          ? t('collection_detail_page.price.off', { pct: intl.formatNumber(pct, { maximumFractionDigits: 2 }) })
          : t('collection_detail_page.price.sale')}
      </b>
    </span>
  )
}

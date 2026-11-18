import type { CSSProperties } from 'react'
import creditsGlyph from '../../../assets/icons/credits.svg'
import manaGlyph from '../../../assets/icons/mana-matic.svg'
import { cx } from '@/data/lib/creators/cx'
import styles from './SaleIcon.module.css'

/** A currency mark, as the Shop draws it beside an amount: a mask tinted with currentColor. Credits unless told. */
export function CurrencyIcon({
  className,
  size,
  inline = false,
  currency = 'credits'
}: {
  className?: string
  /** Sized to the text around it (1em) instead of 20px. */
  inline?: boolean
  size?: number
  currency?: 'credits' | 'mana'
}) {
  const style = {
    '--icon-url': `url("${currency === 'mana' ? manaGlyph : creditsGlyph}")`,
    ...(size ? { width: size, height: size } : {})
  } as CSSProperties
  return (
    <span
      className={cx(styles.mark, className)}
      data-sale-icon
      data-inline={inline || undefined}
      aria-hidden
      style={style}
    />
  )
}

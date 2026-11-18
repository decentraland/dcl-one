import { type CSSProperties, type ReactNode } from 'react'
import creditsGlyph from '~/assets/icons/credits.svg'
import manaGlyph from '~/assets/icons/mana-matic.svg'
import styles from './CurrencyAmount.module.css'

export type Currency = 'credits' | 'mana'

const GLYPHS: Record<Currency, string> = { credits: creditsGlyph, mana: manaGlyph }

type Props = {
  currency: Currency
  children: ReactNode
}

/** An amount prefixed with its currency glyph — "Ⓒ 300" / "◈ 500". */
export function CurrencyAmount({ currency, children }: Props) {
  return (
    <>
      <span
        className={styles.mark}
        aria-hidden
        style={{ '--icon-url': `url("${GLYPHS[currency]}")` } as CSSProperties}
      />
      {children}
    </>
  )
}

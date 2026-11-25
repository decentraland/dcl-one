import type { HTMLAttributes, ReactNode } from 'react'
import { GeneralSmartItem } from '../../foundations/icons/general'
import styles from './ItemLabel.module.css'

export type ItemLabelVariant = 'new' | 'smart' | 'amount'

export type ItemLabelProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  variant: ItemLabelVariant
  /**
   * The text. `new` and `smart` default to "New" and "Smart", drawn in capitals; pass the
   * localized word. `amount` shows what the caller gives, such as "x2".
   */
  children?: ReactNode
}

const DEFAULT_TEXT: Partial<Record<ItemLabelVariant, string>> = { new: 'New', smart: 'Smart' }

/**
 * A small label over or beside an item's picture. It is text, not a control; the smart mark is
 * decorative. A caller whose amount is not clear from its text (a bare "x2") names the label.
 */
export function ItemLabel({ variant, children, className, ...rest }: ItemLabelProps) {
  return (
    <span {...rest} className={[styles.root, className].filter(Boolean).join(' ')} data-variant={variant}>
      {variant === 'smart' ? (
        <GeneralSmartItem className={styles.icon} size={12} focusable="false" aria-hidden="true" />
      ) : null}
      {children ?? DEFAULT_TEXT[variant]}
    </span>
  )
}

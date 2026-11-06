// ShopModal — the New Shop's dialog ("Modals"): a 600px purple card with a pink glow off its top-right
// corner and another under its bottom edge, a centred 396px column, and its CTA row. Presentational: the
// host draws the scrim, places the card and owns focus and Escape.

import glowBottom from '../../../assets/shop/modal/glow-bottom.svg'
import glowTop from '../../../assets/shop/modal/glow-top.svg'
import styles from './ShopModal.module.css'

export interface ShopModalProps {
  /** The dialog's accessible name (its title). */
  label: string
  children: React.ReactNode
  /** The CTA row under the content. */
  actions?: React.ReactNode
  /** Work in progress: the content is a spinner (Processing), and the bottom glow sits higher, as drawn. */
  busy?: boolean
}

export function ShopModal({ label, children, actions, busy = false }: ShopModalProps): React.JSX.Element {
  return (
    <div className={`${styles.modal} ${busy ? styles.busy : ''}`.trim()} role="dialog" aria-modal="true" aria-label={label} aria-busy={busy || undefined}>
      <div className={styles.content}>
        {children}
        {actions != null && <div className={styles.actions}>{actions}</div>}
      </div>
      <img className={styles.glowTop} src={glowTop} alt="" aria-hidden="true" />
      <img className={styles.glowBottom} src={glowBottom} alt="" aria-hidden="true" />
    </div>
  )
}

/** The design's invisible separators: each adds one more gap of the column's spacing. */
export function ShopModalGap({ count = 1 }: { count?: number }): React.JSX.Element {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <span key={i} className={styles.gap} aria-hidden="true" />
      ))}
    </>
  )
}

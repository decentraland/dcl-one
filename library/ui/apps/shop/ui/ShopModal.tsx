// ShopModal — the design system's Explorer modal (explorer-modal/no-title-bar/…/desktop), as the Shop's dialogs use it:
// a 600px Brand/Purple Dark card with a pink glow off its top-right corner and another under its bottom edge, a centred
// 472px column whose sections stand 32px apart, and its CTA row last. Presentational: the host draws the scrim, places
// the card and owns focus and Escape.

import styles from './ShopModal.module.css'

export interface ShopModalProps {
  /** The dialog's accessible name (its title). */
  label: string
  children: React.ReactNode
  /** The CTA row under the content. */
  actions?: React.ReactNode
  /** Work in progress (Processing): said to assistive technology. */
  busy?: boolean
}

export function ShopModal({ label, children, actions, busy = false }: ShopModalProps): React.JSX.Element {
  return (
    <div className={styles.modal} role="dialog" aria-modal="true" aria-label={label} aria-busy={busy || undefined}>
      <div className={styles.content}>
        {children}
        {actions != null && <div className={styles.actions}>{actions}</div>}
      </div>
    </div>
  )
}

/**
 * A section's header: its illustration (if any), its title, and what stands under the title (a balance, a count),
 * 16px apart.
 */
export function ShopModalHeader({ children }: { children: React.ReactNode }): React.JSX.Element {
  return <div className={styles.header}>{children}</div>
}

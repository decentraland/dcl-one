// MenuPanel — the panel a navigation button opens beside the bar: a flat Shadow surface with a
// title and a list of rows. MenuRow is one row: an icon, a label, the key that does the same, and
// an optional trailing mark (a count, a live badge).

import type { ReactNode } from 'react'
import { Panel } from './Panel'
import styles from './MenuPanel.module.css'

interface MenuPanelProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'title'> {
  /** The accessible name of the panel. */
  label: string
  /** The heading drawn at the top, e.g. "DISCOVER"; none when omitted. */
  title?: ReactNode
  /** 274 (default), or 400 for a panel with pictures. */
  width?: 274 | 400
}

export function MenuPanel({ label, title, width = 274, className = '', children, ...rest }: MenuPanelProps): React.JSX.Element {
  return (
    <Panel role="dialog" aria-label={label} className={`${styles.panel} ${width === 400 ? styles.wide : ''} ${className}`.replace(/\s+/g, ' ').trim()} {...rest}>
      {title != null && <h2 className={styles.title}>{title}</h2>}
      {children}
    </Panel>
  )
}

/** The rows of a MenuPanel, stacked with no gap. */
export function MenuList({ children }: { children: ReactNode }): React.JSX.Element {
  return <div className={styles.list}>{children}</div>
}

interface MenuRowProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Drawn in a 28px box before the label. */
  icon?: ReactNode
  label: string
  /** The key bound to the same action, drawn as "[key]" after the label. */
  hint?: string
  /** At the row's far end: a count, a status mark. */
  trailing?: ReactNode
}

export function MenuRow({ icon, label, hint, trailing, className = '', type = 'button', ...rest }: MenuRowProps): React.JSX.Element {
  return (
    <button type={type} className={`${styles.row} ${className}`.trim()} {...rest}>
      {icon != null && (
        <span className={styles.icon} aria-hidden="true">
          {icon}
        </span>
      )}
      <span className={styles.label}>{label}</span>
      {hint != null && hint !== '' && (
        <span className={styles.hint} aria-hidden="true">
          [{hint}]
        </span>
      )}
      {trailing != null && <span className={styles.trailing}>{trailing}</span>}
    </button>
  )
}

/** The hairline between two groups of rows. */
export function MenuDivider(): React.JSX.Element {
  return <div className={styles.divider} role="separator" />
}

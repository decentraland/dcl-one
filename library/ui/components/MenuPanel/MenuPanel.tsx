import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from 'react'
import { Panel } from '../Panel/Panel'
import styles from './MenuPanel.module.css'

export type MenuPanelProps = Omit<HTMLAttributes<HTMLDivElement>, 'title'> & {
  /** The accessible name of the panel. */
  label: string
  /** The heading drawn at the top, for example "DISCOVER"; none when omitted. */
  title?: ReactNode
  /** 274 by default, or 400 for a panel with pictures. */
  width?: 274 | 400
}

/**
 * The panel a navigation button opens beside the bar: a dialog with an optional title over its
 * content, usually a `MenuList` of `MenuRow`s. The caller owns opening, closing and focus.
 */
export function MenuPanel({ label, title, width = 274, className, children, ...rest }: MenuPanelProps) {
  return (
    <Panel
      role="dialog"
      aria-label={label}
      {...rest}
      className={[styles.root, width === 400 && styles.wide, className].filter(Boolean).join(' ')}
      data-width={width}
    >
      {title != null && <h2 className={styles.title}>{title}</h2>}
      {children}
    </Panel>
  )
}

/** The rows of a MenuPanel, stacked with no gap. */
export function MenuList({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={[styles.list, className].filter(Boolean).join(' ')}>{children}</div>
}

export type MenuRowProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> & {
  /** Drawn in a 28px box before the label, hidden from assistive technology. */
  icon?: ReactNode
  label: string
  /** The key bound to the same action, drawn as "[key]" after the label and not read. */
  hint?: string
  /** At the row's far end: a count, a status mark. */
  trailing?: ReactNode
}

/** One row of a MenuPanel: a native button with an icon, a label, a key hint and a trailing mark. */
export function MenuRow({ icon, label, hint, trailing, className, type = 'button', ...rest }: MenuRowProps) {
  return (
    <button {...rest} type={type} className={[styles.row, className].filter(Boolean).join(' ')}>
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
export function MenuDivider() {
  return <div className={styles.divider} role="separator" />
}

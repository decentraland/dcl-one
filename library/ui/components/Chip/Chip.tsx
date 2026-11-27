import type { ReactNode } from 'react'
import styles from './Chip.module.css'

export type ChipProps = {
  label: string
  /** Drawn before the label. */
  icon?: ReactNode
  /** The orange look, reported as `aria-pressed`. */
  selected?: boolean
  /**
   * `light` is an off-white box, 42px high, for a breadcrumb of filters. `dark` is a translucent
   * 40px pill for a row of categories over a dark page.
   */
  variant?: 'light' | 'dark'
  onClick?: () => void
  /** Adds a clear button after the label. Its outline and glyph are drawn for a `selected` chip. */
  onClear?: () => void
  /** The clear button's accessible name. */
  clearLabel?: string
  /** The clear button's glyph, 10px. A cross by default. */
  clearIcon?: ReactNode
}

const cross = (
  <svg width={10} height={10} viewBox="0 0 10 10" fill="none" focusable="false" aria-hidden="true">
    <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" strokeWidth={1.6} strokeLinecap="round" />
  </svg>
)

/**
 * A filter chip: a toggle with an optional icon, and an optional clear button beside it. The chip
 * is two native buttons, so clearing a filter is reachable on its own.
 */
export function Chip({
  label,
  icon,
  selected = false,
  variant = 'light',
  onClick,
  onClear,
  clearLabel = 'Clear',
  clearIcon = cross
}: ChipProps) {
  const classes = [styles.root, variant === 'dark' && styles.dark, selected && styles.selected]
    .filter(Boolean)
    .join(' ')
  return (
    <span className={classes}>
      <button type="button" className={styles.main} aria-pressed={selected} onClick={onClick}>
        {icon}
        <span className={styles.label}>{label}</span>
      </button>
      {onClear != null && (
        <button type="button" className={styles.clear} aria-label={clearLabel} onClick={onClear}>
          {clearIcon}
        </button>
      )}
    </span>
  )
}

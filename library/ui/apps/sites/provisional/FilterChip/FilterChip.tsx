// TODO(figma-ds): replace with the Figma-generated filter chip from library/ui/components.
import type { ReactNode } from 'react'
import styles from './FilterChip.module.css'

type FilterChipProps = {
  label: string
  /** A decorative glyph before the label. */
  icon?: ReactNode
  active: boolean
  onSelect: () => void
}

type FilterChipGroupProps = {
  /** Names the group for assistive technology, e.g. "Places filter". */
  label: string
  children: ReactNode
}

/** A toggle pill: outlined when off, filled white when on, with its state exposed as pressed. */
function FilterChip({ label, icon, active, onSelect }: FilterChipProps) {
  return (
    <button
      type="button"
      className={active ? `${styles.chip} ${styles.active}` : styles.chip}
      aria-pressed={active}
      onClick={onSelect}
    >
      {icon ? (
        <span className={styles.icon} aria-hidden="true">
          {icon}
        </span>
      ) : null}
      {label}
    </button>
  )
}

/** The row the chips of one filter sit in, wrapping on narrow screens. */
function FilterChipGroup({ label, children }: FilterChipGroupProps) {
  return (
    <div className={styles.group} role="group" aria-label={label}>
      {children}
    </div>
  )
}

export { FilterChip, FilterChipGroup }
export type { FilterChipGroupProps, FilterChipProps }

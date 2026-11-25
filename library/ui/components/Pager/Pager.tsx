import type { ReactNode } from 'react'
import styles from './Pager.module.css'

const WINDOW = 5

/**
 * The pages shown: the current one stays centred past the first half, and the window clamps at both
 * ends. Exported for its tests.
 */
export function pageWindow(current: number, count: number): number[] {
  const size = Math.min(WINDOW, count)
  const half = Math.floor(WINDOW / 2)
  const start = current > half ? Math.min(current - half, count - size) : 0
  return Array.from({ length: size }, (_, index) => start + index)
}

export type PagerProps = {
  /** The current page, counted from zero. */
  page: number
  /** How many pages there are. Nothing renders for one page or none. */
  count: number
  onChange: (page: number) => void
  /**
   * `backpack` has 34px page buttons and violet arrows. `shop` has 38px round pages, white-washed
   * arrows, and an ellipsis while more pages follow the window.
   */
  variant?: 'backpack' | 'shop'
  /** The glyph of the previous arrow, 12px; the next arrow mirrors it. A chevron by default. */
  arrowIcon?: ReactNode
  /** The landmark's accessible name. "Pages" by default. */
  label?: string
  /** The previous arrow's accessible name. "Previous page" by default. */
  previousLabel?: string
  /** The next arrow's accessible name. "Next page" by default. */
  nextLabel?: string
  className?: string
}

const chevron = (
  <svg width={12} height={12} viewBox="0 0 12 12" fill="none" focusable="false" aria-hidden="true">
    <path d="M8 1.5L3.5 6 8 10.5" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
  </svg>
)

/**
 * Previous and next arrows around a window of up to five page numbers. The arrows wrap: previous on
 * the first page goes to the last.
 */
export function Pager({
  page,
  count,
  onChange,
  variant = 'backpack',
  arrowIcon = chevron,
  label = 'Pages',
  previousLabel = 'Previous page',
  nextLabel = 'Next page',
  className
}: PagerProps) {
  if (!(count > 1)) return null
  const pages = pageWindow(page, count)
  const more = variant === 'shop' && pages[pages.length - 1]! < count - 1
  return (
    <nav
      className={[styles.root, variant === 'shop' && styles.shop, className].filter(Boolean).join(' ')}
      data-variant={variant}
      aria-label={label}
    >
      <button
        type="button"
        className={styles.arrow}
        aria-label={previousLabel}
        onClick={() => onChange(page === 0 ? count - 1 : page - 1)}
      >
        <span className={styles.glyph}>{arrowIcon}</span>
      </button>
      <span className={styles.pages}>
        {pages.map((index) => (
          <button
            key={index}
            type="button"
            className={[styles.page, index === page && styles.current].filter(Boolean).join(' ')}
            aria-current={index === page ? 'page' : undefined}
            onClick={() => onChange(index)}
          >
            {index + 1}
          </button>
        ))}
        {more && (
          <span className={styles.more} aria-hidden="true">
            ...
          </span>
        )}
      </span>
      <button
        type="button"
        className={`${styles.arrow} ${styles.next}`}
        aria-label={nextLabel}
        onClick={() => onChange(page >= count - 1 ? 0 : page + 1)}
      >
        <span className={styles.glyph}>{arrowIcon}</span>
      </button>
    </nav>
  )
}

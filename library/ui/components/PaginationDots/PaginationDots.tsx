import styles from './PaginationDots.module.css'

export type PaginationDotsProps = {
  /** How many pages there are. Nothing renders for one page or none. */
  count: number
  /** The page in view, counted from zero. */
  current: number
  /** Called with the page a dot stands for, counted from zero. */
  onSelect: (page: number) => void
  /**
   * `default` is Figma's `pagination-dots`: 14px dots and a 40px pill for the current page. `plain` is
   * decentraland.org's carousels and rails: 8px dots, the current one white.
   */
  appearance?: 'default' | 'plain'
  /** The row's accessible name. With one, the dots are a named group. */
  label?: string
  /** A dot's accessible name, from its page counted from one. "Go to slide N" by default. */
  pageLabel?: (page: number, count: number) => string
  className?: string
}

const slideName = (page: number): string => `Go to slide ${page}`

/**
 * A row of dots, one button per page, under a carousel or a rail. The current page's dot carries
 * `aria-current`; pressing a dot asks for its page.
 */
export function PaginationDots({
  count,
  current,
  onSelect,
  appearance = 'default',
  label,
  pageLabel = slideName,
  className
}: PaginationDotsProps) {
  if (!(count > 1)) return null
  return (
    <div
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-appearance={appearance}
      {...(label ? { role: 'group', 'aria-label': label } : null)}
    >
      {Array.from({ length: count }, (_, page) => (
        <button
          key={page}
          type="button"
          className={styles.dot}
          aria-label={pageLabel(page + 1, count)}
          aria-current={page === current ? 'true' : undefined}
          onClick={() => onSelect(page)}
        />
      ))}
    </div>
  )
}

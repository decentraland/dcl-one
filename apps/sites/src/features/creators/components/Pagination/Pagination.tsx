import { ChevronLeftIcon, ChevronRightIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { pageWindow } from '~/lib/pagination'
import styles from './Pagination.module.css'

type Props = {
  page: number
  pages: number
  onPageChange: (page: number) => void
}

/** Windowed page numbers + chevrons. Render only when there is more than one page. */
export function Pagination({ page, pages, onPageChange }: Props) {
  const { t } = useTranslation()
  return (
    <div className={styles.root} data-testid="pagination">
      <button
        className={styles.pageButton}
        type="button"
        aria-label={t('pagination.previous')}
        disabled={page <= 1}
        onClick={() => onPageChange(page - 1)}
      >
        <ChevronLeftIcon fontSize="small" />
      </button>
      {pageWindow(page, pages).map(n => (
        <button
          className={styles.pageButton}
          key={n}
          type="button"
          data-current={n === page || undefined}
          aria-current={n === page ? 'page' : undefined}
          onClick={() => onPageChange(n)}
        >
          {n}
        </button>
      ))}
      <button
        className={styles.pageButton}
        type="button"
        aria-label={t('pagination.next')}
        disabled={page >= pages}
        onClick={() => onPageChange(page + 1)}
      >
        <ChevronRightIcon fontSize="small" />
      </button>
    </div>
  )
}

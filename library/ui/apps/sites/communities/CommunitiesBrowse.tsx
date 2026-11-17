import { useId, type ReactNode } from 'react'
import { Button } from '../../../components/Button/Button'
import { SearchBar } from '../../../components/SearchBar/SearchBar'
import styles from './CommunitiesBrowse.module.css'

type CommunitiesBrowseProps = {
  /** The visible section heading. */
  heading: string
  search: string
  onSearch: (value: string) => void
  searchLabel: string
  searchPlaceholder: string
  clearLabel: string
  /** The grid, or the message that stands in for it. */
  children: ReactNode
}

/** The /places/communities section: its heading and search, then the results. */
function CommunitiesBrowse({
  heading,
  search,
  onSearch,
  searchLabel,
  searchPlaceholder,
  clearLabel,
  children
}: CommunitiesBrowseProps) {
  const headingId = useId()
  return (
    <section className={styles.band} aria-labelledby={headingId}>
      <div className={styles.inner}>
        <div className={styles.toolbar}>
          <h2 id={headingId} className={styles.heading}>
            {heading}
          </h2>
          <SearchBar
            className={styles.search}
            label={searchLabel}
            placeholder={searchPlaceholder}
            value={search}
            onValueChange={onSearch}
            onClear={() => onSearch('')}
            clearLabel={clearLabel}
          />
        </div>
        {children}
      </div>
    </section>
  )
}

type CommunityGridProps = {
  /** Accessible name of the list. */
  label: string
  /** Loading placeholders are in the list. */
  busy?: boolean
  /** One `<li>` per card. */
  children: ReactNode
}

/** The grid: 1 column on phones, 2 on tablets, 3 on desktops and 4 on wide screens, as the places grid. */
function CommunityGrid({ label, busy = false, children }: CommunityGridProps) {
  return (
    <ul className={styles.grid} aria-label={label} aria-busy={busy || undefined}>
      {children}
    </ul>
  )
}

type CommunitiesMessageProps = {
  message: string
  /** `alert` for a failure, so it is announced; plain text otherwise. */
  tone?: 'info' | 'alert'
  action?: { label: string; onClick: () => void; loading?: boolean }
}

/** The empty and error states, or the next-page control: the text, and the way out when there is one. */
function CommunitiesMessage({ message, tone = 'info', action }: CommunitiesMessageProps) {
  return (
    <div className={styles.message}>
      {message ? (
        <p className={styles.messageText} role={tone === 'alert' ? 'alert' : undefined}>
          {message}
        </p>
      ) : null}
      {action ? (
        <Button variant="secondary" size="md" loading={action.loading} onClick={action.onClick}>
          {action.label}
        </Button>
      ) : null}
    </div>
  )
}

export { CommunitiesBrowse, CommunitiesMessage, CommunityGrid }
export type { CommunitiesBrowseProps, CommunitiesMessageProps, CommunityGridProps }

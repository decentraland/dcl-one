import { useId } from 'react'
import {
  CommunityCard,
  type CommunityCardLinkComponent
} from '../../../components/CommunityCard/CommunityCard'
import { SearchBar } from '../../../components/SearchBar/SearchBar'
import { PlacesMessage } from './PlaceGrid'
import styles from './CommunitiesDirectory.module.css'

export type DirectoryCard = {
  id: string
  name: string
  href: string
  image?: string | null
  ownerName?: string | null
  privacy: 'public' | 'private'
  membersLabel: string
  description?: string | null
}

export type CommunitiesDirectoryLabels = {
  heading: string
  search: string
  searchPlaceholder: string
  empty: string
  error: string
  retry: string
  loading: string
}

export type CommunitiesDirectoryProps = {
  search: string
  onSearch: (value: string) => void
  state: 'loading' | 'error' | 'ready'
  cards: readonly DirectoryCard[]
  onRetry: () => void
  labels: CommunitiesDirectoryLabels
  linkComponent?: CommunityCardLinkComponent
}

const SKELETONS = 8

/** The Places Communities tab, inside `PlacesPage`: a heading and a search over the communities, then their cards. */
export function CommunitiesDirectory({
  search,
  onSearch,
  state,
  cards,
  onRetry,
  labels,
  linkComponent
}: CommunitiesDirectoryProps) {
  const headingId = useId()
  return (
    <section className={styles.root} aria-labelledby={headingId}>
      <div className={styles.header}>
        <h2 id={headingId} className={styles.heading}>
          {labels.heading}
        </h2>
        <SearchBar
          className={styles.search}
          label={labels.search}
          placeholder={labels.searchPlaceholder}
          value={search}
          onValueChange={onSearch}
        />
      </div>
      {state === 'error' ? (
        <PlacesMessage
          message={labels.error}
          tone="alert"
          action={{ label: labels.retry, onClick: onRetry }}
        />
      ) : state === 'ready' && cards.length === 0 ? (
        <PlacesMessage message={labels.empty} />
      ) : (
        <ul className={styles.grid} aria-busy={state === 'loading' || undefined}>
          {state === 'loading'
            ? Array.from({ length: SKELETONS }, (_, index) => (
                <li key={index}>
                  <CommunityCard loading loadingLabel={labels.loading} />
                </li>
              ))
            : cards.map((card, index) => (
                <li key={card.id}>
                  <CommunityCard
                    name={card.name}
                    href={card.href}
                    image={card.image}
                    priority={index < 4}
                    ownerName={card.ownerName}
                    privacy={card.privacy}
                    membersLabel={card.membersLabel}
                    description={card.description}
                    linkComponent={linkComponent}
                  />
                </li>
              ))}
        </ul>
      )}
    </section>
  )
}

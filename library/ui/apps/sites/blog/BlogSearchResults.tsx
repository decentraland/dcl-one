import { useEffect, useRef, type MouseEvent } from 'react'
import { Button } from '../shared/Button/Button'
import { inAppLink, type BlogNavigate } from './blogLinks'
import type { BlogSearchHit } from './BlogSearch'
import styles from './BlogSearchResults.module.css'
import { HighlightText } from './SearchHighlight'

// The search page body, as source's SearchPage and SearchResultCard (pages/blog/SearchPage.tsx,
// components/blog/SearchResultCard): the query heading, the hits with their highlights, skeletons
// while loading, "Load More" while more exist, and "Nothing to show" for a query with no hits.

export type BlogSearchResultsProps = {
  /** The query, as typed. */
  q: string
  /** Searches run from 3 characters (source skips shorter queries). */
  searchable: boolean
  hits: readonly BlogSearchHit[]
  /** The first page is loading. */
  loading: boolean
  /** A next page is loading. */
  loadingMore: boolean
  hasMore: boolean
  onLoadMore: () => void
  onNavigate?: BlogNavigate
}

function ResultCard({ hit, onNavigate }: { hit: BlogSearchHit; onNavigate?: BlogNavigate }) {
  return (
    <div className={styles.card}>
      <a className={styles.cardLink} href={hit.url} {...inAppLink(hit.url, onNavigate)}>
        {hit.image ? (
          <img
            className={styles.cardImage}
            src={hit.image}
            alt=""
            loading="lazy"
            decoding="async"
          />
        ) : null}
        <div className={styles.cardText}>
          <p className={styles.cardTitle}>
            <HighlightText value={hit.highlightedTitle} />
          </p>
          <p className={styles.cardDescription}>
            <HighlightText value={hit.highlightedDescription} />
          </p>
        </div>
      </a>
    </div>
  )
}

function ResultSkeleton() {
  return (
    <div className={styles.card} aria-hidden="true">
      <div className={`${styles.bone} ${styles.boneImage}`} />
      <div className={styles.cardText}>
        <div className={`${styles.bone} ${styles.boneTitle}`} />
        <div className={`${styles.bone} ${styles.boneLine}`} />
        <div className={`${styles.bone} ${styles.boneLine} ${styles.boneLineShort}`} />
      </div>
    </div>
  )
}

export function BlogSearchResults({
  q,
  searchable,
  hits,
  loading,
  loadingMore,
  hasMore,
  onLoadMore,
  onNavigate
}: BlogSearchResultsProps) {
  // "Load More" leaves the page while the next hits load (as source), so keyboard focus would fall to
  // the document. When the button had focus, focus moves to the first new hit once it arrives.
  const list = useRef<HTMLDivElement>(null)
  const focusFrom = useRef<number | null>(null)
  useEffect(() => {
    const from = focusFrom.current
    if (from === null || hits.length <= from) return
    focusFrom.current = null
    list.current?.querySelectorAll<HTMLAnchorElement>(':scope > div > a')[from]?.focus()
  }, [hits.length])
  const loadMore = (event: MouseEvent<HTMLButtonElement>) => {
    focusFrom.current = document.activeElement === event.currentTarget ? hits.length : null
    onLoadMore()
  }
  const showLoading = loading && hits.length === 0
  const showEmpty = searchable && !loading && hits.length === 0
  return (
    <>
      {searchable ? (
        <div className={styles.header}>
          <p className={styles.subtitle}>
            Search results for <span className={styles.query}>&ldquo;{q}&rdquo;</span>
          </p>
        </div>
      ) : null}
      {showLoading ? (
        <div className={styles.results}>
          {Array.from({ length: 5 }, (_, index) => (
            <ResultSkeleton key={index} />
          ))}
        </div>
      ) : null}
      {hits.length > 0 ? (
        <div className={styles.results} ref={list}>
          {hits.map((hit, index) => (
            <ResultCard key={`${hit.url}-${index}`} hit={hit} onNavigate={onNavigate} />
          ))}
        </div>
      ) : null}
      {loadingMore ? (
        <div className={styles.results}>
          {Array.from({ length: 3 }, (_, index) => (
            <ResultSkeleton key={index} />
          ))}
        </div>
      ) : null}
      {hasMore && hits.length > 0 && !loadingMore ? (
        <div className={styles.more}>
          <Button onClick={loadMore}>Load More</Button>
        </div>
      ) : null}
      {showEmpty ? (
        <div className={styles.empty}>
          <h5 className={styles.emptyTitle}>Nothing to show</h5>
          <p className={styles.emptyText}>No results found for &ldquo;{q}&rdquo;</p>
        </div>
      ) : null}
    </>
  )
}

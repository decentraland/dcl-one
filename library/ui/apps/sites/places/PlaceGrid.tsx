import { useEffect, useRef } from 'react'
import { Button } from '../shared/Button/Button'
import { PlaceCardSlot } from './cards/PlaceCardSlot'
import type { PlaceCardData, PlaceCardHandlers, PlaceCardLabels } from './cards/types'
import styles from './PlaceGrid.module.css'

type PlaceGridProps = {
  cards: readonly PlaceCardData[]
  labels: PlaceCardLabels
  /** Placeholder cards appended while a page loads. */
  skeletons?: number
  /** Accessible name of the list. */
  label: string
} & PlaceCardHandlers

/** The Explore grid: 1 column on phones, 2 on tablets, 3 on desktops and 4 on wide screens. */
function PlaceGrid({ cards, labels, skeletons = 0, label, onOpen, onJumpIn }: PlaceGridProps) {
  return (
    <ul className={styles.grid} aria-label={label} aria-busy={skeletons > 0 || undefined}>
      {cards.map((card) => (
        <PlaceCardSlot
          key={card.id}
          card={card}
          labels={labels}
          onOpen={onOpen}
          onJumpIn={onJumpIn}
        />
      ))}
      {Array.from({ length: skeletons }, (_, index) => (
        <PlaceCardSlot key={`skeleton-${index}`} skeleton />
      ))}
    </ul>
  )
}

type PlacesMessageProps = {
  message: string
  /** `alert` for a failure, so it is announced; plain text otherwise. */
  tone?: 'info' | 'alert'
  action?: { label: string; onClick: () => void }
}

/** The empty, error and sign-in states: the reason, and the way out when there is one. */
function PlacesMessage({ message, tone = 'info', action }: PlacesMessageProps) {
  return (
    <div className={styles.message}>
      <p className={styles.messageText} role={tone === 'alert' ? 'alert' : undefined}>
        {message}
      </p>
      {action ? <Button onClick={action.onClick}>{action.label}</Button> : null}
    </div>
  )
}

type LoadMoreProps = {
  hasMore: boolean
  loading: boolean
  onLoadMore: () => void
  /** Read to assistive technology while the next page loads. */
  loadingLabel: string
  /** The button shown where the browser cannot watch the sentinel. */
  loadMoreLabel: string
}

/** Infinite scroll, as source: the next page loads when this sentinel nears the viewport. */
function LoadMore({ hasMore, loading, onLoadMore, loadingLabel, loadMoreLabel }: LoadMoreProps) {
  const sentinel = useRef<HTMLDivElement>(null)
  const latest = useRef(onLoadMore)
  useEffect(() => {
    latest.current = onLoadMore
  })
  const observable = typeof IntersectionObserver !== 'undefined'

  useEffect(() => {
    const node = sentinel.current
    if (!node || !hasMore || loading || !observable) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) latest.current()
      },
      { rootMargin: '600px 0px' }
    )
    observer.observe(node)
    return () => observer.disconnect()
  }, [hasMore, loading, observable])

  return (
    <div ref={sentinel} className={styles.sentinel}>
      {loading ? <span className={styles.spinner} aria-hidden="true" /> : null}
      <span role="status" className={styles.visuallyHidden}>
        {loading ? loadingLabel : ''}
      </span>
      {hasMore && !loading && !observable ? (
        <Button variant="secondary" onClick={onLoadMore}>
          {loadMoreLabel}
        </Button>
      ) : null}
    </div>
  )
}

export { LoadMore, PlaceGrid, PlacesMessage }
export type { LoadMoreProps, PlaceGridProps, PlacesMessageProps }

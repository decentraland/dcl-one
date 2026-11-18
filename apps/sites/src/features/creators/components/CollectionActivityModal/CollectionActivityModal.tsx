import { useEffect } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '@/features/creators/components/Button'
import { Modal } from '@/features/creators/components/Modal'
import { useCollectionEvents } from '@/features/creators/hooks/useCollectionEvents'
import { useCommittee } from '@/features/creators/hooks/useCommittee'
import { track } from '@/data/lib/creators/analytics'
import { type Collection } from '@/data/lib/creators/collections'
import { type ReviewParty } from '@/data/lib/creators/events'
import { type Item } from '@/data/lib/creators/items'
import { CommentComposer } from './CommentComposer'
import { EventRow } from './EventRow'
import styles from './CollectionActivityModal.module.css'

type Props = {
  collection: Collection
  address: string
  /** The collection's items, when the caller has them, so findings show item names. */
  items?: Item[]
  /** The side the viewer writes as: the collection's role holders as its creator, the committee as a curator. Without it they only read. */
  commentAs?: ReviewParty
  onClose: () => void
}

/** The collection's review timeline, newest first, and the box creators and curators write to each other from. */
export function CollectionActivityModal({ collection, address, items = [], commentAs, onClose }: Props) {
  const { t } = useTranslation()
  const { isCurator } = useCommittee(address)
  const query = useCollectionEvents(address, collection)
  const { isLoading, isAvailable, hasNextPage, isFetchingNextPage, isFetchNextPageError, fetchNextPage, refetch } =
    query
  const events = query.events ?? []
  // A failed "Load more" keeps what is loaded on screen; only a failed first page empties the modal.
  const isError = query.isError && !isFetchNextPageError

  useEffect(() => {
    track('Open activity', { collectionId: collection.id })
  }, [collection.id])

  const composer = commentAs && isAvailable && (
    <CommentComposer collection={collection} address={address} role={commentAs} />
  )

  return (
    <Modal title={t('activity_modal.title')} onClose={onClose} size="large" testId="activity-modal">
      <div className={styles.body}>
        {isLoading ? (
          <ol className={styles.timeline} aria-busy="true" data-testid="activity-loading">
            {Array.from({ length: 4 }, (_, i) => (
              <div key={i} className={`${styles.skeletonRow} skeleton`} />
            ))}
          </ol>
        ) : isError ? (
          <>
            <p className={styles.state} data-testid="activity-error">
              {t('activity_modal.error')}
            </p>
            <div className={styles.stateActions}>
              <Button type="button" variant="secondary" onClick={() => void refetch()}>
                {t('activity_modal.retry')}
              </Button>
            </div>
          </>
        ) : !isAvailable ? (
          <p className={styles.state} data-testid="activity-unavailable">
            {t('activity_modal.unavailable')}
          </p>
        ) : events.length === 0 ? (
          <>
            {composer}
            <p className={styles.state} data-testid="activity-empty">
              {t('activity_modal.empty')}
            </p>
          </>
        ) : (
          <>
            {composer}
            <ol className={styles.timeline} data-testid="activity-timeline">
              {events.map(event => (
                <EventRow key={event.id} event={event} items={items} isCurator={isCurator} />
              ))}
            </ol>
            {isFetchNextPageError && (
              <p className={styles.state} data-testid="activity-load-more-error">
                {t('activity_modal.error')}
              </p>
            )}
            {hasNextPage && (
              <div className={styles.more}>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  loading={isFetchingNextPage}
                  data-testid="activity-load-more"
                  onClick={() => void fetchNextPage()}
                >
                  {t(isFetchNextPageError ? 'activity_modal.retry' : 'activity_modal.load_more')}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </Modal>
  )
}

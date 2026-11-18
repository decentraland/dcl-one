import { useMemo } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { useRecentCollectionEvents } from '@/features/creators/hooks/useCollectionEvents'
import {
  CollectionDisplayStatus,
  canManageCollectionItems,
  getCollectionDisplayStatus,
  type Collection
} from '@/data/lib/creators/collections'
import { getAwaitedReply } from '@/data/lib/creators/events'
import { cx } from '@/data/lib/creators/cx'
import styles from './ReviewReplyBadge.module.css'

/** Collections whose review is still open: the only ones worth a timeline request per list row. */
const IN_REVIEW: CollectionDisplayStatus[] = [CollectionDisplayStatus.UNDER_REVIEW, CollectionDisplayStatus.REJECTED]

type Props = { collection: Collection; address: string | undefined; className?: string }

/** Marks a collection in the creator's lists when a curator's comment is waiting for an answer. */
export function ReviewReplyBadge({ collection, address, className }: Props) {
  const { t } = useTranslation()
  // Minters can't read the timeline (builder-server answers 401), so only the collection's managers ask.
  const inReview =
    IN_REVIEW.includes(getCollectionDisplayStatus(collection)) && canManageCollectionItems(collection, address)
  const recent = useRecentCollectionEvents(address, collection.id, inReview)
  const awaited = useMemo(() => !!recent.data && getAwaitedReply(recent.data) === 'creator', [recent.data])
  if (!awaited) return null
  return (
    <span className={cx(styles.badge, className)} data-testid="review-reply-badge">
      {t('review_flow.curator_replied')}
    </span>
  )
}

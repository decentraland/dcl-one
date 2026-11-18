import { useEffect, useState } from 'react'
import { useIntl } from '@/data/lib/creators/intl'
import { ArrowBackIcon, EditIcon, PersonAddAltIcon as AssignIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { AssignCuratorModal } from '../../AssignCuratorModal'
import { Button } from '../../Button'
import { CollectionStatusPill } from '../../CollectionStatusPill'
import { ConfirmModal } from '../../ConfirmModal'
import { ProfileBadge } from '../../ProfileBadge'
import { Tooltip } from '../../Tooltip'
import { useCollectionCuration } from '../../../hooks/useCuration'
import { useItemCurations } from '../../../hooks/useLinkedCollection'
import { usePendingLinkedItems, useRejectLinkedItems, useReviewedItems } from '../../../hooks/useLinkedReview'
import { getReviewThreshold, type ReviewedContent } from '@/data/lib/creators/approveLinkedCollection'
import { type Session } from '@/data/lib/creators/auth'
import { type Collection } from '@/data/lib/creators/collections'
import { curationListUrl } from '@/data/lib/creators/curation'
import { type Item } from '@/data/lib/creators/items'
import { useNotifications } from '@/data/lib/creators/notifications'
import { type ThirdParty } from '@/data/lib/creators/thirdParty'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { ForumVerdictModal } from './ForumVerdictModal'
import { LinkedApprovalFlowModal } from './LinkedApprovalFlowModal'
import { ThirdPartyStatusFlow } from './ThirdPartyStatusFlow'
import styles from './ReviewBar.module.css'
import { Link } from 'react-router'

type Props = {
  session: Session
  collection: Collection
  thirdParty: ThirdParty
  selectedId: string | null
  onSelectItem: (item: Item) => void
  /** The pending items opened so far, for the sidebar to tick. */
  onReviewedChange?: (ids: ReadonlySet<string>) => void
}

type Dialog = 'assign' | 'reject' | 'approve' | 'disable' | 'enable' | null

/**
 * The curator's toolbar over a linked collection: the items waiting for review, how many were opened against the
 * review threshold, and the committee's actions (approve, reject, enable or disable the third party).
 */
export function LinkedReviewBar({
  session,
  collection,
  thirdParty,
  selectedId,
  onSelectItem,
  onReviewedChange
}: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const address = session.address
  const showToast = useNotifications(state => state.showToast)
  const curationQuery = useCollectionCuration(address, collection)
  const curation = curationQuery.data ?? null
  const pending = usePendingLinkedItems(address, collection)
  const curations = useItemCurations(address, collection)
  const reject = useRejectLinkedItems(address, collection)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [verdict, setVerdict] = useState(false)
  // What the curator had seen when they clicked Approve: later refetches must not widen what gets signed.
  const [approvalReview, setApprovalReview] = useState<ReviewedContent | null>(null)
  // Likewise for Reject: items submitted after the dialog opened were never looked at.
  const [rejecting, setRejecting] = useState<Item[]>([])

  const pendingItems = pending.data ?? []
  const { reviewedCount, nextToReview, review } = useReviewedItems(pendingItems, selectedId)
  const reviewedKey = [...review.reviewedHashes.keys()].join(',')
  useEffect(() => {
    onReviewedChange?.(new Set(review.reviewedHashes.keys()))
    // Reported when the opened set changes, not on every new map with the same ids.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reviewedKey])
  const threshold = getReviewThreshold(pendingItems.length)
  const canApprove = pendingItems.length > 0 && reviewedCount >= threshold
  // Like the legacy builder, the third party's own switch only shows with nothing to review: approving pending items
  // enables it anyway. Enabling is only offered back to one approved before (it has a root), which legacy left to do.
  const canToggleThirdParty =
    thirdParty.published &&
    !pending.isLoading &&
    pendingItems.length === 0 &&
    (thirdParty.isApproved || !!thirdParty.root)
  const assignee = curation?.assignee ?? null
  const isLoading = pending.isLoading || curations.isLoading
  const [timeKey, time] = !curation
    ? ['created', collection.createdAt]
    : curation.status === 'pending'
      ? ['requested', curation.createdAt]
      : [curation.status, curation.updatedAt]

  const closeDialog = () => {
    setDialog(null)
    reject.reset()
  }

  function reviewNext() {
    const item = nextToReview()
    if (item) onSelectItem(item)
  }

  return (
    <div
      className={styles.reviewBar}
      role="toolbar"
      aria-label={t('item_editor.review.title')}
      data-testid="review-bar"
    >
      <div className={styles.identity}>
        <Link
          className={styles.backLink}
          to={curationListUrl()}
          aria-label={t('item_editor.review.back')}
          data-testid="review-back"
        >
          <ArrowBackIcon fontSize="small" />
        </Link>
        <h1 className={styles.name} title={collection.name}>
          {collection.name}
        </h1>
        <CollectionStatusPill collection={collection} />
      </div>

      <div className={styles.meta}>
        {!isLoading && (
          <span data-testid="review-linked-progress">
            {pendingItems.length === 0
              ? t('item_editor.linked_review.nothing_pending')
              : t('item_editor.linked_review.reviewed', { count: reviewedCount, threshold })}
          </span>
        )}
        {curation && (
          <span data-testid="review-requested">
            {t(`item_editor.review.${timeKey}`, { time: formatTimeAgo(time, intl.locale) })}
          </span>
        )}
        {assignee ? (
          <button
            className={styles.assigneeChip}
            type="button"
            aria-label={t('item_editor.review.change_assignee')}
            data-testid="review-assignee"
            onClick={() => setDialog('assign')}
          >
            <ProfileBadge address={assignee} self={assignee === address.toLowerCase()} />
            <EditIcon fontSize="inherit" />
          </button>
        ) : (
          curation && (
            <button
              className={styles.assigneeChip}
              type="button"
              data-testid="review-assign-me"
              onClick={() => setDialog('assign')}
            >
              <AssignIcon fontSize="small" />
              {t('item_editor.review.assign_to_me')}
            </button>
          )
        )}
      </div>

      <div className={styles.actions}>
        {pendingItems.length > reviewedCount && (
          <Button type="button" size="sm" variant="dark" data-testid="review-linked-next" onClick={reviewNext}>
            {t('item_editor.linked_review.review_next')}
          </Button>
        )}
        {pendingItems.length > 0 && (
          <>
            <Tooltip
              content={canApprove ? null : t('item_editor.linked_review.approve_blocked', { threshold })}
              asChild
              testId="review-linked-approve-tooltip"
            >
              <Button
                type="button"
                size="sm"
                variant="primary"
                aria-disabled={!canApprove || undefined}
                data-testid="review-action-approve"
                onClick={() => {
                  if (!canApprove) return
                  setApprovalReview(review)
                  setDialog('approve')
                }}
              >
                {t('item_editor.review.action.approve')}
              </Button>
            </Tooltip>
            <Button
              type="button"
              size="sm"
              variant="dark"
              data-testid="review-action-reject"
              onClick={() => {
                setRejecting(pendingItems)
                setDialog('reject')
              }}
            >
              {t('item_editor.review.action.reject')}
            </Button>
          </>
        )}
        {canToggleThirdParty &&
          (thirdParty.isApproved ? (
            <Button
              type="button"
              size="sm"
              variant="dark"
              data-testid="review-action-disable"
              onClick={() => setDialog('disable')}
            >
              {t('item_editor.linked_review.disable_third_party')}
            </Button>
          ) : (
            <Button
              type="button"
              size="sm"
              variant="primary"
              data-testid="review-action-enable"
              onClick={() => setDialog('enable')}
            >
              {t('item_editor.linked_review.enable_third_party')}
            </Button>
          ))}
      </div>

      {dialog === 'assign' && (
        <AssignCuratorModal
          collection={collection}
          curation={curation}
          address={address}
          mode={assignee ? 'edit' : 'self'}
          onClose={closeDialog}
        />
      )}
      {dialog === 'reject' && (
        <ConfirmModal
          title={t('item_editor.review.reject.title', { collection: collection.name })}
          description={t('item_editor.linked_review.reject_description', { count: rejecting.length })}
          error={reject.isError ? t('item_editor.review.reject.error') : null}
          busy={reject.isPending}
          onClose={closeDialog}
          cancel={{ label: t('item_editor.review.cancel'), onClick: closeDialog, testId: 'review-reject-cancel' }}
          confirm={{
            label: t('item_editor.review.reject.confirm'),
            testId: 'review-reject-confirm',
            onClick: () =>
              reject.mutate(
                { items: rejecting },
                {
                  onSuccess: () => {
                    showToast(t('item_editor.review.reject.success', { collection: collection.name }))
                    closeDialog()
                    if (collection.forumLink) setVerdict(true)
                  }
                }
              )
          }}
          testId="review-reject"
        />
      )}
      {(dialog === 'disable' || dialog === 'enable') && (
        <ThirdPartyStatusFlow
          session={session}
          thirdParty={thirdParty}
          approved={dialog === 'enable'}
          onClose={closeDialog}
          onDone={() => {
            if (dialog === 'disable' && collection.forumLink) setVerdict(true)
          }}
        />
      )}
      {verdict && collection.forumLink && (
        <ForumVerdictModal
          collection={{ ...collection, forumLink: collection.forumLink }}
          onClose={() => setVerdict(false)}
        />
      )}
      {dialog === 'approve' && approvalReview && (
        <LinkedApprovalFlowModal
          session={session}
          collection={collection}
          thirdParty={thirdParty}
          review={approvalReview}
          onClose={closeDialog}
        />
      )}
    </div>
  )
}

import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ChatBubbleOutlineIcon as CommentIcon,
  ErrorOutlineIcon as ErrorIcon,
  InfoOutlinedIcon as InfoIcon
} from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '../../Button'
import { CollectionActivityModal } from '../../CollectionActivityModal'
import { Tooltip } from '../../Tooltip'
import { useAppealCuration, useRequestValidation } from '../../../hooks/useCollectionEvents'
import { track } from '@/data/lib/creators/analytics'
import {
  ValidationLimitError,
  isAppealAlreadyOpen,
  isCollectionBusy,
  isReviewConflict,
  isValidationRunning
} from '@/data/lib/creators/builder'
import type { Collection } from '@/data/lib/creators/collections'
import {
  getReviewStage,
  hasReviewConversation,
  isRejectedStage,
  ReviewStage,
  type CollectionCuration
} from '@/data/lib/creators/curation'
import {
  countUnansweredComments,
  countValidationAttempts,
  getAutomatedIssuesByItem,
  getValidationAttemptsLeft,
  getLatestVerdict,
  hasOpenAppeal,
  isFinalValidationError,
  isUnsupportedItemsReview,
  isValidationInProgress,
  latestStageEvent,
  startOfUtcDay,
  type CollectionEvent
} from '@/data/lib/creators/events'
import { hasUpdatesSinceRejection } from '@/data/lib/creators/itemSync'
import type { Item } from '@/data/lib/creators/items'
import { useNotifications } from '@/data/lib/creators/notifications'
import {
  AdditionalReviewModal,
  AutomatedIssuesModal,
  DailyLimitModal,
  RequestCuratorReviewModal,
  ReviewingModal,
  SentForReviewModal
} from './ReviewModals'
import { markReviewModalSeen, wasReviewModalSeen } from '@/data/lib/creators/reviewModalMemory'
import type { ItemCheck } from '@/data/lib/creators/validation'
import styles from './ReviewFlow.module.css'

/** What the page asks for: a modal (right after publishing, or at the daily limit), another automated review, or a curator's. */
export type ReviewFlowRequest = 'reviewing' | 'limit' | 'validate' | 'appeal'

type ReviewModal =
  Exclude<ReviewFlowRequest, 'validate' | 'appeal'> | 'issues' | 'request' | 'sent' | 'additional' | 'activity'

type Props = {
  collection: Collection
  address: string
  curation: CollectionCuration | null
  /** `null` while the timeline is unavailable: no notice shows then, as without auto curation. */
  events: CollectionEvent[] | null
  items: Item[]
  /** Owner or collaborator: may review again and ask for a curator. */
  canManage: boolean
  request: ReviewFlowRequest | null
  onRequestHandled: () => void
}

/**
 * What the creator sees about a published collection's review: a one-line notice per stage, the modals the
 * review opens by itself (running, rejected, handed to a curator) and the ones the creator opens (ask a curator).
 */
export function ReviewFlow({
  collection,
  address,
  curation,
  events,
  items,
  canManage,
  request,
  onRequestHandled
}: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const requestValidation = useRequestValidation(address)
  const appeal = useAppealCuration(address)
  const [modal, setModal] = useState<ReviewModal | null>(null)

  const timeline = useMemo(() => events ?? [], [events])
  const stage = useMemo(() => getReviewStage(collection, curation, events), [collection, curation, events])
  const verdict = useMemo(() => getLatestVerdict(timeline), [timeline])
  const automatedResults = useMemo<ItemCheck[]>(() => {
    const byItem = getAutomatedIssuesByItem(timeline, items)
    return items.filter(item => byItem.has(item.id)).map(item => ({ item, issues: byItem.get(item.id)! }))
  }, [timeline, items])
  // The allowance resets at 00:00 UTC, so the count is redone when the day changes, not only when events do.
  const today = startOfUtcDay(Date.now())
  const attemptsUsed = useMemo(() => countValidationAttempts(timeline, today), [timeline, today])
  const running = useMemo(() => isValidationInProgress(timeline), [timeline])
  const appealOpen = useMemo(() => hasOpenAppeal(timeline), [timeline])
  const cause = useMemo(() => latestStageEvent(timeline), [timeline])
  const curatorComments = useMemo(() => countUnansweredComments(timeline, 'creator'), [timeline])

  const open = useCallback(
    (next: ReviewModal) => {
      setModal(next)
      track('bd_review_modal_opened', { collection_id: collection.id, stage, modal: next })
    },
    [collection.id, stage]
  )

  const { mutate: startValidation } = requestValidation
  const validateAgain = useCallback(() => {
    if (running) {
      showToast(t('review_flow.running'), { type: 'warn' })
      return
    }
    // Counted now, not from the last render: a tab left open past 00:00 UTC has its allowance back.
    if (getValidationAttemptsLeft(timeline) === 0) {
      open('limit')
      return
    }
    startValidation(
      { collection, events: timeline },
      {
        onSuccess: () => showToast(t('review_flow.validation_started')),
        onError: error => {
          if (error instanceof ValidationLimitError) open('limit')
          else if (isValidationRunning(error)) showToast(t('review_flow.running'), { type: 'warn' })
          else if (isReviewConflict(error)) showToast(t('review_flow.nothing_to_validate'), { type: 'warn' })
          else if (isCollectionBusy(error)) showToast(t('review_flow.busy'), { type: 'warn' })
          else showToast(t('review_flow.validation_error'), { type: 'error' })
        }
      }
    )
  }, [running, timeline, collection, startValidation, showToast, t, open])

  const requestReview = useCallback(() => {
    if (appealOpen) {
      showToast(t('review_flow.appeal_open'), { type: 'warn' })
      return
    }
    open('request')
  }, [appealOpen, showToast, t, open])

  useEffect(() => {
    if (!request) return
    if (request === 'validate') validateAgain()
    else if (request === 'appeal') requestReview()
    else open(request)
    onRequestHandled()
  }, [request, open, validateAgain, requestReview, onRequestHandled])

  // The verdict and the hand-off announce themselves once; the notice reopens the findings any time.
  useEffect(() => {
    if (modal || !canManage) return
    if (stage === ReviewStage.REJECTED_BY_VALIDATOR && verdict && automatedResults.length > 0) {
      const key = `verdict:${verdict.id}`
      if (wasReviewModalSeen(collection.id, key)) return
      markReviewModalSeen(collection.id, key)
      open('issues')
    } else if (stage === ReviewStage.NEEDS_CURATOR && cause) {
      const key = `handoff:${cause.id}`
      if (wasReviewModalSeen(collection.id, key)) return
      markReviewModalSeen(collection.id, key)
      open('additional')
    }
  }, [modal, canManage, stage, verdict, automatedResults.length, cause, collection.id, open])

  const close = () => {
    setModal(null)
    appeal.reset()
  }

  function confirmReview(note: string) {
    appeal.mutate({ collection, note: note || undefined }, { onSuccess: () => open('sent') })
  }

  /** From the limit modal: its copy already says what a curator review means, so it asks without a second dialog. */
  function requestReviewNow() {
    if (appealOpen) {
      showToast(t('review_flow.appeal_open'), { type: 'warn' })
      return
    }
    confirmReview('')
  }

  const appealError = isAppealAlreadyOpen(appeal.error)
    ? t('review_flow.appeal_open')
    : isCollectionBusy(appeal.error)
      ? t('review_flow.busy')
      : appeal.isError
        ? t('review_flow.appeal_error')
        : null
  // Only a rejection can be sent to a curator; the limit modal on submitted changes just says to wait.
  const canAppeal = canManage && isRejectedStage(stage)

  const modals = (
    <>
      {modal === 'reviewing' && <ReviewingModal attemptsUsed={Math.max(attemptsUsed, 1)} onDone={close} />}
      {modal === 'issues' && (
        <AutomatedIssuesModal
          results={automatedResults}
          onRequestReview={canManage ? requestReview : undefined}
          onClose={close}
        />
      )}
      {modal === 'request' && (
        <RequestCuratorReviewModal
          afterFixes={stage === ReviewStage.REJECTED_BY_CURATOR}
          busy={appeal.isPending}
          error={appealError}
          onConfirm={confirmReview}
          onClose={close}
        />
      )}
      {modal === 'sent' && <SentForReviewModal onDone={close} />}
      {modal === 'limit' && (
        <DailyLimitModal
          attemptsUsed={attemptsUsed}
          busy={appeal.isPending}
          error={appealError}
          onRequestReview={canAppeal ? requestReviewNow : undefined}
          onClose={close}
        />
      )}
      {modal === 'additional' && <AdditionalReviewModal onDone={close} />}
      {modal === 'activity' && (
        <CollectionActivityModal
          collection={collection}
          address={address}
          items={items}
          commentAs={canManage && hasReviewConversation(curation) ? 'creator' : undefined}
          onClose={close}
        />
      )}
    </>
  )

  if (!collection.isPublished) return modals

  let text: string | null = null
  let tone: 'info' | 'error' | undefined
  switch (stage) {
    case ReviewStage.AUTOMATED_REVIEWING:
      text = t('review_flow.notice.automated_reviewing')
      tone = 'info'
      break
    case ReviewStage.AWAITING_CURATOR:
      text = t('review_flow.notice.awaiting_curator')
      tone = 'info'
      break
    case ReviewStage.NEEDS_CURATOR:
      text = t(
        isUnsupportedItemsReview(cause) || isFinalValidationError(cause)
          ? 'review_flow.notice.needs_curator'
          : 'review_flow.notice.human_review'
      )
      tone = 'info'
      break
    case ReviewStage.APPEALED:
      text = t('review_flow.notice.appealed')
      tone = 'info'
      break
    case ReviewStage.REJECTED_BY_VALIDATOR:
      text = t('review_flow.notice.rejected_by_validator')
      tone = 'error'
      break
    case ReviewStage.REJECTED_BY_CURATOR:
      text = t('review_flow.notice.rejected_by_curator')
      tone = 'error'
      break
  }
  if (!text && curatorComments === 0) return modals

  // The findings modal carries the creator's next steps, so the notice only opens it.
  const hasIssues = stage === ReviewStage.REJECTED_BY_VALIDATOR && automatedResults.length > 0
  // A curator's reasons and message are read in the activity, and the next review is asked for from the page.
  const curatorFixes = stage === ReviewStage.REJECTED_BY_CURATOR
  // Reading the feedback comes first; once items were updated, asking for the review does.
  const fixesMade = curatorFixes && hasUpdatesSinceRejection(curation, items)
  // Only a curator's unanswered comments are flagged here; writing first is done from the activity.
  const canReply = canManage && events !== null && curatorComments > 0
  const commentsHint = t('review_flow.notice.curator_comments', { count: curatorComments })

  return (
    <>
      <div
        className={styles.notice}
        data-testid="review-notice"
        data-stage={stage ?? undefined}
        data-tone={tone ?? 'info'}
      >
        <p className={styles.noticeText}>
          {tone === 'error' ? <ErrorIcon fontSize="small" aria-hidden /> : <InfoIcon fontSize="small" aria-hidden />}
          <span>{text ?? commentsHint}</span>
        </p>
        {stage === ReviewStage.AUTOMATED_REVIEWING && (
          <span className={styles.noticeSpinner} data-testid="review-notice-spinner" aria-hidden />
        )}
        {(hasIssues || curatorFixes || (canManage && (stage === ReviewStage.REJECTED_BY_VALIDATOR || canReply))) && (
          <div className={styles.noticeActions}>
            {hasIssues && (
              <Button
                type="button"
                variant="primary"
                size="sm"
                data-testid="review-see-issues"
                onClick={() => open('issues')}
              >
                {t('review_flow.issues.see')}
              </Button>
            )}
            {canManage && !hasIssues && stage === ReviewStage.REJECTED_BY_VALIDATOR && (
              <Button
                type="button"
                variant="dark"
                size="sm"
                data-testid="request-curator-review"
                onClick={requestReview}
              >
                {t('review_flow.request_review')}
              </Button>
            )}
            {canReply && (
              <>
                <Tooltip content={commentsHint} describeChild asChild testId="review-comment-count-tooltip">
                  <span className={styles.commentCount} tabIndex={0} data-testid="review-comment-count">
                    <CommentIcon fontSize="small" aria-hidden />
                    {curatorComments}
                  </span>
                </Tooltip>
                {!curatorFixes && (
                  <Button
                    type="button"
                    variant="dark"
                    size="sm"
                    data-testid="review-reply"
                    onClick={() => open('activity')}
                  >
                    {t('review_flow.view_comments', { count: curatorComments })}
                  </Button>
                )}
              </>
            )}
            {curatorFixes && (
              <Button
                type="button"
                variant={fixesMade ? 'dark' : 'primary'}
                size="sm"
                data-testid="review-view-feedback"
                onClick={() => open('activity')}
              >
                {t('review_flow.issues.see')}
              </Button>
            )}
          </div>
        )}
      </div>
      {modals}
    </>
  )
}

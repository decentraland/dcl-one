import { useEffect, useMemo, useRef, useState } from 'react'
import { useIntl } from 'react-intl'
import {
  ArrowBackIcon,
  ChatBubbleOutlineIcon as ActivityIcon,
  EditIcon,
  PersonAddAltIcon as AssignIcon
} from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { AssignCuratorModal } from '@/features/creators/components/AssignCuratorModal'
import { Button } from '@/features/creators/components/Button'
import { CollectionActivityModal } from '@/features/creators/components/CollectionActivityModal'
import { ConfirmModal } from '@/features/creators/components/ConfirmModal'
import { CurationStatePill } from '@/features/creators/components/CurationStatePill'
import { ProfileBadge } from '@/features/creators/components/ProfileBadge'
import { AutomatedReviewIcon } from '@/features/creators/components/AutomatedReviewIcon'
import { Tooltip } from '@/features/creators/components/Tooltip'
import { ValidationBadge, toValidationSubject, type ItemResult } from '@/features/creators/components/ValidationBadge'
import { useCollectionEvents } from '@/features/creators/hooks/useCollectionEvents'
import { useCollectionValidation } from '@/features/creators/hooks/useCollectionValidation'
import { useCollectionCuration, useRejectCuration } from '@/features/creators/hooks/useCuration'
import { useFeatureFlag } from '@/features/creators/hooks/useFeatureFlag'
import { useItemSyncs } from '@/features/creators/hooks/useItemSync'
import { type ApprovalMode } from '@/features/creators/hooks/useApprovalFlow'
import { track } from '@/data/lib/creators/analytics'
import { type Session } from '@/data/lib/creators/auth'
import { type Collection } from '@/data/lib/creators/collections'
import {
  CurationState,
  ReviewAction,
  canEditAssignee,
  curationListUrl,
  ReviewStage,
  getCurationState,
  getCuratorState,
  getReviewActions,
  getReviewStage,
  hasReviewConversation
} from '@/data/lib/creators/curation'
import { getAwaitedReply, getCurrentVerdict } from '@/data/lib/creators/events'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { ItemSyncStatus, hasPendingChanges } from '@/data/lib/creators/itemSync'
import { type Item } from '@/data/lib/creators/items'
import { useNotifications } from '@/data/lib/creators/notifications'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { countIssues, getValidationStatus } from '@/data/lib/creators/validation'
import { AutomatedVerdictModal } from './AutomatedVerdictModal'
import { ApprovalFlowModal } from './ApprovalFlowModal'
import { DisableCollectionFlow } from './DisableCollectionFlow'
import { ForumVerdictModal } from './ForumVerdictModal'
import { RejectCurationModal } from './RejectCurationModal'
import styles from './ReviewBar.module.css'
import { Link } from 'react-router-dom'

type Props = {
  session: Session
  collection: Collection
  items: Item[]
  onSelectItem: (item: Item) => void
}

type Dialog = 'assign' | 'reject' | 'disable' | 'verdict' | 'activity' | null

const DECISIONS = [ReviewAction.APPROVE, ReviewAction.ENABLE, ReviewAction.REJECT]
// The constructive actions, as in the legacy panel; Reject and Disable stay dark.
const PRIMARY = new Set([ReviewAction.APPROVE, ReviewAction.ENABLE, ReviewAction.DEPLOY_MISSING])
// Only a pending request has content waiting on the committee's decision.
const VALIDATED_STATES = [CurationState.TO_REVIEW]
const NO_ITEMS: Item[] = []

/** The curator's toolbar over the read-only editor: the collection's review state and the committee's actions. */
export function ReviewBar({ session, collection, items, onSelectItem }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const address = session.address
  const showToast = useNotifications(state => state.showToast)
  const curationQuery = useCollectionCuration(address, collection)
  const curation = curationQuery.data ?? null
  const syncs = useItemSyncs(address, collection, items)
  const eventsQuery = useCollectionEvents(address, collection)
  const { events } = eventsQuery
  const autoCurationFlag = useFeatureFlag(FeatureFlag.AUTO_CURATION_REVIEW_FLOW)
  const autoCuration = autoCurationFlag.enabled
  // Only a server that answered without a timeline (a 404) refuses the rejection fields: anywhere else the
  // reason form is the rejection, and it waits for the flag and the timeline's first answer.
  const timelineMissing = eventsQuery.isSuccess && !eventsQuery.isAvailable
  const withReasons = autoCuration && !timelineMissing
  const reasonsPending = autoCurationFlag.isLoading || (withReasons && eventsQuery.isPending)
  const reject = useRejectCuration(address)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [approval, setApproval] = useState<ApprovalMode | null>(null)
  // The curator about to decide is assigned first, so the request always names who curated the collection.
  const [takeOver, setTakeOver] = useState<ReviewAction | null>(null)
  const [forumVerdict, setForumVerdict] = useState(false)
  const self = address.toLowerCase()

  const hasMissingEntities = useMemo(
    () => [...syncs.values()].some(sync => sync.status === ItemSyncStatus.UNSYNCED && !sync.entity),
    [syncs]
  )
  const state = getCurationState(collection, curation)
  const stage = useMemo(() => getReviewStage(collection, curation, events), [collection, curation, events])
  const curatorState = getCuratorState(collection, curation, stage)
  const automatedVerdict = useMemo(() => (events ? getCurrentVerdict(events, curation) : null), [events, curation])
  const automatedRunning = stage === ReviewStage.AUTOMATED_REVIEWING
  const automatedStatus = automatedRunning ? 'running' : automatedVerdict?.outcome
  const creatorReplied = useMemo(() => !!events && getAwaitedReply(events) === 'curator', [events])
  const actions = collection.isPublished ? getReviewActions(collection, curation, hasMissingEntities) : []
  const assignee = curation?.assignee ?? null
  // A first-review rejection opens the request itself, so its creation time only means something while pending.
  const [timeKey, time] = !curation
    ? ['created', collection.createdAt]
    : curation.status === 'pending'
      ? ['requested', curation.createdAt]
      : [curation.status, curation.updatedAt]
  const canAssign = canEditAssignee(collection, curation)
  const isLoading = collection.isPublished && curationQuery.isLoading
  // Without the request, Reject and Assign would open a new one instead of updating it.
  const isCurationError = collection.isPublished && curationQuery.isError && !curationQuery.data

  const showActions = !isLoading && !isCurationError && actions.length > 0

  const validates = collection.isPublished && !isLoading && !isCurationError && VALIDATED_STATES.includes(state)
  // An item the Catalyst couldn't compare is checked anyway rather than left out.
  const validationItems = useMemo(
    () =>
      validates
        ? items.filter(item => {
            const sync = syncs.get(item.id)
            return hasPendingChanges(sync?.status) || !!sync?.lookupFailed
          })
        : NO_ITEMS,
    [validates, items, syncs]
  )
  const validation = useCollectionValidation(validationItems, validates)
  const validationResults = useMemo<ItemResult[]>(
    () =>
      validationItems.flatMap(item => {
        const issues = validation.results.get(item.id)?.issues ?? []
        if (issues.length === 0) return []
        return [
          {
            id: item.id,
            subject: toValidationSubject(item),
            issues,
            onSelect: () => onSelectItem(item)
          }
        ]
      }),
    [validationItems, validation.results, onSelectItem]
  )
  const validationIssues = useMemo(() => validationResults.flatMap(result => result.issues), [validationResults])
  // Approved items read as loading until the Catalyst answers: which of them changed isn't known yet.
  const isResolvingItems = useMemo(
    () =>
      validates &&
      items.some(item => {
        const sync = syncs.get(item.id)
        return sync?.status === ItemSyncStatus.LOADING && !sync.lookupFailed
      }),
    [validates, items, syncs]
  )
  const validationStatus = getValidationStatus(
    validationItems.length > 0 || isResolvingItems ? validationIssues : undefined,
    validation.isValidating || isResolvingItems
  )
  // One result per collection opened (the bar is keyed by collection), cached checks included.
  const startedAt = useRef(Date.now())
  const reported = useRef(false)
  useEffect(() => {
    if (validationStatus === 'loading' || validationStatus === 'idle' || reported.current) return
    reported.current = true
    track('Review Validation Result', {
      collectionId: collection.id,
      itemCount: validationItems.length,
      ...countIssues(validationIssues),
      itemsWithIssues: validationResults.length,
      durationMs: Date.now() - startedAt.current
    })
  }, [validationStatus, collection.id, validationItems.length, validationIssues, validationResults.length])

  function onAction(action: ReviewAction) {
    // Enable never opens a request (it only switches the collection back on): with none, there is nothing to take
    // over; with one, taking it over only records who enabled it.
    const nothingToTake = action === ReviewAction.ENABLE && !curation
    if (DECISIONS.includes(action) && curation?.assignee !== self && !nothingToTake) return setTakeOver(action)
    runAction(action)
  }

  function runAction(action: ReviewAction) {
    if (action === ReviewAction.APPROVE) setApproval('approve')
    else if (action === ReviewAction.ENABLE) setApproval('enable')
    else if (action === ReviewAction.DEPLOY_MISSING) setApproval('deploy_missing')
    else if (action === ReviewAction.REJECT) setDialog('reject')
    else setDialog('disable')
  }

  const closeDialog = () => {
    setDialog(null)
    reject.reset()
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
        {!isLoading && !isCurationError && <CurationStatePill state={curatorState} />}
        <ValidationBadge
          status={validationStatus}
          results={validationResults}
          scope="collection"
          onOpen={() => track('Review Validation Opened', { collectionId: collection.id, status: validationStatus })}
          testId="review-validation"
        />
      </div>

      <div className={styles.meta}>
        {!collection.isPublished ? (
          <span data-testid="review-unpublished">{t('item_editor.review.unpublished')}</span>
        ) : isLoading ? null : isCurationError ? (
          <>
            <span data-testid="review-curation-error">{t('item_editor.review.curation_error')}</span>
            <Button
              type="button"
              size="sm"
              variant="dark"
              data-testid="review-curation-retry"
              onClick={() => void curationQuery.refetch()}
            >
              {t('item_editor.review.retry')}
            </Button>
          </>
        ) : (
          <>
            <span data-testid="review-requested">
              {t(`item_editor.review.${timeKey}`, { time: formatTimeAgo(time, intl.locale) })}
            </span>
            {automatedStatus && (
              <Tooltip
                content={t(`automated_verdict.button_${automatedStatus}`)}
                describeChild
                asChild
                testId="review-automated-verdict-tooltip"
              >
                <button
                  className={styles.assigneeChip}
                  type="button"
                  aria-disabled={automatedRunning || undefined}
                  data-status={automatedStatus}
                  data-testid="review-automated-verdict"
                  onClick={() => !automatedRunning && setDialog('verdict')}
                >
                  <AutomatedReviewIcon status={automatedStatus} />
                  {t('automated_verdict.button')}
                </button>
              </Tooltip>
            )}
            {events && (
              <button
                className={styles.assigneeChip}
                type="button"
                data-awaited={creatorReplied ? '' : undefined}
                data-testid="review-activity"
                onClick={() => setDialog('activity')}
              >
                <ActivityIcon fontSize="small" />
                {t(creatorReplied ? 'item_editor.review.creator_replied' : 'item_editor.review.activity')}
              </button>
            )}
            {assignee ? (
              <button
                className={styles.assigneeChip}
                type="button"
                disabled={!canAssign}
                aria-label={t('item_editor.review.change_assignee')}
                data-testid="review-assignee"
                onClick={() => setDialog('assign')}
              >
                <ProfileBadge address={assignee} self={assignee === address.toLowerCase()} />
                {canAssign && <EditIcon fontSize="inherit" />}
              </button>
            ) : (
              canAssign && (
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
          </>
        )}
      </div>

      {showActions && (
        <div className={styles.actions}>
          {actions.map(action => (
            <Button
              key={action}
              type="button"
              size="sm"
              variant={PRIMARY.has(action) ? 'primary' : 'dark'}
              data-testid={`review-action-${action}`}
              disabled={action === ReviewAction.REJECT && reasonsPending}
              onClick={() => onAction(action)}
            >
              {t(`item_editor.review.action.${action}`)}
            </Button>
          ))}
        </div>
      )}

      {dialog === 'assign' && (
        <AssignCuratorModal
          collection={collection}
          curation={curation}
          address={address}
          mode={assignee ? 'edit' : 'self'}
          onClose={closeDialog}
        />
      )}
      {dialog === 'reject' && withReasons && (
        <RejectCurationModal collection={collection} curation={curation} address={address} onClose={closeDialog} />
      )}
      {dialog === 'reject' && !withReasons && (
        <ConfirmModal
          title={t('item_editor.review.reject.title', { collection: collection.name })}
          description={t(
            collection.isApproved ? 'item_editor.review.reject.changes' : 'item_editor.review.reject.first'
          )}
          error={reject.isError ? t('item_editor.review.reject.error') : null}
          busy={reject.isPending}
          onClose={closeDialog}
          cancel={{ label: t('item_editor.review.cancel'), onClick: closeDialog, testId: 'review-reject-cancel' }}
          confirm={{
            label: t('item_editor.review.reject.confirm'),
            testId: 'review-reject-confirm',
            onClick: () =>
              reject.mutate(
                { collection, curation },
                {
                  onSuccess: () => {
                    showToast(t('item_editor.review.reject.success', { collection: collection.name }))
                    closeDialog()
                    if (collection.forumLink) setForumVerdict(true)
                  }
                }
              )
          }}
          testId="review-reject"
        />
      )}
      {takeOver && (
        <AssignCuratorModal
          collection={collection}
          curation={curation}
          address={address}
          mode="self"
          onClose={() => setTakeOver(null)}
          onAssigned={() => {
            setTakeOver(null)
            runAction(takeOver)
          }}
        />
      )}
      {dialog === 'disable' && (
        <DisableCollectionFlow
          session={session}
          collection={collection}
          onClose={closeDialog}
          onDisabled={() => {
            // With the timeline the creator reads the decision in the app; the forum is no longer where it goes.
            if (collection.forumLink && !withReasons) setForumVerdict(true)
          }}
        />
      )}
      {forumVerdict && collection.forumLink && (
        <ForumVerdictModal
          collection={{ ...collection, forumLink: collection.forumLink }}
          onClose={() => setForumVerdict(false)}
        />
      )}
      {dialog === 'activity' && (
        <CollectionActivityModal
          collection={collection}
          address={address}
          items={items}
          commentAs={hasReviewConversation(curation) ? 'curator' : undefined}
          onClose={closeDialog}
        />
      )}
      {dialog === 'verdict' && automatedVerdict && (
        <AutomatedVerdictModal
          verdict={automatedVerdict}
          requestedByCreator={stage === ReviewStage.APPEALED}
          items={items}
          onClose={closeDialog}
        />
      )}
      {approval && (
        <ApprovalFlowModal
          session={session}
          collection={collection}
          curation={curation}
          mode={approval}
          items={items}
          withNote={withReasons}
          onClose={() => setApproval(null)}
        />
      )}
    </div>
  )
}

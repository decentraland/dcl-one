import { useMemo, useState } from 'react'
import { useIntl } from '@/data/lib/creators/intl'
import { useTranslation } from '@/data/lib/creators/intl'
import { ItemFindingsList } from '../FindingsList'
import { AutomatedReviewIcon } from '../AutomatedReviewIcon'
import { ProfileBadge } from '../ProfileBadge'
import {
  COLLECTION_EVENT_TYPES,
  countFailedItems,
  getFailedItems,
  isRejectReasonCode,
  isUnsupportedItemsReview,
  isValidationTrigger,
  type CollectionEvent,
  isFinalValidationError
} from '@/data/lib/creators/events'
import { shortenAddress } from '@/data/lib/creators/address'
import type { Item } from '@/data/lib/creators/items'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { useProfile } from '../../hooks/useProfile'
import styles from './CollectionActivityModal.module.css'

type Props = {
  event: CollectionEvent
  items: Item[]
  /** Committee members read the validator's own failure text. */
  isCurator: boolean
}

/** Types whose line reads as "{actor} did X"; the rest are system sentences without a subject. */
const ACTOR_LED = new Set([
  'collection.published',
  'review.appeal_requested',
  'review.assigned',
  'review.approved',
  'review.rejected',
  'changes.submitted',
  'review.comment',
  'collection.disabled'
])

const OUTCOMES: Record<string, 'passed' | 'rejected' | undefined> = {
  'review.automated_passed': 'passed',
  'review.approved': 'passed',
  'review.automated_rejected': 'rejected',
  'review.rejected': 'rejected'
}

function AssigneeName({ address }: { address: string }) {
  const { data: profile } = useProfile(address)
  return <>{profile?.name || shortenAddress(address)}</>
}

export function EventRow({ event, items, isCurator }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  const [showFindings, setShowFindings] = useState(false)
  const { payload } = event
  const failedItems = useMemo(
    () => (Array.isArray(payload.items) ? getFailedItems({ verdict: 'rejected', items: payload.items }) : []),
    [payload.items]
  )
  const reasons = useMemo(() => (payload.rejectionReasons ?? []).filter(isRejectReasonCode), [payload.rejectionReasons])
  const known = (COLLECTION_EVENT_TYPES as readonly string[]).includes(event.type)

  function line() {
    switch (event.type) {
      case 'review.automated_started':
        return t('activity_modal.event.review.automated_started', {
          trigger: t(`activity_modal.trigger.${isValidationTrigger(payload.trigger) ? payload.trigger : 'publish'}`)
        })
      case 'review.automated_rejected':
        return t('activity_modal.event.review.automated_rejected', {
          count: countFailedItems(Array.isArray(payload.items) ? payload.items : [])
        })
      case 'review.automated_error':
        if (isFinalValidationError(event)) return t('activity_modal.event.review.automated_error_final')
        return t(
          isCurator
            ? 'activity_modal.event.review.automated_error_curator'
            : 'activity_modal.event.review.automated_error'
        )
      case 'review.human_required':
        if (isUnsupportedItemsReview(event)) return t('activity_modal.event.review.human_required_unsupported')
        if (isFinalValidationError(event)) return t('activity_modal.event.review.automated_error_final')
        return t('activity_modal.event.review.human_required')
      case 'review.assigned':
        return payload.assignee
          ? intl.formatMessage(
              { id: 'activity_modal.event.review.assigned' },
              { assignee: <AssigneeName key="assignee" address={payload.assignee} /> }
            )
          : t('activity_modal.event.review.unassigned')
      case 'changes.submitted':
        return t('activity_modal.event.changes.submitted', { count: payload.itemIds?.length ?? 0 })
      default:
        return known ? t(`activity_modal.event.${event.type}`) : t('activity_modal.event.unknown', { type: event.type })
    }
  }

  // Every decision on the collection, automated or a curator's, leads with the same pass/fail mark.
  const outcome = OUTCOMES[event.type]
  const actor =
    event.actorAddress && (event.actor === 'creator' || event.actor === 'curator') ? (
      <ProfileBadge address={event.actorAddress} testId="activity-actor" />
    ) : (
      <span className={styles.actor} data-testid="activity-actor">
        {t('activity_modal.actor.system')}
      </span>
    )
  const quote = payload.rejectionMessage ?? payload.message ?? payload.note
  const aboutItem = useMemo(
    () => (payload.itemId ? items.find(item => item.id === payload.itemId) : undefined),
    [payload.itemId, items]
  )

  return (
    <li className={styles.row} data-testid="activity-event" data-type={event.type}>
      <div className={styles.line}>
        {outcome && <AutomatedReviewIcon status={outcome} />}
        {ACTOR_LED.has(event.type) && actor}
        <span data-testid="activity-line">{line()}</span>
      </div>
      <time
        className={styles.time}
        dateTime={new Date(event.createdAt).toISOString()}
        title={intl.formatDate(event.createdAt, { dateStyle: 'medium', timeStyle: 'short' })}
      >
        {formatTimeAgo(event.createdAt, intl.locale)}
      </time>
      {(reasons.length > 0 || quote || failedItems.length > 0) && (
        <div className={styles.details}>
          {aboutItem && (
            <span className={styles.itemTag} data-testid="activity-item">
              {aboutItem.name}
            </span>
          )}
          {reasons.length > 0 && (
            <ul className={styles.reasons} data-testid="activity-reasons">
              {reasons.map(code => (
                <li className={styles.reason} key={code}>
                  {t(`reject_reason.${code}`)}
                </li>
              ))}
            </ul>
          )}
          {quote && (
            <blockquote className={styles.quote} data-testid="activity-quote">
              {quote}
            </blockquote>
          )}
          {failedItems.length > 0 && (
            <>
              <button
                className={styles.toggle}
                type="button"
                aria-expanded={showFindings}
                data-testid="activity-toggle-findings"
                onClick={() => setShowFindings(open => !open)}
              >
                {t(showFindings ? 'activity_modal.hide_findings' : 'activity_modal.show_findings')}
              </button>
              {showFindings && (
                <ItemFindingsList
                  results={failedItems}
                  items={items}
                  showErrors={isCurator}
                  testId="activity-findings"
                />
              )}
            </>
          )}
        </div>
      )}
    </li>
  )
}

import { useMemo } from 'react'
import { useIntl } from '@/data/lib/creators/intl'
import { Link } from 'react-router'
import { EditIcon } from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { CollectionMosaic } from '@/features/creators/components/CollectionMosaic'
import { CurationStatePill } from '@/features/creators/components/CurationStatePill'
import { ReviewTypePill } from '../ReviewTypePill'
import { ProfileBadge } from '@/features/creators/components/ProfileBadge'
import { AutomatedReviewIcon } from '@/features/creators/components/AutomatedReviewIcon'
import { Tooltip } from '@/features/creators/components/Tooltip'
import { useRecentCollectionEvents } from '@/features/creators/hooks/useCollectionEvents'
import { getAwaitedReply, getCurrentVerdict, type CollectionEvent } from '@/data/lib/creators/events'
import { isLinkedCollection, type Collection } from '@/data/lib/creators/collections'
import { openForumPost } from '@/data/lib/creators/forumPost'
import {
  canEditAssignee,
  getReviewStage,
  getReviewType,
  type CollectionCuration,
  getCuratorState,
  ReviewStage,
  VALIDATOR_REVIEWER
} from '@/data/lib/creators/curation'
import { formatTimeAgo } from '@/data/lib/creators/time'
import { cx } from '@/data/lib/creators/cx'
import styles from './CurationRow.module.css'

type Props = {
  collection: Collection
  curation: CollectionCuration | null
  address: string
  onAssign: (collection: Collection, curation: CollectionCuration | null, mode: 'self' | 'edit') => void
}

const NO_EVENTS: CollectionEvent[] = []

function reviewUrl(collection: Collection): string {
  return `/collections/editor?collection=${collection.id}&reviewing=true`
}

export function CurationRow({ collection, curation, address, onAssign }: Props) {
  const { t } = useTranslation()
  const intl = useIntl()
  // Only a pending request needs the timeline to tell its step. A decided one carries its reviewer, which
  // only a server with the timeline writes, so its stage needs no request per row.
  const pending = curation?.status === 'pending'
  // A curator's request for fixes is the other case with a conversation: the creator may answer it in writing.
  const fixesRequested =
    curation?.status === 'rejected' && !!curation.reviewedBy && curation.reviewedBy !== VALIDATOR_REVIEWER
  const recent = useRecentCollectionEvents(address, collection.id, pending || fixesRequested)
  const events = pending ? (recent.data ?? null) : recent.flagEnabled && curation?.reviewedBy ? NO_EVENTS : null
  const stage = useMemo(() => getReviewStage(collection, curation, events), [collection, curation, events])
  const curatorState = getCuratorState(collection, curation, stage)
  // Only the current request's result says why it is in the queue; an older run belongs to another request.
  const verdict = useMemo(() => (events ? getCurrentVerdict(events, curation) : null), [events, curation])
  const automatedStatus =
    stage === ReviewStage.REJECTED_BY_VALIDATOR ? 'rejected' : pending && verdict ? verdict.outcome : null
  const creatorReplied = useMemo(() => !!recent.data && getAwaitedReply(recent.data) === 'curator', [recent.data])
  const assignee = curation?.assignee ?? null
  const canAssign = canEditAssignee(collection, curation)

  return (
    <article
      className={styles.row}
      data-testid="curation-row"
      data-state={curatorState}
      data-stage={stage ?? undefined}
    >
      <div className={styles.nameCell}>
        <div className={styles.thumb}>
          <CollectionMosaic collectionId={collection.id} itemCount={collection.itemCount} />
        </div>
        <Link className={styles.rowLink} to={reviewUrl(collection)} data-testid="curation-row-link">
          <h3 className={styles.name} title={collection.name}>
            {collection.name}
          </h3>
          <span className={styles.sub}>
            {t('collections_page.item_count', { count: collection.itemCount })}
            {isLinkedCollection(collection) && ` · ${t('collection_status.linked')}`}
          </span>
        </Link>
      </div>
      <div className={styles.cell} data-cell="owner">
        {collection.owner ? (
          <ProfileBadge address={collection.owner} testId="curation-row-owner" />
        ) : (
          // Linked collections belong to a third party, not a wallet.
          <span className={styles.unassigned} data-testid="curation-row-owner">
            {t('curation_page.list.third_party')}
          </span>
        )}
      </div>
      <div className={styles.cell} data-cell="requested" data-testid="curation-row-requested">
        <span className={styles.cellLabel}>
          {t(curation ? 'curation_page.list.requested' : 'curation_page.list.created')}
        </span>
        {formatTimeAgo(curation?.createdAt ?? collection.createdAt, intl.locale)}
      </div>
      <div className={styles.pills}>
        <div className={styles.cell} data-cell="type">
          <ReviewTypePill type={getReviewType(collection)} />
        </div>
        <div className={styles.cell} data-cell="state">
          <span className={styles.stateWithReview}>
            <CurationStatePill state={curatorState} />
            {automatedStatus && (
              <Tooltip
                content={[
                  t(`automated_verdict.status_${automatedStatus}`),
                  stage === ReviewStage.APPEALED && t('automated_verdict.reason_requested')
                ]
                  .filter(Boolean)
                  .join('\n')}
                testId="curation-row-automated"
              >
                <AutomatedReviewIcon status={automatedStatus} />
              </Tooltip>
            )}
          </span>
        </div>
      </div>
      <div className={styles.discussionCell} data-testid="curation-row-discussion">
        <span className={styles.cellLabel} data-mobile-only>
          {t('curation_page.list.discussion')}
        </span>
        {creatorReplied && (
          <span className={styles.replied} data-testid="curation-row-replied">
            {t('curation_page.list.creator_replied')}
          </span>
        )}
        {collection.forumLink ? (
          <button
            className={cx(styles.textAction, styles.forumLink)}
            type="button"
            data-testid="curation-row-forum-link"
            onClick={() => openForumPost(collection, 'curation_list')}
          >
            {t('curation_page.list.forum_post')}
          </button>
        ) : (
          <span className={styles.unassigned}>{t('curation_page.list.not_posted')}</span>
        )}
      </div>
      <div
        className={styles.assigneeCell}
        data-testid="curation-row-assignee"
        data-unassigned={assignee ? undefined : ''}
      >
        {assignee ? (
          <>
            <ProfileBadge address={assignee} self={assignee === address.toLowerCase()} testId="curation-row-curator" />
            {canAssign && (
              <button
                className={styles.iconAction}
                type="button"
                aria-label={t('curation_page.list.edit_assignee')}
                data-testid="curation-row-edit-assignee"
                onClick={() => onAssign(collection, curation, 'edit')}
              >
                <EditIcon fontSize="small" />
              </button>
            )}
          </>
        ) : (
          <>
            <span className={styles.unassigned}>{t('curation_page.list.unassigned')}</span>
            {canAssign && (
              <button
                className={styles.textAction}
                type="button"
                data-testid="curation-row-assign-me"
                onClick={() => onAssign(collection, curation, 'self')}
              >
                {t('curation_page.list.assign_to_me')}
              </button>
            )}
          </>
        )}
      </div>
    </article>
  )
}

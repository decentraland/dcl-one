import { useMemo, useState } from 'react'
import { InfoOutlinedIcon as InfoIcon, ReportProblemOutlinedIcon as ErrorIcon } from '../../Icons'
import reviewArt from '../../../assets/publish-success.png'
import limitArt from '../../../assets/review-limit.png'
import { Button } from '../../Button'
import { ConfirmModal } from '../../ConfirmModal'
import { Modal } from '../../Modal'
import { NoteField } from '../../NoteField'
import { SuccessModal } from '../../SuccessModal'
import { Tooltip } from '../../Tooltip'
import { ItemValidationCard, toValidationSubject } from '../../ValidationBadge'
import { useTranslation } from '@/data/lib/creators/intl'
import { NOTE_MAX_LENGTH, VALIDATION_ATTEMPTS_PER_DAY, getValidationResetIn } from '@/data/lib/creators/events'
import issues from '../PublishCollectionModal/ValidationIssuesView.module.css'
import { type ItemCheck } from '@/data/lib/creators/validation'
import styles from './ReviewFlow.module.css'

type AttemptsProps = { used: number; testId: string }

/** "Automated reviews: N of 5 used today", under the copy of the review modals, with how the limit works. */
function Attempts({ used, testId }: AttemptsProps) {
  const { t } = useTranslation()
  return (
    <div className={styles.attempts}>
      <span data-testid={testId}>
        {t('review_flow.attempts', {
          used: Math.min(used, VALIDATION_ATTEMPTS_PER_DAY),
          max: VALIDATION_ATTEMPTS_PER_DAY
        })}
      </span>
      <Tooltip
        content={t('review_flow.attempts_tooltip', { max: VALIDATION_ATTEMPTS_PER_DAY })}
        testId={`${testId}-tooltip`}
      >
        <InfoIcon fontSize="inherit" aria-label={t('review_flow.attempts_info')} />
      </Tooltip>
    </div>
  )
}

/** Right after publishing: the automated review is running, the creator can close and wait. */
export function ReviewingModal({ attemptsUsed, onDone }: { attemptsUsed: number; onDone: () => void }) {
  const { t } = useTranslation()
  return (
    <SuccessModal
      title={t('review_flow.reviewing.title')}
      description={t('review_flow.reviewing.description')}
      note={<Attempts used={attemptsUsed} testId="reviewing-attempts" />}
      art={reviewArt}
      onDone={onDone}
      testId="reviewing-modal"
    />
  )
}

/** The validator could not decide: a curator takes over, no action needed. */
export function AdditionalReviewModal({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  return (
    <SuccessModal
      title={t('review_flow.additional.title')}
      description={t('review_flow.additional.description')}
      art={reviewArt}
      onDone={onDone}
      testId="additional-review-modal"
    />
  )
}

/** The appeal went through: a curator will look at the collection. */
export function SentForReviewModal({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation()
  return (
    <SuccessModal
      title={t('review_flow.sent.title')}
      description={t('review_flow.sent.description')}
      art={reviewArt}
      onDone={onDone}
      testId="sent-for-review-modal"
    />
  )
}

type RequestProps = {
  /** After a curator's request for fixes: the creator made them and asks for the next review. */
  afterFixes?: boolean
  busy: boolean
  error: string | null
  onConfirm: (note: string) => void
  onClose: () => void
}

/** Confirms the appeal before sending the rejected collection to a curator. */
export function RequestCuratorReviewModal({ afterFixes, busy, error, onConfirm, onClose }: RequestProps) {
  const { t } = useTranslation()
  const [note, setNote] = useState('')
  return (
    <ConfirmModal
      icon={<ErrorIcon />}
      title={t(afterFixes ? 'collection_detail_page.request_review.title' : 'review_flow.request.title')}
      description={t(
        afterFixes ? 'collection_detail_page.request_review.description' : 'review_flow.request.description'
      )}
      error={error}
      busy={busy}
      onClose={onClose}
      cancel={{ label: t('review_flow.request.cancel'), onClick: onClose, testId: 'request-review-cancel' }}
      confirm={{
        label: t('review_flow.request.confirm'),
        onClick: () => onConfirm(note.trim()),
        testId: 'request-review-confirm'
      }}
      testId="request-review-modal"
    >
      <div className={styles.requestNote}>
        <NoteField
          label={t(afterFixes ? 'collection_detail_page.push_note.label' : 'review_flow.request.note')}
          value={note}
          onChange={setNote}
          maxLength={NOTE_MAX_LENGTH}
          placeholder={t(
            afterFixes ? 'collection_detail_page.push_note.placeholder' : 'review_flow.request.note_placeholder'
          )}
          disabled={busy}
          testId="request-review-note"
        />
      </div>
    </ConfirmModal>
  )
}

type LimitProps = {
  attemptsUsed: number
  busy?: boolean
  error?: string | null
  /** Only when the collection was rejected: a curator can't review submitted changes ahead of the queue. */
  onRequestReview?: () => void
  onClose: () => void
}

/** The day's automated reviews are spent: wait, or ask a curator instead. */
export function DailyLimitModal({ attemptsUsed, busy, error, onRequestReview, onClose }: LimitProps) {
  const { t } = useTranslation()
  const reset = useMemo(() => getValidationResetIn(), [])
  return (
    <ConfirmModal
      art={{ src: limitArt }}
      title={t('review_flow.limit.title')}
      description={t(onRequestReview ? 'review_flow.limit.description' : 'review_flow.limit.description_wait', {
        max: VALIDATION_ATTEMPTS_PER_DAY,
        time: t(`review_flow.limit.reset_${reset.unit}`, { count: reset.count })
      })}
      error={error}
      busy={busy}
      onClose={onClose}
      cancel={{ label: t('review_flow.limit.not_now'), onClick: onClose, testId: 'daily-limit-not-now' }}
      confirm={
        onRequestReview
          ? { label: t('review_flow.limit.request'), onClick: onRequestReview, testId: 'daily-limit-request' }
          : undefined
      }
      size="medium"
      spreadActions
      testId="daily-limit-modal"
    >
      <div className={styles.limitAttempts}>
        <Attempts used={attemptsUsed} testId="daily-limit-attempts" />
      </div>
    </ConfirmModal>
  )
}

type IssuesProps = {
  results: ItemCheck[]
  /** Hidden for viewers who cannot appeal. */
  onRequestReview?: () => void
  onClose: () => void
}

/** The validator's findings per item, in the same dress as the browser's checks before publishing. */
export function AutomatedIssuesModal({ results, onRequestReview, onClose }: IssuesProps) {
  const { t } = useTranslation()
  const sorted = useMemo(() => [...results].sort((a, b) => a.item.name.localeCompare(b.item.name)), [results])
  return (
    <Modal
      title={t('review_flow.issues.title')}
      size="large"
      hideTitle
      showClose
      onClose={onClose}
      testId="automated-issues-modal"
    >
      <div className={issues.content} data-variant="errors_blocking" data-testid="automated-issues-content">
        <div className={issues.header}>
          <span className={issues.headerIcon} data-variant="errors" aria-hidden>
            <ErrorIcon />
          </span>
          <h2 className={issues.title} data-testid="automated-issues-title">
            {t('review_flow.issues.title')}
          </h2>
        </div>
        <p className={issues.intro}>{t('review_flow.issues.intro')}</p>
        <div className={issues.list}>
          {sorted.map(result => (
            <ItemValidationCard
              key={result.item.id}
              subject={toValidationSubject(result.item)}
              issues={result.issues}
              testId={`automated-issues-${result.item.id}`}
            />
          ))}
        </div>
        <div className={issues.footer} data-single={onRequestReview ? undefined : ''}>
          {onRequestReview && (
            <Button
              type="button"
              variant="secondary"
              data-testid="automated-issues-request-review"
              onClick={onRequestReview}
            >
              {t('review_flow.issues.request')}
            </Button>
          )}
          <Button type="button" variant="primary" data-testid="automated-issues-back" onClick={onClose}>
            {t('review_flow.issues.back')}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

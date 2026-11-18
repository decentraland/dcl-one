import { useEffect, useMemo } from 'react'
import { ChevronRightIcon, WarningAmberOutlinedIcon as WarningIcon } from '~/components/Icons'
import approvedArt from '~/assets/send-success.png'
import errorArt from '~/assets/modal-error.png'
import { useTranslation } from '~/intl'
import { Button } from '~/components/Button'
import { ConfirmModal } from '~/components/ConfirmModal'
import { ItemThumbnail } from '~/components/ItemThumbnail'
import { Modal } from '~/components/Modal'
import { PendingModal } from '~/components/PendingModal'
import { StepIndicator } from '~/components/StepIndicator'
import { SuccessModal } from '~/components/SuccessModal'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useCatalystItemSizes } from '~/hooks/useItemSize'
import { useLinkedApprovalFlow } from '~/hooks/useLinkedReview'
import { isSocialLogin, type Session } from '~/lib/auth'
import { getContentsStorageUrl } from '~/lib/builder'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { decodeLinkedUrn, type ThirdParty } from '~/lib/thirdParty'
import { type ReviewedContent } from '~/lib/approveLinkedCollection'
import { ProgressModal } from '~/components/ProgressModal'
import { formatItemBytes } from './formatSize'
import { cx } from '~/lib/cx'
import styles from './ApprovalFlowModal.module.css'
import publishModal from '~/components/CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'
import shared from '~/styles/shared.module.css'

type Props = {
  session: Session
  collection: Collection
  thirdParty: ThirdParty
  /** What the curator reviewed: approving stops if the pending items changed since. */
  review: ReviewedContent
  onClose: () => void
}

const SHOWN_ITEMS = 50

/** The committee's approval of a linked collection: the Merkle root transaction, then the Catalyst uploads. */
export function LinkedApprovalFlowModal({ session, collection, thirdParty, review, onClose }: Props) {
  const { t } = useTranslation()
  const flow = useLinkedApprovalFlow(session, collection, thirdParty, review)
  const { view, plan, steps, start, stop } = flow
  // Each failed item with the step it stopped at, in plain words; the raw errors go to Sentry, not to the screen.
  const failedText = useMemo(
    () =>
      view.kind === 'error'
        ? view.failed
            .map(failure => `${failure.item.name}: ${t(`linked_approval_flow.stage.${failure.stage}`)}`)
            .join('\n')
        : '',
    [view, t]
  )
  useBeforeUnloadGuard(view.kind === 'step' && (view.phase.kind !== 'idle' || flow.running))
  const pendingItems = plan?.pendingItems
  const sizeItems = useMemo(() => pendingItems ?? [], [pendingItems])
  const sizes = useCatalystItemSizes(sizeItems, { enabled: view.kind === 'step' && view.step === 'deploy' })

  useEffect(() => {
    void start()
    return stop
    // Runs once per opened modal.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  switch (view.kind) {
    case 'loading':
      return <PendingModal label={t('approval_flow.loading')} onCancel={onClose} testId="approval-loading" />
    case 'success':
      return (
        <SuccessModal
          title={t('linked_approval_flow.success.title')}
          description={t('linked_approval_flow.success.body', { collection: collection.name })}
          art={approvedArt}
          onDone={onClose}
        />
      )
    case 'error':
      if (view.step === 'changed') {
        return (
          <ConfirmModal
            title={t('approval_flow.changed.title')}
            description={t('linked_approval_flow.changed')}
            onClose={onClose}
            confirm={{ label: t('approval_flow.changed.confirm'), onClick: onClose, testId: 'approval-changed-review' }}
            testId="approval-changed"
          />
        )
      }
      if (view.step === 'has_video') {
        return (
          <ConfirmModal
            title={t('linked_approval_flow.has_video.title')}
            description={t('linked_approval_flow.has_video.description', {
              items: (view.items ?? []).map(item => item.name).join(', ')
            })}
            onClose={onClose}
            confirm={{ label: t('approval_flow.cancel'), onClick: onClose, testId: 'approval-has-video-close' }}
            testId="approval-has-video"
          />
        )
      }
      if (view.step === 'unsubmitted') {
        return (
          <ConfirmModal
            title={t('approval_flow.changed.title')}
            description={t('linked_approval_flow.unsubmitted')}
            onClose={onClose}
            confirm={{ label: t('approval_flow.cancel'), onClick: onClose, testId: 'approval-unsubmitted-close' }}
            testId="approval-unsubmitted"
          />
        )
      }
      return (
        <ConfirmModal
          title={t('approval_flow.error.title_approve')}
          description={t(view.wallet ? 'wallet_error.wrong_account' : `linked_approval_flow.error.${view.step}`)}
          art={{ src: errorArt }}
          onClose={onClose}
          cancel={{ label: t('approval_flow.cancel'), onClick: onClose, testId: 'approval-error-cancel' }}
          confirm={{ label: t('approval_flow.error.retry'), onClick: () => void start(), testId: 'approval-retry' }}
          testId="approval-error"
        >
          {failedText && (
            <div className={styles.detail}>
              <div className={styles.detailHeader}>
                <span>{t('linked_approval_flow.failed_items')}</span>
              </div>
              <pre className={styles.detailText} data-testid="approval-error-detail">
                {failedText}
              </pre>
            </div>
          )}
        </ConfirmModal>
      )
  }

  const { step, phase } = view
  if (phase.kind === 'signing' && !isSocialLogin(session)) {
    return (
      <PendingModal
        title={t('approval_flow.signature_title')}
        label={t('linked_approval_flow.root.signing')}
        testId="approval-signing"
      />
    )
  }
  if (phase.kind === 'pending') {
    return <PendingModal label={t('linked_approval_flow.root.pending')} testId="approval-pending" />
  }
  if (phase.kind === 'uploading') {
    return (
      <ProgressModal
        title={t('approval_flow.title')}
        heading={t('approval_flow.deploy.uploading_title')}
        description={t('approval_flow.deploy.uploading_body')}
        label={t('approval_flow.deploy.uploading')}
        done={phase.done}
        total={phase.total}
        testId="approval-uploading"
      />
    )
  }

  const busy = flow.running
  const items = plan?.pendingItems ?? []
  const single = steps.length === 1
  const isLast = steps.indexOf(step) === steps.length - 1

  return (
    <Modal
      title={t(single ? `linked_approval_flow.${step}.title` : 'approval_flow.title')}
      onClose={onClose}
      closeDisabled={busy}
      size="large"
      testId="approval-flow-modal"
    >
      <div className={cx(publishModal.main, styles.main)} data-single={single || undefined}>
        {!single && <StepIndicator current={steps.indexOf(step) + 1} total={steps.length} testId="approval-steps" />}
        <div className={cx(publishModal.step, styles.step)} data-testid={`approval-step-${step}`}>
          {!single && <h3 className={publishModal.heading}>{t(`linked_approval_flow.${step}.title`)}</h3>}
          <p className={publishModal.text}>
            {t(`linked_approval_flow.${step}.body`, {
              count: items.length,
              slots: plan?.slots.length ? plan.approvalData.cheque.qty : 0
            })}
          </p>
          {step === 'root' && !thirdParty.isApproved && (
            <div className={styles.warning} data-testid="approval-reenables-third-party">
              <WarningIcon aria-hidden />
              <p>{t('linked_approval_flow.root.reenables')}</p>
            </div>
          )}
          <div className={cx(publishModal.table, styles.table)}>
            <div className={cx(publishModal.tableHeader, styles.header)} data-table="deploy">
              <span>{t('approval_flow.columns.item')}</span>
              <span>{t(step === 'deploy' ? 'approval_flow.columns.size' : 'linked_approval_flow.columns.urn')}</span>
            </div>
            <div className={publishModal.tableBody}>
              {items.slice(0, SHOWN_ITEMS).map(item => {
                return (
                  <div className={styles.row} key={item.id} data-table="deploy" data-testid="approval-item-row">
                    <ItemName item={item} />
                    {step === 'deploy' ? (
                      <span data-testid="approval-item-size">
                        {formatItemBytes(t, sizes, item.id) ?? (
                          <span
                            className={cx(shared.spinner, styles.sizeSpinner)}
                            role="status"
                            aria-label={t('approval_flow.size_measuring')}
                          />
                        )}
                      </span>
                    ) : (
                      <span title={item.urn}>{decodeLinkedUrn(item.urn)?.itemSuffix ?? item.urn}</span>
                    )}
                  </div>
                )
              })}
              {items.length > SHOWN_ITEMS && (
                <div className={styles.row} data-table="deploy">
                  <span>{t('linked_approval_flow.more_items', { count: items.length - SHOWN_ITEMS })}</span>
                </div>
              )}
            </div>
          </div>
          <div className={cx(publishModal.footer, styles.footer)}>
            <Button type="button" variant="secondary" disabled={busy} onClick={onClose}>
              {t('approval_flow.cancel')}
            </Button>
            <Button
              type="button"
              loading={busy}
              data-testid={`approval-${step}`}
              onClick={() => void (step === 'root' ? flow.runRoot() : flow.runDeploy())}
            >
              {t(`linked_approval_flow.${step}.action`)}
              {!isLast && <ChevronRightIcon fontSize="small" />}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  )
}

function ItemName({ item }: { item: Item }) {
  const thumbnail = item.contents[item.thumbnail]
  return (
    <div className={styles.nameCell}>
      <span className={publishModal.thumb}>
        <ItemThumbnail src={thumbnail ? getContentsStorageUrl(thumbnail) : null} rarity={item.rarity} />
      </span>
      <span title={item.name}>{item.name}</span>
    </div>
  )
}

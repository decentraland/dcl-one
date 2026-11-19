import { useEffect, useMemo, useState } from 'react'
import { CheckIcon, ChevronRightIcon, ContentCopyIcon } from '../../Icons'
import approvedArt from '../../../assets/send-success.png'
import errorArt from '../../../assets/modal-error.png'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '../../Button'
import { CollectionMosaic } from '../../CollectionMosaic'
import { ConfirmModal } from '../../ConfirmModal'
import { BodyShapeIcon, CategoryIcon } from '../../ItemIcons'
import { ItemThumbnail } from '../../ItemThumbnail'
import { Modal } from '../../Modal'
import { NoteField } from '../../NoteField'
import { RarityPill } from '../../RarityPill'
import { StepIndicator } from '../../StepIndicator'
import { SuccessModal } from '../../SuccessModal'
import { Tooltip } from '../../Tooltip'
import { ProgressModal } from '../../ProgressModal'
import { PendingModal } from '../../PendingModal'
import { useCatalystItemSizes } from '../../../hooks/useItemSize'
import {
  useApprovalFlow,
  type ApprovalMode,
  type ApprovalPlan,
  type ApprovalStep
} from '../../../hooks/useApprovalFlow'
import { useBeforeUnloadGuard } from '../../../hooks/useBeforeUnloadGuard'
import { track } from '@/data/lib/creators/analytics'
import { isSocialLogin, type Session } from '@/data/lib/creators/auth'
import { copyToClipboard } from '@/data/lib/creators/clipboard'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import type { Collection } from '@/data/lib/creators/collections'
import type { CollectionCuration } from '@/data/lib/creators/curation'
import { NOTE_MAX_LENGTH } from '@/data/lib/creators/events'
import { getItemBodyShapeType, type Item } from '@/data/lib/creators/items'
import { formatItemBytes } from './formatSize'
import { cx } from '@/data/lib/creators/cx'
import publishModal from '../../CollectionDetailPage/PublishCollectionModal/PublishCollectionModal.module.css'
import styles from './ApprovalFlowModal.module.css'
import shared from '../../../styles/shared.module.css'

type Props = {
  session: Session
  collection: Collection
  curation: CollectionCuration | null
  mode: ApprovalMode
  /** The items as the curator reviewed them: approving stops if the creator saved changes since. */
  items: Item[]
  /** Offers a note for the creator on the approving step, where builder-server keeps it. */
  withNote?: boolean
  onClose: () => void
}

const COPIED_FEEDBACK_MS = 1500

const shortHash = (hash: string) => `${hash.slice(0, 6)}...${hash.slice(-6)}`

/** The committee's approval: one stepper screen per transaction or upload. */
export function ApprovalFlowModal({ session, collection, curation, mode, items, withNote = false, onClose }: Props) {
  const { t } = useTranslation()
  const [note, setNote] = useState('')
  const flow = useApprovalFlow(session, collection, curation, mode, items, note)
  const { view, plan, start, stop } = flow
  const errorText = useMemo(
    () =>
      view.kind !== 'error'
        ? ''
        : view.failed.length
          ? view.failed.map(failure => `${failure.item.name}: ${failure.message}`).join('\n')
          : (view.detail ?? ''),
    [view]
  )
  // Reloading mid-transaction or mid-upload loses track of it; the flow would have to start over.
  useBeforeUnloadGuard(view.kind === 'step' && (view.phase.kind !== 'idle' || flow.running))

  // Runs once: the collection and curation refetch mid-flow, which recreates `start`, and re-running it would
  // reopen the flow from its first step.
  useEffect(() => {
    void start()
    return stop
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  switch (view.kind) {
    case 'loading':
      return <PendingModal label={t('approval_flow.loading')} onCancel={onClose} testId="approval-loading" />
    case 'success':
      return (
        <SuccessModal
          title={t(`approval_flow.success.title_${mode}`)}
          description={t(`approval_flow.success.body_${mode}`, { collection: collection.name })}
          art={approvedArt}
          onDone={onClose}
        />
      )
    case 'error':
      if (view.step === 'changed') {
        return (
          <ConfirmModal
            title={t('approval_flow.changed.title')}
            description={t('approval_flow.changed.description')}
            onClose={onClose}
            cancel={{ label: t('approval_flow.cancel'), onClick: onClose, testId: 'approval-changed-cancel' }}
            confirm={{
              label: t('approval_flow.changed.confirm'),
              onClick: () => {
                void flow.reloadItems()
                onClose()
              },
              testId: 'approval-changed-review'
            }}
            testId="approval-changed"
          />
        )
      }
      return (
        <ConfirmModal
          title={t(`approval_flow.error.title_${mode}`)}
          description={t(view.wallet ? 'wallet_error.wrong_account' : `approval_flow.error.${view.step}`)}
          art={{ src: errorArt }}
          onClose={onClose}
          cancel={{ label: t('approval_flow.cancel'), onClick: onClose, testId: 'approval-error-cancel' }}
          confirm={{ label: t('approval_flow.error.retry'), onClick: () => void start(), testId: 'approval-retry' }}
          testId="approval-error"
        >
          <ErrorDetail collectionId={collection.id} text={errorText} />
        </ConfirmModal>
      )
  }

  const { step, phase } = view
  // Custodial wallets sign without a prompt to wait on: their screen just spins the confirm button.
  if (phase.kind === 'signing' && !isSocialLogin(session)) {
    return (
      <PendingModal
        title={t('approval_flow.signature_title')}
        label={t(`approval_flow.${step}.signing`)}
        onCancel={flow.cancelSigning}
        testId="approval-signing"
      />
    )
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
  if (phase.kind === 'pending') {
    return (
      <PendingModal
        label={t(`approval_flow.${step}.pending`, { tx: phase.tx, txs: phase.txs })}
        testId="approval-pending"
      />
    )
  }

  // A request the curator backed out of is still open in the wallet: closing would lose track of it.
  const busy = phase.kind !== 'idle' || flow.running
  const run = { rescue: flow.runRescue, deploy: flow.runDeploy, approve: flow.runApprove }[step]
  const count = step === 'rescue' ? plan.rescue.length : plan.deploy.length
  const single = plan.steps.length === 1
  // Only a step with another one after it continues; the last one completes the approval.
  const isLast = plan.steps.indexOf(step) === plan.steps.length - 1

  return (
    <Modal
      title={t(single ? `approval_flow.${step}.title` : 'approval_flow.title')}
      onClose={onClose}
      closeDisabled={busy}
      size="large"
      testId="approval-flow-modal"
    >
      <div className={cx(publishModal.main, styles.main)} data-single={single || undefined}>
        {!single && (
          <StepIndicator current={plan.steps.indexOf(step) + 1} total={plan.steps.length} testId="approval-steps" />
        )}
        <div className={cx(publishModal.step, styles.step)} data-testid={`approval-step-${step}`}>
          {!single && <h3 className={publishModal.heading}>{t(`approval_flow.${step}.title`)}</h3>}
          <p className={publishModal.text}>
            {t(mode === 'enable' ? 'approval_flow.enable_body' : `approval_flow.${step}.body`, { count })}
          </p>
          <StepTable step={step} plan={plan} collection={collection} />
          {withNote && mode === 'approve' && isLast && (
            <NoteField
              label={t('approval_flow.note.label')}
              value={note}
              onChange={setNote}
              maxLength={NOTE_MAX_LENGTH}
              placeholder={t('approval_flow.note.placeholder')}
              disabled={busy}
              testId="approval-note"
            />
          )}
          <div className={cx(publishModal.footer, styles.footer)}>
            <Button type="button" variant="secondary" disabled={busy} onClick={onClose}>
              {t('approval_flow.cancel')}
            </Button>
            {/* Stays clickable after backing out of the wallet: it shows the request still open there. */}
            <Button
              type="button"
              loading={phase.kind !== 'idle'}
              data-testid={`approval-${step}`}
              onClick={() => void run()}
            >
              {t(`approval_flow.${step}.action`)}
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

function StepTable({ step, plan, collection }: { step: ApprovalStep; plan: ApprovalPlan; collection: Collection }) {
  const { t } = useTranslation()
  const sizes = useCatalystItemSizes(plan.deploy, { enabled: step === 'deploy' })
  const column = (key: string, optional = false) => (
    <span data-optional={optional || undefined}>{t(`approval_flow.columns.${key}`)}</span>
  )

  if (step === 'approve') {
    return (
      <div className={cx(publishModal.table, styles.table)}>
        <div className={cx(publishModal.tableHeader, styles.header)} data-table={step}>
          {column('collection')}
          {column('items')}
        </div>
        <div className={styles.row} data-table={step} data-testid="approval-collection-row">
          <div className={styles.nameCell}>
            <span className={publishModal.mosaicFrame}>
              <CollectionMosaic collectionId={collection.id} itemCount={collection.itemCount} />
            </span>
            <span title={collection.name}>{collection.name}</span>
          </div>
          <span>{t('approval_flow.item_count', { count: collection.itemCount })}</span>
        </div>
      </div>
    )
  }

  if (step === 'deploy') {
    return (
      <div className={cx(publishModal.table, styles.table)}>
        <div className={cx(publishModal.tableHeader, styles.header)} data-table={step}>
          {column('item')}
          {column('size')}
        </div>
        <div className={publishModal.tableBody}>
          {plan.deploy.map(item => {
            return (
              <div className={styles.row} key={item.id} data-table={step} data-testid="approval-item-row">
                <ItemName item={item} />
                <span data-testid="approval-item-size">
                  {formatItemBytes(t, sizes, item.id) ?? (
                    <span
                      className={cx(shared.spinner, styles.sizeSpinner)}
                      role="status"
                      aria-label={t('approval_flow.size_measuring')}
                    />
                  )}
                </span>
              </div>
            )
          })}
        </div>
      </div>
    )
  }

  return (
    <div className={cx(publishModal.table, styles.table)}>
      <div className={cx(publishModal.tableHeader, styles.header)} data-table={step}>
        {column('item')}
        {column('body_shape', true)}
        {column('category', true)}
        {column('rarity')}
        {column('hash', true)}
      </div>
      <div className={publishModal.tableBody}>
        {plan.rescue.map(({ item, contentHash }) => {
          const bodyShape = getItemBodyShapeType(item)
          return (
            <div className={styles.row} key={item.id} data-table={step} data-testid="approval-item-row">
              <ItemName item={item} />
              <span data-optional>{bodyShape ? <BodyShapeIcon bodyShape={bodyShape} withLabel /> : '—'}</span>
              <span data-optional>
                {item.data.category ? <CategoryIcon category={item.data.category} withLabel /> : '—'}
              </span>
              <span>{item.rarity && <RarityPill rarity={item.rarity} />}</span>
              <span data-optional title={contentHash}>
                {shortHash(contentHash)}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

/** The raw failure, which only curators see: support asks for it, so it can be copied whole. */
function ErrorDetail({ text, collectionId }: { text: string; collectionId: string }) {
  const { t } = useTranslation()
  const [copied, setCopied] = useState<boolean | null>(null)
  useEffect(() => {
    if (copied === null) return
    const reset = setTimeout(() => setCopied(null), COPIED_FEEDBACK_MS)
    return () => clearTimeout(reset)
  }, [copied])
  if (!text) return null
  const label =
    copied === null
      ? t('approval_flow.error.copy')
      : t(copied ? 'approval_flow.error.copied' : 'collection_detail_page.actions.copy_failed')
  return (
    <div className={styles.detail}>
      <div className={styles.detailHeader}>
        <span>{t('approval_flow.error.details')}</span>
        <Tooltip content={label} asChild placement="top">
          <button
            className={styles.copyButton}
            type="button"
            aria-label={label}
            data-copied={copied || undefined}
            data-testid="approval-error-copy"
            onClick={() => {
              void copyToClipboard(text).then(setCopied)
              track('bd_approval_error_copied', { collection_id: collectionId })
            }}
          >
            {copied ? <CheckIcon fontSize="small" /> : <ContentCopyIcon fontSize="small" />}
          </button>
        </Tooltip>
      </div>
      <pre className={styles.detailText} data-testid="approval-error-detail">
        {text}
      </pre>
    </div>
  )
}

import { useMemo, useState } from 'react'
import { Button } from '@/features/creators/components/Button'
import { AddItemsModal, type AddItemsPrefill } from '@/features/creators/components/CollectionDetailPage/AddItemsModal'
import { CollectionNameModal } from '@/features/creators/components/CollectionNameModal'
import { Modal } from '@/features/creators/components/Modal'
import { Select } from '@/features/creators/components/Select'
import { useSaveCollection } from '@/features/creators/hooks/useCollection'
import { useDraftCollections } from '@/features/creators/hooks/useCollections'
import { useTranslation } from '@/data/lib/creators/intl'
import { isCollectionLocked, type Collection } from '@/data/lib/creators/collections'
import { buildNewCollection } from '@/data/lib/creators/saveCollection'
import { useWallet } from '@/features/creators/store/wallet'
import styles from './LivePreviewPage.module.css'
import shared from '@/features/creators/styles/shared.module.css'

type Props = {
  /** The streamed GLB, snapshotted when the flow opened. */
  file: File
  prefill: AddItemsPrefill
  onClose: () => void
  /** The item was uploaded into this collection. */
  onDone: (collectionId: string) => void
}

type Step = { kind: 'pick' } | { kind: 'create' } | { kind: 'add'; collection: Collection }

/**
 * Sign in → pick (or create) a draft collection → the regular add-items modal with the model and the
 * tuning from the live preview prefilled.
 */
export function AddToCollectionFlow({ file, prefill, onClose, onDone }: Props) {
  const { t } = useTranslation()
  const { session, signIn } = useWallet()
  const address = session?.address
  const [step, setStep] = useState<Step>({ kind: 'pick' })
  const [targetId, setTargetId] = useState<string | null>(null)
  const drafts = useDraftCollections(address)
  const saveCollection = useSaveCollection(address)

  const targets = useMemo(() => (drafts.data ?? []).filter(c => !isCollectionLocked(c)), [drafts.data])
  const options = useMemo(() => targets.map(c => ({ value: c.id, label: c.name })), [targets])

  if (!address) {
    return (
      <Modal title={t('live_preview.sign_in.title')} onClose={onClose} testId="live-preview-sign-in">
        <div className={styles.modalBody}>
          <p className={styles.modalText}>{t('live_preview.sign_in.description')}</p>
          <div className={shared.modalActions}>
            <Button type="button" variant="primary" data-testid="live-preview-sign-in-action" onClick={() => signIn()}>
              {t('collection_detail_page.sign_in.action')}
            </Button>
          </div>
        </div>
      </Modal>
    )
  }

  if (step.kind === 'create') {
    return (
      <CollectionNameModal
        variant="create"
        isPending={saveCollection.isPending}
        error={saveCollection.error?.message ?? null}
        onSubmit={name =>
          saveCollection.mutate(buildNewCollection(name, address), {
            onSuccess: collection => setStep({ kind: 'add', collection })
          })
        }
        onClose={() => {
          saveCollection.reset()
          setStep({ kind: 'pick' })
        }}
      />
    )
  }

  if (step.kind === 'add') {
    return (
      <AddItemsModal
        collection={step.collection}
        address={address}
        files={[file]}
        prefill={prefill}
        onClose={() => onDone(step.collection.id)}
      />
    )
  }

  return (
    <Modal title={t('live_preview.add.title')} onClose={onClose} testId="live-preview-pick-collection">
      <div className={styles.modalBody}>
        <p className={styles.modalText}>{t('live_preview.add.description')}</p>
        {drafts.isLoading ? (
          <p className={styles.modalText} role="status" data-testid="live-preview-pick-loading">
            {t('collection_detail_page.item_actions.move_modal.loading')}
          </p>
        ) : targets.length === 0 ? (
          <p className={styles.modalText} data-testid="live-preview-pick-empty">
            {t('live_preview.add.empty')}
          </p>
        ) : (
          <label className={styles.modalField}>
            {t('collection_detail_page.item_actions.move_modal.collection')}
            <Select
              value={targetId}
              options={options}
              placeholder={t('collection_detail_page.item_actions.move_modal.placeholder')}
              onChange={setTargetId}
              testId="live-preview-pick-target"
            />
          </label>
        )}
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            data-testid="live-preview-pick-create"
            onClick={() => setStep({ kind: 'create' })}
          >
            {t('live_preview.add.new_collection')}
          </Button>
          <Button
            type="button"
            disabled={!targetId}
            data-testid="live-preview-pick-confirm"
            onClick={() => {
              const collection = targets.find(c => c.id === targetId)
              if (collection) setStep({ kind: 'add', collection })
            }}
          >
            {t('live_preview.add.continue')}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

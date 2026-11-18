import { useMemo, useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { isCollectionLocked, type Collection } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { Button } from '@/features/creators/components/Button'
import { Modal } from '@/features/creators/components/Modal'
import { Select } from '@/features/creators/components/Select'
import styles from './MoveItemModal.module.css'
import shared from '@/features/creators/styles/shared.module.css'

type Props = {
  item: Item
  /** The creator's draft collections; `undefined` while they load. */
  collections: Collection[] | undefined
  isMoving: boolean
  isLoading: boolean
  onConfirm: (collection: Collection) => void
  onClose: () => void
}

/** Pick the draft collection an item moves to. Locked drafts (publish in flight) can't take items. */
export function MoveItemModal({ item, collections, isMoving, isLoading, onConfirm, onClose }: Props) {
  const { t } = useTranslation()
  const [targetId, setTargetId] = useState<string | null>(null)

  const targets = useMemo(
    () => (collections ?? []).filter(c => c.id !== item.collectionId && !isCollectionLocked(c)),
    [collections, item.collectionId]
  )
  const options = useMemo(() => targets.map(c => ({ value: c.id, label: c.name })), [targets])

  return (
    <Modal
      title={t('collection_detail_page.item_actions.move_modal.title')}
      onClose={onClose}
      closeDisabled={isMoving}
      testId="move-item-modal"
    >
      <div className={styles.body}>
        <p className={styles.text} data-testid="move-item-modal-description">
          {t('collection_detail_page.item_actions.move_modal.description', { name: item.name })}
        </p>
        {isLoading ? (
          <div className={styles.loading} role="status" data-testid="move-item-loading">
            <span className={shared.spinner} aria-hidden />
            {t('collection_detail_page.item_actions.move_modal.loading')}
          </div>
        ) : targets.length === 0 ? (
          <p className={styles.text} data-testid="move-item-empty">
            {t('collection_detail_page.item_actions.move_modal.empty')}
          </p>
        ) : (
          <label className={styles.field}>
            {t('collection_detail_page.item_actions.move_modal.collection')}
            <Select
              value={targetId}
              options={options}
              placeholder={t('collection_detail_page.item_actions.move_modal.placeholder')}
              onChange={setTargetId}
              testId="move-item-target"
            />
          </label>
        )}
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            disabled={isMoving}
            data-testid="move-item-cancel"
            onClick={onClose}
          >
            {t('collection_detail_page.item_actions.move_modal.cancel')}
          </Button>
          <Button
            type="button"
            loading={isMoving}
            disabled={!targetId}
            data-testid="move-item-confirm"
            onClick={() => {
              const target = targets.find(c => c.id === targetId)
              if (target) onConfirm(target)
            }}
          >
            {t('collection_detail_page.item_actions.move_modal.confirm')}
          </Button>
        </div>
      </div>
    </Modal>
  )
}

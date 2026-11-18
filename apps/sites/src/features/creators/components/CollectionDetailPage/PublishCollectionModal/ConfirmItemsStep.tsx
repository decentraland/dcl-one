import { useEffect, useState } from 'react'
import { ChevronLeftIcon, ChevronRightIcon } from '../../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { useDeleteItem } from '../../../hooks/usePublishCollection'
import { useThumbnailEditor } from '../../../hooks/useThumbnailEditor'
import { useSaveItem } from '../../../hooks/useSaveItem'
import { THUMBNAIL_PATH } from '@/data/lib/creators/itemFiles'
import { type Collection } from '@/data/lib/creators/collections'
import { isItemTooBigError } from '@/data/lib/creators/itemSize'
import { canDeleteItem, type Item } from '@/data/lib/creators/items'
import { useNotifications } from '@/data/lib/creators/notifications'
import { Button } from '../../Button'
import { type ThumbnailPatch } from '../../ThumbnailModal'
import { Checkbox } from '../../Checkbox'
import { DeleteItemModal } from '../DeleteItemModal'
import { PublishItemRow } from './PublishItemRow'
import { cx } from '@/data/lib/creators/cx'
import publishModal from './PublishCollectionModal.module.css'
import itemRow from './PublishItemRow.module.css'

type Props = {
  address: string
  collection: Collection
  items: Item[]
  /** Items can't be edited or removed: the collection is locked by a publish. */
  readOnly?: boolean
  onBusyChange: (busy: boolean) => void
  onBack: () => void
  onConfirm: () => void
}

/** Step 2: review every item; names and rarities are editable in place, items can be removed. */
export function ConfirmItemsStep({
  address,
  collection,
  items,
  readOnly = false,
  onBusyChange,
  onBack,
  onConfirm
}: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const [accepted, setAccepted] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<Item | null>(null)
  // Thumbnail edits stay local until the row is saved, so cancelling the row discards them too.
  const [thumbnailPatch, setThumbnailPatch] = useState<ThumbnailPatch | null>(null)

  const updateItem = useSaveItem(address)
  const deleteItem = useDeleteItem(address)
  const thumbnailEditor = useThumbnailEditor((patch, subject) => {
    // A pick that finishes after the user moved to another row belongs to the row it started on.
    if (subject.kind === 'item' && subject.item.id === editingId) setThumbnailPatch(patch)
  })

  const withActions = !readOnly
  const canContinue = accepted && items.length > 0 && editingId === null
  // A new thumbnail or rarity changes the item's files: the save refuses one that leaves it over its size limit.
  const saveErrorMessage = isItemTooBigError(updateItem.error)
    ? t('add_items_modal.file_error.size_exceeded', updateItem.error.messageParams)
    : undefined

  const isBusy = updateItem.isPending || deleteItem.isPending
  useEffect(() => {
    onBusyChange(isBusy)
  }, [isBusy, onBusyChange])

  function stopEditing() {
    updateItem.reset()
    setEditingId(null)
    setThumbnailPatch(null)
  }

  function saveRow(item: Item, changes: { name: string; rarity: string }) {
    if (!thumbnailPatch && changes.name === item.name && changes.rarity === item.rarity) {
      stopEditing()
      return
    }
    updateItem.mutate(
      {
        item: { ...item, ...changes },
        thumbnail: thumbnailPatch?.contents[THUMBNAIL_PATH],
        imageStale: changes.rarity !== item.rarity
      },
      { onSuccess: stopEditing }
    )
  }

  function confirmDelete() {
    if (!deleting) return
    deleteItem.mutate(deleting, {
      onSuccess: () => setDeleting(null),
      onError: () => showToast(t('collection_detail_page.delete_item.error'), { type: 'error' })
    })
  }

  return (
    <div className={publishModal.step} data-testid="publish-items-step">
      <h3 className={publishModal.heading}>{t('publish_collection_modal.items_step.title')}</h3>
      <div>
        <p className={publishModal.lead}>{t('publish_collection_modal.items_step.subtitle')}</p>
        <p className={publishModal.text}>{t('publish_collection_modal.items_step.description')}</p>
      </div>
      <div className={publishModal.table}>
        <div className={cx(publishModal.tableHeader, itemRow.columns)} data-no-actions={!withActions || undefined}>
          <span>{t('publish_collection_modal.items_step.columns.item')}</span>
          <span>{t('publish_collection_modal.items_step.columns.body_shape')}</span>
          <span>{t('publish_collection_modal.items_step.columns.category')}</span>
          <span>{t('publish_collection_modal.items_step.columns.rarity')}</span>
          {withActions && (
            <span data-testid="publish-items-actions-column">
              {t('publish_collection_modal.items_step.columns.actions')}
            </span>
          )}
        </div>
        <div className={publishModal.tableBody} data-testid="publish-items-list">
          {items.length === 0 ? (
            <p className={publishModal.inlineNote} data-testid="publish-items-empty">
              {t('publish_collection_modal.items_step.empty')}
            </p>
          ) : (
            items.map(item => (
              <PublishItemRow
                key={item.id}
                item={item}
                isEditing={editingId === item.id}
                isSaving={editingId === item.id && updateItem.isPending}
                saveError={editingId === item.id && updateItem.isError}
                saveErrorMessage={saveErrorMessage}
                withActions={withActions}
                canEdit={!readOnly}
                canDelete={!readOnly && items.length > 1 && canDeleteItem(collection, address)}
                locked={editingId !== null && editingId !== item.id}
                pendingThumbnail={editingId === item.id ? (thumbnailPatch?.thumbnail ?? null) : null}
                onEdit={() => {
                  updateItem.reset()
                  setEditingId(item.id)
                }}
                onCancelEdit={stopEditing}
                onEditThumbnail={() => thumbnailEditor.edit({ kind: 'item', item })}
                onSave={changes => saveRow(item, changes)}
                onDelete={() => {
                  deleteItem.reset()
                  setDeleting(item)
                }}
              />
            ))
          )}
        </div>
      </div>
      <Checkbox checked={accepted} onChange={setAccepted} testId="publish-items-accept">
        {t('publish_collection_modal.items_step.checkbox')}
      </Checkbox>
      <div className={publishModal.footer}>
        <Button type="button" variant="secondary" data-testid="publish-back" onClick={onBack}>
          <ChevronLeftIcon fontSize="small" />
          {t('publish_collection_modal.back')}
        </Button>
        <Button type="button" disabled={!canContinue} data-testid="publish-items-confirm" onClick={onConfirm}>
          {t('publish_collection_modal.items_step.confirm')}
          <ChevronRightIcon fontSize="small" />
        </Button>
      </div>

      {thumbnailEditor.modal}

      {deleting && (
        <DeleteItemModal
          item={deleting}
          isDeleting={deleteItem.isPending}
          onCancel={() => setDeleting(null)}
          onConfirm={confirmDelete}
        />
      )}
    </div>
  )
}

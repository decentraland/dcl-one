import { useState } from 'react'
import { useNavigate } from 'react-router'
import { useTranslation } from '@/data/lib/creators/intl'
import { useMediaQuery } from '@/features/creators/hooks/useMediaQuery'
import { useDeleteItem } from '@/features/creators/hooks/usePublishCollection'
import { track } from '@/data/lib/creators/analytics'
import { copyToClipboard } from '@/data/lib/creators/clipboard'
import { type Collection } from '@/data/lib/creators/collections'
import { type Item } from '@/data/lib/creators/items'
import { type SaveItemPatch } from '@/data/lib/creators/itemSaveQueue'
import { isLinkedItemPublished, type ItemCuration } from '@/data/lib/creators/linkedItemStatus'
import { useNotifications } from '@/data/lib/creators/notifications'
import { buildLinkedItemUrn, decodeLinkedUrn } from '@/data/lib/creators/thirdParty'
import { isRefusedRequest } from '@/data/lib/creators/builder'
import { captureError } from '@/data/lib/creators/monitoring'
import { media } from '@/data/lib/creators/theme'
import { ActionsMenu, ActionsMenuDivider, ActionsMenuItem } from '@/features/creators/components/ActionsMenu'
import { EditUrnModal } from './EditUrnModal'
import { DeleteItemModal } from '../DeleteItemModal'

type Props = {
  item: Item
  collection: Collection
  address: string
  /** The item's latest curation; null when it has none, undefined while unknown. */
  curation: ItemCuration | null | undefined
  /** Whether the collection takes edits: a locked one refuses URN changes and deletes. */
  canEdit: boolean
  /** Saves the item through the view's per-item queue, so this never undoes a mapping save still in flight. */
  saveItemPatch: SaveItemPatch
  onDeleted?: () => void
}

type Dialog = 'urn' | 'delete' | null

/** The ⋯ menu of a linked item row: open in the editor, copy or edit its URN, delete it while unpublished. */
export function LinkedItemActionsMenu({
  item,
  collection,
  address,
  curation,
  canEdit,
  saveItemPatch,
  onDeleted
}: Props) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const showToast = useNotifications(state => state.showToast)
  const [urnSave, setUrnSave] = useState<'idle' | 'saving' | 'failed'>('idle')
  const deleteItem = useDeleteItem(address)
  const [dialog, setDialog] = useState<Dialog>(null)
  // Small screens are a viewer: only copying the URN stays available there.
  const compact = useMediaQuery(media.noActions)

  // Until its curation is known the item is treated as published: a locked action is better than a 409.
  const isPublished = curation === undefined || isLinkedItemPublished(item, curation)
  const decoded = decodeLinkedUrn(item.urn)
  const canEditUrn = canEdit && !compact && !isPublished && !!decoded?.itemSuffix
  const canDelete = canEdit && !compact && !isPublished

  async function copyUrn() {
    const copied = !!item.urn && (await copyToClipboard(item.urn))
    if (copied) showToast(t('collection_detail_page.item_actions.copied_urn'))
    else showToast(t('collection_detail_page.actions.copy_failed'), { type: 'error' })
  }

  function closeDialog() {
    setDialog(null)
    setUrnSave('idle')
    deleteItem.reset()
  }

  function saveUrn(suffix: string) {
    const urn = buildLinkedItemUrn(collection.urn, suffix)
    setUrnSave('saving')
    saveItemPatch(item, { urn }).then(
      () => {
        track('Change TP Item URN', { oldURN: item.urn, newURN: urn })
        closeDialog()
        showToast(t('linked_collection.urn_saved'))
      },
      error => {
        if (!isRefusedRequest(error)) captureError(error, { flow: 'linked_item_urn', itemId: item.id })
        setUrnSave('failed')
      }
    )
  }

  function confirmDelete() {
    deleteItem.mutate(item, {
      onSuccess: () => {
        closeDialog()
        onDeleted?.()
        showToast(t('collection_detail_page.item_actions.deleted', { name: item.name }))
      },
      onError: () => showToast(t('collection_detail_page.delete_item.error'), { type: 'error' })
    })
  }

  return (
    <>
      <ActionsMenu label={t('collection_detail_page.row_actions')} variant="row" testId="item-actions">
        {!compact && (
          <ActionsMenuItem
            testId="item-preview"
            onClick={() => navigate(`/collections/editor?collection=${collection.id}&item=${item.id}`)}
          >
            {t('collection_detail_page.item_actions.open_in_editor')}
          </ActionsMenuItem>
        )}
        {item.urn && (
          <ActionsMenuItem testId="item-copy-urn" onClick={() => void copyUrn()}>
            {t('collection_detail_page.item_actions.copy_urn')}
          </ActionsMenuItem>
        )}
        {canEditUrn && (
          <ActionsMenuItem testId="item-edit-urn" onClick={() => setDialog('urn')}>
            {t('linked_collection.item_actions.edit_urn')}
          </ActionsMenuItem>
        )}
        {canDelete && (
          <>
            <ActionsMenuDivider />
            <ActionsMenuItem testId="item-delete" onClick={() => setDialog('delete')}>
              {t('collection_detail_page.item_actions.delete')}
            </ActionsMenuItem>
          </>
        )}
      </ActionsMenu>

      {dialog === 'urn' && decoded?.itemSuffix && (
        <EditUrnModal
          kind="item"
          urnPrefix={collection.urn}
          initialSuffix={decoded.itemSuffix}
          isPending={urnSave === 'saving'}
          error={urnSave === 'failed' ? t('linked_collection.urn_error') : null}
          onSubmit={saveUrn}
          onClose={closeDialog}
        />
      )}
      {dialog === 'delete' && (
        <DeleteItemModal
          item={item}
          isDeleting={deleteItem.isPending}
          onCancel={closeDialog}
          onConfirm={confirmDelete}
        />
      )}
    </>
  )
}

import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useTranslation } from '~/intl'
import { useWallet } from '~/store/wallet'
import { useDraftCollections } from '~/hooks/useCollections'
import { useFeatureFlag } from '~/hooks/useFeatureFlag'
import { useMediaQuery } from '~/hooks/useMediaQuery'
import { useMoveItem, useResetItem } from '~/hooks/useItem'
import { useDeleteItem } from '~/hooks/usePublishCollection'
import { copyToClipboard } from '~/lib/clipboard'
import { FeatureFlag } from '~/lib/featureFlags'
import {
  canManageCollectionItems,
  canSellCollectionItems,
  hasBeenApproved,
  isCollectionLocked,
  type Collection
} from '~/lib/collections'
import { ItemSyncStatus, type ItemSync } from '~/lib/itemSync'
import { canDeleteItem, canEditItemPrice, canManageItem, type Item } from '~/lib/items'
import { type Session } from '~/lib/auth'
import { type ItemListing } from '~/lib/listings'
import { useNotifications } from '~/lib/notifications'
import { media } from '~/styles/theme'
import { ActionsMenu, ActionsMenuDivider, ActionsMenuItem } from '~/components/ActionsMenu'
import { DeleteItemModal } from '../DeleteItemModal'
import { MoveItemModal } from './MoveItemModal'
import { ResetItemModal } from './ResetItemModal'
import { RemoveListingFlow, UpdatePriceFlow } from '../SellItemFlow'

type Props = {
  item: Item
  collection: Collection
  address: string
  /** Catalyst sync of the item; absent while unknown. */
  sync?: ItemSync
  /** The item's primary listing, when the collection is on the market: `null` when not on sale. */
  listing?: ItemListing | null
  /** An automated review is judging the collection's content: resetting it would change what is judged. */
  reviewLocked?: boolean
}

// The sale dialogs keep the listing they opened with: the flows themselves rewrite the listings cache
// (drop on cancel, set on re-list), and the live prop vanishing must not unmount them mid-flow.
type Dialog =
  | 'move'
  | 'reset'
  | 'delete'
  | { kind: 'update-price'; listing: ItemListing & { tradeId: string }; session: Session }
  | { kind: 'remove-listing'; listing: ItemListing; session: Session }
  | null

export function ItemActionsMenu({ item, collection, address, sync, listing, reviewLocked = false }: Props) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const showToast = useNotifications(state => state.showToast)
  const session = useWallet(state => state.session)
  const [dialog, setDialog] = useState<Dialog>(null)

  const moveItem = useMoveItem(address)
  const resetItem = useResetItem(address)
  const deleteItem = useDeleteItem(address)
  const drafts = useDraftCollections(address, dialog === 'move')

  // Small screens are mostly a viewer: only copying the URN and the sale actions stay available there.
  const compact = useMediaQuery(media.noActions)

  const canManage = canManageItem(collection, item, address)
  const canCopyUrn = !!item.urn
  const canEditDraft = !compact && canManage && !collection.isPublished && !isCollectionLocked(collection)
  const canDelete = !compact && canDeleteItem(collection, address)
  const onMarket = hasBeenApproved(collection) && !!listing && !!session
  const canListItems = useFeatureFlag(FeatureFlag.OFFCHAIN_PUBLIC_ITEM_ORDERS).enabled
  // An off-chain order is cancelled by whoever may sell (the owner, who signed it); a legacy store
  // price is cleared on the collection contract, which only the creator and collaborators may edit.
  const canRemove =
    onMarket &&
    (listing.tradeId
      ? canListItems && canSellCollectionItems(collection, address)
      : canManageCollectionItems(collection, address))
  const canEditPrice = canListItems && !!session && canEditItemPrice(collection, item, listing, address)
  const canReset = !compact && canManage && !reviewLocked && sync?.status === ItemSyncStatus.UNSYNCED && !!sync.entity
  const canPreview = !compact

  async function copyUrn() {
    const copied = !!item.urn && (await copyToClipboard(item.urn))
    if (copied) showToast(t('collection_detail_page.item_actions.copied_urn'))
    else showToast(t('collection_detail_page.actions.copy_failed'), { type: 'error' })
  }

  function openEditor() {
    navigate(`/collections/editor?collection=${collection.id}&item=${item.id}`)
  }

  function openSaleDialog(kind: 'update-price' | 'remove-listing') {
    if (!listing || !session) return
    if (kind === 'remove-listing') setDialog({ kind, listing, session })
    else if (listing.tradeId) setDialog({ kind, listing: { ...listing, tradeId: listing.tradeId }, session })
  }

  function closeDialog() {
    setDialog(null)
    moveItem.reset()
    resetItem.reset()
    deleteItem.reset()
  }

  function confirmMove(target: Collection) {
    moveItem.mutate(
      { item, collectionId: target.id },
      {
        onSuccess: () => {
          closeDialog()
          showToast(
            t('collection_detail_page.item_actions.move_modal.moved', { item: item.name, collection: target.name })
          )
        },
        onError: () => showToast(t('collection_detail_page.item_actions.move_modal.error'), { type: 'error' })
      }
    )
  }

  function confirmReset() {
    if (!sync?.entity) return
    resetItem.mutate(
      { item, entity: sync.entity },
      {
        onSuccess: () => {
          closeDialog()
          showToast(t('collection_detail_page.item_actions.reset_modal.done', { name: item.name }))
        },
        onError: () => showToast(t('collection_detail_page.item_actions.reset_modal.error'), { type: 'error' })
      }
    )
  }

  function confirmDelete() {
    deleteItem.mutate(item, {
      onSuccess: () => {
        closeDialog()
        showToast(t('collection_detail_page.item_actions.deleted', { name: item.name }))
      },
      onError: () => showToast(t('collection_detail_page.delete_item.error'), { type: 'error' })
    })
  }

  if (!canCopyUrn && !canPreview && !canEditDraft && !canRemove && !canDelete) return null

  return (
    <>
      <ActionsMenu label={t('collection_detail_page.row_actions')} variant="row" testId="item-actions">
        {canCopyUrn && (
          <ActionsMenuItem testId="item-copy-urn" onClick={() => void copyUrn()}>
            {t('collection_detail_page.item_actions.copy_urn')}
          </ActionsMenuItem>
        )}
        {canPreview && (
          <ActionsMenuItem testId="item-preview" onClick={openEditor}>
            {t('collection_detail_page.item_actions.preview')}
          </ActionsMenuItem>
        )}
        {canEditDraft && (
          <ActionsMenuItem testId="item-move" onClick={() => setDialog('move')}>
            {t('collection_detail_page.item_actions.move')}
          </ActionsMenuItem>
        )}
        {canEditPrice && (
          <ActionsMenuItem testId="item-edit-price" onClick={() => openSaleDialog('update-price')}>
            {t('collection_detail_page.item_actions.edit_price')}
          </ActionsMenuItem>
        )}
        {canRemove && (
          <ActionsMenuItem testId="item-remove-from-sale" onClick={() => openSaleDialog('remove-listing')}>
            {t('collection_detail_page.item_actions.remove_from_sale')}
          </ActionsMenuItem>
        )}
        {canReset && (
          <ActionsMenuItem testId="item-reset" onClick={() => setDialog('reset')}>
            {t('collection_detail_page.item_actions.reset')}
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

      {dialog === 'move' && (
        <MoveItemModal
          item={item}
          collections={drafts.data}
          isLoading={drafts.isFetching}
          isMoving={moveItem.isPending}
          onConfirm={confirmMove}
          onClose={closeDialog}
        />
      )}
      {dialog === 'reset' && (
        <ResetItemModal item={item} isResetting={resetItem.isPending} onConfirm={confirmReset} onClose={closeDialog} />
      )}
      {typeof dialog === 'object' && dialog?.kind === 'update-price' && (
        <UpdatePriceFlow
          item={item}
          collection={collection}
          listing={dialog.listing}
          session={dialog.session}
          onClose={closeDialog}
        />
      )}
      {typeof dialog === 'object' && dialog?.kind === 'remove-listing' && (
        <RemoveListingFlow
          item={item}
          collection={collection}
          listing={dialog.listing}
          session={dialog.session}
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

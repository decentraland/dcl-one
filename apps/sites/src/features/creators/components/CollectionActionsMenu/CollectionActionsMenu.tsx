import { useState } from 'react'
import { useTranslation } from '@/data/lib/creators/intl'
import { useDeleteCollection } from '@/features/creators/hooks/useCollection'
import { useMediaQuery } from '@/features/creators/hooks/useMediaQuery'
import { copyToClipboard } from '@/data/lib/creators/clipboard'
import { hasBeenApproved, isCollectionLocked, isLinkedCollection, type Collection } from '@/data/lib/creators/collections'
import { isCollectionOwner, type RoleKind } from '@/data/lib/creators/collectionRoles'
import { type ReviewParty } from '@/data/lib/creators/events'
import { openForumPost } from '@/data/lib/creators/forumPost'
import { openExternal } from '@/data/lib/creators/navigation'
import { shopCollectionUrl } from '@/data/lib/creators/shop'
import { useNotifications } from '@/data/lib/creators/notifications'
import { media } from '@/data/lib/creators/theme'
import { ActionsMenu, ActionsMenuDivider, ActionsMenuItem } from '@/features/creators/components/ActionsMenu'
import { JumpInIcon, OpenInNewIcon } from '@/features/creators/components/Icons'
import { CollectionActivityModal } from '@/features/creators/components/CollectionActivityModal'
import { type Item } from '@/data/lib/creators/items'
import { DeleteCollectionModal } from './DeleteCollectionModal'

type Props = {
  collection: Collection
  address: string
  /** Compact 32px trigger for table rows; the default is the page-header icon button. */
  variant?: 'header' | 'row'
  /** The owner-only role entries (collaborators / senders); off in list rows. */
  showRoles?: boolean
  label?: string
  /** Launches the collection in the desktop explorer; desktop-only, since phones would only reach the download page. */
  onPreviewInWorld?: () => void
  /** The collection's items, so the activity timeline can name them in validator findings. */
  items?: Item[]
  /** The review timeline entry: published collections, for the people who can read it (roles and the committee). */
  showActivity?: boolean
  /** The side the viewer writes on the timeline as; without it they only read. */
  commentAs?: ReviewParty
  /** Opens the Send Items flow; the header button covers this on desktop, so the item shows only when compact. */
  onSendItems?: () => void
  /** Opens the items in the editor; the header button covers this on desktop, so the item shows only when compact. */
  onPreviewItems?: () => void
  /** Opens the Create Discount flow; the header button covers this on desktop, so the item shows only when compact. */
  onCreateDiscount?: () => void
  /** The collection's forum post; the header button covers it on desktop, so the item shows only when compact. */
  forumLink?: string
  /** The menu is the forum post's only entry on every screen size, once reviews are discussed in the app. */
  forumInMenu?: boolean
  /** Opens the collaborators / senders list; without it the owner-only entries are not rendered. */
  onManageRoles?: (kind: RoleKind) => void
  /** Opens the transfer-ownership dialog; without it the owner-only entry is not rendered. */
  onTransferOwnership?: () => void
  /** Linked collections only: opens the URN dialog while the collection is unpublished. */
  onEditUrn?: () => void
  /** Linked collections only: re-points the collection to another NFT contract, published or not. */
  onEditContract?: () => void
  /** Linked collections only: re-uploads zips over the items they name. */
  onEditInBulk?: () => void
  onDeleted?: () => void
}

export function CollectionActionsMenu({
  collection,
  address,
  variant = 'header',
  showRoles = true,
  label,
  onPreviewInWorld,
  items,
  showActivity = false,
  commentAs,
  onSendItems,
  onPreviewItems,
  onCreateDiscount,
  forumLink,
  forumInMenu = false,
  onManageRoles,
  onTransferOwnership,
  onEditUrn,
  onEditContract,
  onEditInBulk,
  onDeleted
}: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const deleteCollection = useDeleteCollection(address)
  const [isDeleteOpen, setDeleteOpen] = useState(false)
  const [isActivityOpen, setActivityOpen] = useState(false)

  // Small screens are mostly a viewer: copying and the role lists stay, deleting is desktop-only.
  const compact = useMediaQuery(media.noActions)

  const isOnChain = collection.isPublished
  const isLinked = isLinkedCollection(collection)
  const isOwner = isCollectionOwner(collection, address)
  const shopUrl =
    hasBeenApproved(collection) && collection.contractAddress ? shopCollectionUrl(collection.contractAddress) : null
  // A locked draft has a publish transaction in flight: nothing can be done to it yet.
  const canDelete = !compact && !isOnChain && !isCollectionLocked(collection)
  // The explorer is desktop-only; phones would only reach the download page.
  const showPreviewInWorld = !compact && !!onPreviewInWorld
  // The header's Send Items button is desktop-only, so the menu carries the action on small screens.
  const showSend = compact && !!onSendItems
  // Same for the header's Open Editor button; on mobile the editor is a viewer, hence "Preview".
  const showPreview = compact && !!onPreviewItems
  const showForum = (compact || forumInMenu) && !!forumLink
  const showDiscount = compact && !!onCreateDiscount
  const showEditUrn = !compact && isLinked && !isOnChain && !!onEditUrn
  const showEditContract = !compact && isLinked && !!onEditContract
  const showEditInBulk = !compact && isLinked && !!onEditInBulk
  // A linked collection's URN is meaningful from day one (it names its third party); a standard one's only on chain.
  const showCopyUrn = isOnChain || isLinked

  const hasLeadingActions = showPreviewInWorld || showSend || showPreview || showDiscount || showForum

  if (!showCopyUrn && !canDelete && !hasLeadingActions && !showEditUrn && !showEditContract && !showEditInBulk) {
    return null
  }

  async function copy(text: string | undefined, successKey: string) {
    const copied = !!text && (await copyToClipboard(text))
    if (copied) showToast(t(successKey))
    else showToast(t('collection_detail_page.actions.copy_failed'), { type: 'error' })
  }

  function confirmDelete() {
    deleteCollection.mutate(collection.id, {
      onSuccess: () => {
        setDeleteOpen(false)
        showToast(t('collection_detail_page.actions.deleted', { name: collection.name }))
        onDeleted?.()
      },
      onError: () => showToast(t('collection_detail_page.actions.delete_error'), { type: 'error' })
    })
  }

  return (
    <>
      <ActionsMenu
        label={label ?? t('collection_detail_page.more_actions')}
        variant={variant}
        testId="collection-actions"
      >
        {showPreviewInWorld && (
          <ActionsMenuItem testId="preview-in-world-action" onClick={onPreviewInWorld}>
            {t('preview_collection_button.label')}
            <JumpInIcon />
          </ActionsMenuItem>
        )}
        {showPreview && (
          <ActionsMenuItem testId="preview-items-action" onClick={onPreviewItems}>
            {t('collection_detail_page.actions.preview_items')}
          </ActionsMenuItem>
        )}
        {showSend && (
          <ActionsMenuItem testId="send-items-action" onClick={onSendItems}>
            {t('collection_detail_page.send_items')}
          </ActionsMenuItem>
        )}
        {showDiscount && (
          <ActionsMenuItem testId="create-discount-action" onClick={onCreateDiscount}>
            {t('collection_discount.create')}
          </ActionsMenuItem>
        )}
        {showForum && (
          <ActionsMenuItem
            testId="forum-post-action"
            onClick={() => openForumPost({ ...collection, forumLink }, 'detail_menu')}
          >
            {t('collection_detail_page.forum_post')}
            <OpenInNewIcon aria-hidden />
          </ActionsMenuItem>
        )}
        {showCopyUrn && (
          <>
            {hasLeadingActions && <ActionsMenuDivider />}
            <ActionsMenuItem
              testId="copy-urn"
              onClick={() => void copy(collection.urn, 'collection_detail_page.actions.copied_urn')}
            >
              {t('collection_detail_page.actions.copy_urn')}
            </ActionsMenuItem>
            {showEditUrn && (
              <ActionsMenuItem testId="edit-collection-urn" onClick={onEditUrn}>
                {t('linked_collection.actions.edit_urn')}
              </ActionsMenuItem>
            )}
            {showEditContract && (
              <ActionsMenuItem testId="edit-linked-contract" onClick={onEditContract}>
                {t('linked_collection.actions.edit_contract')}
              </ActionsMenuItem>
            )}
            {showEditInBulk && (
              <ActionsMenuItem testId="edit-in-bulk" onClick={onEditInBulk}>
                {t('linked_collection.actions.edit_in_bulk')}
              </ActionsMenuItem>
            )}
            {/* Linked collections have no contract of their own: their items live under the third party. */}
            {isOnChain && !isLinked && (
              <ActionsMenuItem
                testId="copy-address"
                onClick={() => void copy(collection.contractAddress, 'collection_detail_page.actions.copied_address')}
              >
                {t('collection_detail_page.actions.copy_address')}
              </ActionsMenuItem>
            )}
            {shopUrl && (
              <ActionsMenuItem testId="view-in-shop" onClick={() => openExternal(shopUrl)}>
                {t('collection_detail_page.actions.view_in_shop')}
                <OpenInNewIcon aria-hidden />
              </ActionsMenuItem>
            )}
            {showActivity && (
              <ActionsMenuItem testId="collection-activity" onClick={() => setActivityOpen(true)}>
                {t('collection_detail_page.actions.activity')}
              </ActionsMenuItem>
            )}
          </>
        )}
        {isOnChain && isOwner && showRoles && onManageRoles && (
          <>
            <ActionsMenuDivider />
            <ActionsMenuItem testId="manage-collaborators" onClick={() => onManageRoles('collaborators')}>
              {t('collection_detail_page.actions.collaborators')}
            </ActionsMenuItem>
            <ActionsMenuItem testId="manage-senders" onClick={() => onManageRoles('senders')}>
              {t('collection_detail_page.actions.senders')}
            </ActionsMenuItem>
          </>
        )}
        {isOnChain && !isLinked && isOwner && showRoles && onTransferOwnership && (
          <>
            <ActionsMenuDivider />
            <ActionsMenuItem testId="transfer-ownership" tone="danger" onClick={onTransferOwnership}>
              {t('collection_detail_page.actions.transfer_ownership')}
            </ActionsMenuItem>
          </>
        )}
        {canDelete && (
          <ActionsMenuItem testId="delete-collection" onClick={() => setDeleteOpen(true)}>
            {t('collection_detail_page.actions.delete')}
          </ActionsMenuItem>
        )}
      </ActionsMenu>

      {isActivityOpen && (
        <CollectionActivityModal
          collection={collection}
          address={address}
          items={items}
          commentAs={commentAs}
          onClose={() => setActivityOpen(false)}
        />
      )}
      {isDeleteOpen && (
        <DeleteCollectionModal
          name={collection.name}
          isDeleting={deleteCollection.isPending}
          onConfirm={confirmDelete}
          onClose={() => setDeleteOpen(false)}
        />
      )}
    </>
  )
}

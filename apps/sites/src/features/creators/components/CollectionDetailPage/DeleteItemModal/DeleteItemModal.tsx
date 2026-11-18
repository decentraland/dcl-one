import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'
import { type Item } from '~/lib/items'

type Props = {
  item: Item
  isDeleting: boolean
  onCancel: () => void
  onConfirm: () => void
}

/** "Delete this item?" — the confirm step before an item leaves the collection for good. */
export function DeleteItemModal({ item, isDeleting, onCancel, onConfirm }: Props) {
  const { t } = useTranslation()
  return (
    <ConfirmModal
      title={t('collection_detail_page.delete_item.title')}
      description={t('collection_detail_page.delete_item.description', { name: item.name })}
      busy={isDeleting}
      onClose={onCancel}
      cancel={{
        label: t('collection_detail_page.delete_item.cancel'),
        onClick: onCancel,
        testId: 'delete-item-cancel'
      }}
      confirm={{
        label: t('collection_detail_page.delete_item.confirm'),
        onClick: onConfirm,
        testId: 'delete-item-confirm'
      }}
      testId="delete-item-modal"
    />
  )
}

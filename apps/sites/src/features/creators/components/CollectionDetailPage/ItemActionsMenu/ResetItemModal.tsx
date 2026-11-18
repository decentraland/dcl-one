import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'
import { type Item } from '~/lib/items'

type Props = {
  item: Item
  isResetting: boolean
  onConfirm: () => void
  onClose: () => void
}

/** "Reset changes?" — restores the version of the item the committee approved. */
export function ResetItemModal({ item, isResetting, onConfirm, onClose }: Props) {
  const { t } = useTranslation()
  return (
    <ConfirmModal
      title={t('collection_detail_page.item_actions.reset_modal.title')}
      description={t('collection_detail_page.item_actions.reset_modal.description', { name: item.name })}
      busy={isResetting}
      onClose={onClose}
      cancel={{
        label: t('collection_detail_page.item_actions.reset_modal.cancel'),
        onClick: onClose,
        testId: 'reset-item-cancel'
      }}
      confirm={{
        label: t('collection_detail_page.item_actions.reset_modal.confirm'),
        onClick: onConfirm,
        testId: 'reset-item-confirm'
      }}
      testId="reset-item-modal"
    />
  )
}

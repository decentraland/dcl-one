import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'

type Props = {
  name: string
  isDeleting: boolean
  onConfirm: () => void
  onClose: () => void
}

export function DeleteCollectionModal({ name, isDeleting, onConfirm, onClose }: Props) {
  const { t } = useTranslation()
  return (
    <ConfirmModal
      title={t('collection_detail_page.actions.delete_modal.title')}
      description={t('collection_detail_page.actions.delete_modal.description', { name })}
      busy={isDeleting}
      onClose={onClose}
      cancel={{
        label: t('collection_detail_page.actions.delete_modal.cancel'),
        onClick: onClose,
        testId: 'delete-collection-cancel'
      }}
      confirm={{
        label: t('collection_detail_page.actions.delete_modal.confirm'),
        onClick: onConfirm,
        testId: 'delete-collection-confirm'
      }}
      testId="delete-collection-modal"
    />
  )
}

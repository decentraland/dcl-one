import { ConfirmModal } from '~/components/ConfirmModal'
import { useTranslation } from '~/intl'

type Props = {
  /** With reviewed items, offer to save them; with none, collapse to a simple discard confirm. */
  hasCheckedDrafts: boolean
  isSaving: boolean
  onLeave: () => void
  onSaveChanges: () => void
  onStay: () => void
}

export function LeaveConfirmModal({ hasCheckedDrafts, isSaving, onLeave, onSaveChanges, onStay }: Props) {
  const { t } = useTranslation()
  const variant = hasCheckedDrafts ? 'save' : 'discard'

  return (
    <ConfirmModal
      title={t(`add_items_modal.leave.${variant}_title`)}
      description={t(`add_items_modal.leave.${variant}_description`)}
      busy={isSaving}
      showClose={hasCheckedDrafts}
      onClose={onStay}
      cancel={{
        label: t('add_items_modal.leave.leave_without_saving'),
        onClick: onLeave,
        testId: 'leave-without-saving'
      }}
      confirm={
        hasCheckedDrafts
          ? { label: t('add_items_modal.leave.save_changes'), onClick: onSaveChanges, testId: 'save-changes' }
          : { label: t('add_items_modal.leave.keep_editing'), onClick: onStay, testId: 'keep-editing' }
      }
      testId="leave-confirm-modal"
    />
  )
}

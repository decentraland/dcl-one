import { useMemo, useState } from 'react'
import { ChevronRightIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { Button } from '~/components/Button'
import { Modal } from '~/components/Modal'
import { Select } from '~/components/Select'
import { useAssignCurator, useCuratorOptions } from '~/hooks/useCuration'
import { type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { useNotifications } from '~/lib/notifications'
import styles from './AssignCuratorModal.module.css'
import shared from '~/styles/shared.module.css'

const UNASSIGN = 'unassign'

type Props = {
  collection: Collection
  curation: CollectionCuration | null
  address: string
  /** `self` confirms taking the collection; `edit` picks any curator or none. */
  mode: 'self' | 'edit'
  onClose: () => void
  /** Taking the collection over right before deciding on it: continues into the decision instead of closing. */
  onAssigned?: () => void
}

export function AssignCuratorModal({ collection, curation, address, mode, onClose, onAssigned }: Props) {
  const { t } = useTranslation()
  const showToast = useNotifications(state => state.showToast)
  const assign = useAssignCurator(address)
  const self = address.toLowerCase()
  const [choice, setChoice] = useState<string>(curation?.assignee ?? self)

  const unassign = useMemo(() => ({ value: UNASSIGN, label: t('assign_curator_modal.unassign') }), [t])
  const options = useCuratorOptions(address, unassign)

  const assignee = mode === 'self' ? self : choice === UNASSIGN ? null : choice
  const unchanged = mode === 'edit' && assignee === (curation?.assignee ?? null)

  function submit(event: React.FormEvent) {
    event.preventDefault()
    if (unchanged) return onClose()
    assign.mutate(
      { collection, curation, assignee },
      {
        onSuccess: () => {
          const name = options.find(option => option.value === assignee)?.label ?? ''
          showToast(
            assignee
              ? t('assign_curator_modal.assigned', { collection: collection.name, name })
              : t('assign_curator_modal.unassigned', { collection: collection.name }),
            { type: 'success' }
          )
          if (onAssigned) onAssigned()
          else onClose()
        }
      }
    )
  }

  const current = curation?.assignee && curation.assignee !== self ? curation.assignee : null
  const currentName = useMemo(
    () => (current ? (options.find(option => option.value === current)?.label ?? current) : ''),
    [options, current]
  )

  return (
    <Modal
      title={t(`assign_curator_modal.title_${mode}`, { collection: collection.name })}
      onClose={onClose}
      closeDisabled={assign.isPending}
      testId="assign-curator-modal"
    >
      <form className={styles.form} onSubmit={submit}>
        {mode === 'self' ? (
          <p className={styles.text} data-testid="assign-curator-body">
            {current
              ? t('assign_curator_modal.take_over_body', { assignee: currentName })
              : t('assign_curator_modal.self_body')}
          </p>
        ) : (
          <Select
            value={choice}
            options={options}
            onChange={setChoice}
            ariaLabel={t('assign_curator_modal.curator')}
            testId="assign-curator-select"
          />
        )}
        {assign.isError && (
          <p className={styles.error} data-testid="assign-curator-error">
            {t('assign_curator_modal.error')}
          </p>
        )}
        <div className={shared.modalActions}>
          <Button type="button" variant="secondary" disabled={assign.isPending} onClick={onClose}>
            {t('assign_curator_modal.cancel')}
          </Button>
          <Button type="submit" loading={assign.isPending} data-testid="assign-curator-submit">
            {t(onAssigned ? 'assign_curator_modal.submit_continue' : `assign_curator_modal.submit_${mode}`)}
            {onAssigned && <ChevronRightIcon fontSize="small" />}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

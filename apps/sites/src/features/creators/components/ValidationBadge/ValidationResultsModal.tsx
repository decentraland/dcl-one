import { Modal } from '~/components/Modal'
import { useTranslation } from '~/intl'
import { type ValidationIssue } from '~/lib/validation'
import { ItemValidationCard, type ValidationSubject } from './ItemValidationCard'
import styles from './ValidationResultsModal.module.css'

/** One item's results in a list of several. */
export type ItemResult = { id: string; subject: ValidationSubject; issues: ValidationIssue[]; onSelect?: () => void }

type Props = {
  subject?: ValidationSubject
  issues: ValidationIssue[]
  /** Several items' results instead of one: replaces `subject` and `issues`. */
  results?: ItemResult[]
  onRerun?: () => Promise<unknown>
  onClose: () => void
  testId?: string
}

/** One item's validation results in a dialog, or several items' as a list. */
export function ValidationResultsModal({
  subject,
  issues,
  results,
  onRerun,
  onClose,
  testId = 'validation-results'
}: Props) {
  const { t } = useTranslation()
  return (
    <Modal title={t('item_editor.validation.modal_title')} onClose={onClose} compact testId={`${testId}-modal`}>
      {results ? (
        <div className={styles.list}>
          {results.map(result => (
            <ItemValidationCard
              key={result.id}
              subject={result.subject}
              issues={result.issues}
              onSelect={result.onSelect}
              testId={`${testId}-${result.id}`}
            />
          ))}
        </div>
      ) : (
        <ItemValidationCard subject={subject} issues={issues} onRerun={onRerun} testId={testId} />
      )}
    </Modal>
  )
}

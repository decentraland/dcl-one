import { useId, useState, type ReactNode } from 'react'
import { CheckboxLabel } from '../../../components/CheckboxLabel/CheckboxLabel'
import { Button } from '../../../components/Button/Button'
import { WebTextarea } from '../../../components/InputField/InputFieldWebControls'
import { Modal } from '../../../components/Modal/Modal'
import { adminCopy, REJECT_REASONS, rejectionNoteLimit, type RejectReasonId } from './admin-copy'
import { Notice } from './Notice'
import styles from './AdminModal.module.css'

export type RejectEventModalProps = {
  busy: boolean
  /** The outcome of a failed submission, shown inside the dialog. */
  feedback?: ReactNode
  onClose: () => void
  onSubmit: (payload: { reasons: RejectReasonId[]; notes: string }) => void
}

/**
 * decentraland.org's RejectEventModal: at least one reason, an optional note for the creator, Cancel
 * and Submit. The page builds the stored reason from both, so the note is limited to what fits beside
 * the chosen reasons' titles, with a counter; a note made too long by choosing more reasons blocks Submit.
 */
export function RejectEventModal({ busy, feedback, onClose, onSubmit }: RejectEventModalProps) {
  const notesId = useId()
  const counterId = useId()
  const [selected, setSelected] = useState<RejectReasonId[]>([])
  const [notes, setNotes] = useState('')
  const [missing, setMissing] = useState(false)
  const limit = rejectionNoteLimit(selected)
  const tooLong = notes.length > limit
  return (
    <Modal
      title={adminCopy.reject.title}
      closeButton={false}
      dismissible={!busy}
      onClose={onClose}
      divider
      actions={
        <>
          <Button size="lg" variant="secondary" disabled={busy} onClick={onClose}>
            {adminCopy.reject.cancel}
          </Button>
          <Button
            size="lg"
            disabled={busy || tooLong}
            onClick={() => {
              if (selected.length === 0) {
                setMissing(true)
                return
              }
              onSubmit({ reasons: selected, notes: notes.trim() })
            }}
          >
            {adminCopy.reject.submit}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <ul className={styles.reasons}>
          {REJECT_REASONS.map((reason) => (
            <li key={reason.id} className={styles.reason}>
              <CheckboxLabel
                label={
                  <>
                    <strong>{reason.title}</strong> – ({reason.description})
                  </>
                }
                checked={selected.includes(reason.id)}
                disabled={busy}
                onCheckedChange={(checked) => {
                  setMissing(false)
                  setSelected((current) =>
                    checked
                      ? [...current, reason.id]
                      : current.filter((value) => value !== reason.id)
                  )
                }}
              />
            </li>
          ))}
        </ul>
        {missing ? <Notice tone="error">{adminCopy.reject.required}</Notice> : null}
        <div className={styles.notes}>
          <label className={styles.notesLabel} htmlFor={notesId}>
            {adminCopy.reject.otherLabel}
          </label>
          <WebTextarea
            id={notesId}
            placeholder={adminCopy.reject.otherPlaceholder}
            maxLength={limit}
            rows={3}
            value={notes}
            disabled={busy}
            aria-describedby={counterId}
            aria-invalid={tooLong || undefined}
            onChange={(event) => setNotes(event.target.value)}
          />
          <p id={counterId} className={styles.notesCount} data-over={tooLong || undefined}>
            {adminCopy.reject.noteCount(notes.length, limit)}
          </p>
        </div>
        {tooLong ? <Notice tone="error">{adminCopy.reject.noteTooLong(limit)}</Notice> : null}
        {feedback}
      </div>
    </Modal>
  )
}

import { useId, useState } from 'react'
import { ChevronRightIcon } from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { NAME_ALREADY_IN_USE_ERROR, validateCollectionName } from '@/data/lib/creators/collections'
import { validateLinkedCollectionName } from '@/data/lib/creators/thirdParty'
import { EMAIL_MAX_LENGTH, isValidEmail } from '@/data/lib/creators/email'
import { Button } from '@/features/creators/components/Button'
import { CollectionNameInput } from '@/features/creators/components/CollectionNameInput'
import { Checkbox } from '@/features/creators/components/Checkbox'
import styles from './PublishCollectionModal.module.css'

type Props = {
  initialName: string
  /** The email already confirmed in this wizard, else the profile email; empty when there is neither. */
  initialEmail: string
  isSaving: boolean
  /** Raw error message from a failed rename, mapped to friendly copy here. */
  saveError: string | null
  /** Linked collections take builder-server's 42-character cap and no uniqueness hint. */
  nameMaxLength?: number
  /** A published linked collection can't be renamed: the field only confirms the name. */
  nameDisabled?: boolean
  onCancel: () => void
  onConfirm: (details: { name: string; email: string }) => void
}

/** Step 1: the creator fixes typos in the name, leaves a contact email and acknowledges the name is final. */
export function ConfirmNameStep({
  initialName,
  initialEmail,
  isSaving,
  saveError,
  nameMaxLength,
  nameDisabled = false,
  onCancel,
  onConfirm
}: Props) {
  const { t } = useTranslation()
  const emailErrorId = useId()
  const [name, setName] = useState(initialName)
  const [email, setEmail] = useState(initialEmail)
  const [emailBlurred, setEmailBlurred] = useState(false)
  const [accepted, setAccepted] = useState(false)
  const [editedSinceSubmit, setEditedSinceSubmit] = useState(false)

  const trimmed = name.trim()
  const validation = nameMaxLength ? validateLinkedCollectionName(trimmed) : validateCollectionName(trimmed)
  const localError = trimmed && validation ? t('publish_collection_modal.name_step.invalid_name') : null
  const serverError =
    saveError && !editedSinceSubmit
      ? saveError === NAME_ALREADY_IN_USE_ERROR
        ? t('publish_collection_modal.name_step.name_taken')
        : t('publish_collection_modal.name_step.save_error')
      : null
  const shownError = localError ?? serverError
  const trimmedEmail = email.trim()
  const emailValid = isValidEmail(trimmedEmail)
  // Judged once the creator leaves the field, so typing isn't interrupted mid-address.
  const showEmailError = emailBlurred && !!trimmedEmail && !emailValid
  const canContinue = accepted && emailValid && !validation && !isSaving && !serverError

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!canContinue) return
    setEditedSinceSubmit(false)
    onConfirm({ name: trimmed, email: trimmedEmail })
  }

  return (
    <form className={styles.step} onSubmit={handleSubmit} data-testid="publish-name-step">
      <h3 className={styles.heading}>{t('publish_collection_modal.name_step.title')}</h3>
      <p className={styles.text}>
        {t('publish_collection_modal.name_step.description')}
        <br />
        {t('publish_collection_modal.name_step.description_2')}
      </p>
      <div className={styles.inputWrapper}>
        <CollectionNameInput
          value={name}
          error={shownError}
          disabled={isSaving || nameDisabled}
          maxLength={nameMaxLength}
          hideHint={!!nameMaxLength}
          testId="publish-name"
          onChange={value => {
            setName(value)
            setEditedSinceSubmit(true)
          }}
        />
        <label className={styles.emailField}>
          {t('publish_collection_modal.name_step.email_label')}
          <input
            className={styles.emailInput}
            type="text"
            inputMode="email"
            spellCheck={false}
            autoComplete="email"
            maxLength={EMAIL_MAX_LENGTH}
            aria-required
            value={email}
            placeholder={t('publish_collection_modal.name_step.email_placeholder')}
            disabled={isSaving}
            aria-invalid={showEmailError}
            aria-describedby={showEmailError ? emailErrorId : undefined}
            data-invalid={showEmailError ? true : undefined}
            data-testid="publish-email-input"
            onChange={event => setEmail(event.target.value)}
            onFocus={() => setEmailBlurred(false)}
            onBlur={() => setEmailBlurred(true)}
          />
          {showEmailError && (
            <p className={styles.errorText} id={emailErrorId} role="alert" data-testid="publish-email-error">
              {t('publish_collection_modal.name_step.invalid_email')}
            </p>
          )}
        </label>
      </div>
      <Checkbox checked={accepted} disabled={isSaving} onChange={setAccepted} testId="publish-name-accept">
        {t('publish_collection_modal.name_step.checkbox', { name: trimmed || initialName })}
      </Checkbox>
      <div className={styles.footer}>
        <Button type="button" variant="secondary" disabled={isSaving} data-testid="publish-cancel" onClick={onCancel}>
          {t('publish_collection_modal.cancel')}
        </Button>
        <Button type="submit" loading={isSaving} disabled={!canContinue} data-testid="publish-name-confirm">
          {t('publish_collection_modal.name_step.confirm')}
          <ChevronRightIcon fontSize="small" />
        </Button>
      </div>
    </form>
  )
}

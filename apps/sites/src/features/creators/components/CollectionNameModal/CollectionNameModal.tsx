import { useState } from 'react'
import { useTranslation } from '~/intl'
import { NAME_ALREADY_IN_USE_ERROR, validateCollectionName } from '~/lib/collections'
import { validateLinkedCollectionName } from '~/lib/thirdParty'
import { Modal } from '~/components/Modal'
import { Button } from '~/components/Button'
import { CollectionNameInput } from '~/components/CollectionNameInput'
import styles from './CollectionNameModal.module.css'
import shared from '~/styles/shared.module.css'

type Props = {
  variant: 'create' | 'rename'
  initialName?: string
  /** Linked collections take builder-server's 42-character cap and need no uniqueness hint. */
  nameMaxLength?: number
  isPending: boolean
  /** Raw error message from the failed save, mapped to friendly copy here. */
  error: string | null
  onSubmit: (name: string) => void
  onClose: () => void
}

/** The New Collection / Rename Collection dialog: one validated name field. */
export function CollectionNameModal({
  variant,
  initialName = '',
  nameMaxLength,
  isPending,
  error,
  onSubmit,
  onClose
}: Props) {
  const { t } = useTranslation()
  const [name, setName] = useState(initialName)
  const [localError, setLocalError] = useState<string | null>(null)
  // Hides a stale server error once the user starts typing a different name.
  const [editedSinceSubmit, setEditedSinceSubmit] = useState(false)

  const trimmed = name.trim()

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault()
    if (!trimmed || isPending) return
    if (nameMaxLength ? validateLinkedCollectionName(trimmed) : validateCollectionName(trimmed)) {
      // Only invalid_character can survive here: empty is blocked above, length by maxLength.
      setLocalError(t('collection_name_modal.error_invalid_character'))
      return
    }
    setLocalError(null)
    setEditedSinceSubmit(false)
    onSubmit(trimmed)
  }

  const serverError =
    error && !editedSinceSubmit
      ? error === NAME_ALREADY_IN_USE_ERROR
        ? t('collection_name_modal.error_name_taken')
        : t('collection_name_modal.error_generic', { variant })
      : null
  const shownError = localError ?? serverError

  return (
    <Modal
      title={t('collection_name_modal.title', { variant })}
      onClose={onClose}
      closeDisabled={isPending}
      testId="collection-name-modal"
    >
      <form className={styles.form} onSubmit={handleSubmit}>
        <div className={styles.intro}>
          <h3 className={styles.heading}>{t('collection_name_modal.heading', { variant })}</h3>
          <p className={styles.subtitle}>{t('collection_name_modal.subtitle')}</p>
        </div>
        <CollectionNameInput
          value={name}
          error={shownError}
          placeholder={t('collection_name_modal.name_placeholder')}
          maxLength={nameMaxLength}
          hideHint={!!nameMaxLength}
          autoFocus
          testId="collection-name"
          onChange={value => {
            setName(value)
            setLocalError(null)
            setEditedSinceSubmit(true)
          }}
        />
        <div className={shared.modalActions}>
          <Button
            type="button"
            variant="secondary"
            data-testid="collection-name-cancel"
            disabled={isPending}
            onClick={onClose}
          >
            {t('collection_name_modal.cancel')}
          </Button>
          <Button type="submit" data-testid="collection-name-submit" loading={isPending} disabled={!trimmed}>
            {t('collection_name_modal.submit', { variant })}
          </Button>
        </div>
      </form>
    </Modal>
  )
}

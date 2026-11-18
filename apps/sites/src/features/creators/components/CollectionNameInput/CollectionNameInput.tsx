import { useId } from 'react'
import { InfoOutlinedIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { COLLECTION_NAME_MAX_LENGTH } from '~/lib/collections'
import styles from './CollectionNameInput.module.css'

type Props = {
  value: string
  onChange: (value: string) => void
  /** Friendly error copy; replaces the uniqueness hint while set. */
  error?: string | null
  disabled?: boolean
  autoFocus?: boolean
  placeholder?: string
  /** Standard collections keep the 32-character cap; linked ones take builder-server's 42. */
  maxLength?: number
  /** Drops the uniqueness hint, which only standard collection names are held to. */
  hideHint?: boolean
  /** Prefix for the `-input`, `-count`, `-error` and `-hint` test ids. */
  testId: string
}

/** Labeled, length-capped collection name field with the uniqueness hint or an error below it. */
export function CollectionNameInput({
  value,
  onChange,
  error,
  disabled,
  autoFocus,
  placeholder,
  maxLength = COLLECTION_NAME_MAX_LENGTH,
  hideHint = false,
  testId
}: Props) {
  const { t } = useTranslation()
  const errorId = useId()
  return (
    <div className={styles.field}>
      <label className={styles.label}>
        {t('collection_name_input.label')}
        <div className={styles.box} data-invalid={error ? true : undefined}>
          <input
            value={value}
            placeholder={placeholder}
            maxLength={maxLength}
            disabled={disabled}
            autoFocus={autoFocus}
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? errorId : undefined}
            data-testid={`${testId}-input`}
            onChange={event => onChange(event.target.value)}
          />
          <span className={styles.charCount} data-testid={`${testId}-count`}>
            {t('collection_name_input.char_count', { count: value.length, max: maxLength })}
          </span>
        </div>
      </label>
      {error ? (
        <p className={styles.errorText} id={errorId} data-testid={`${testId}-error`}>
          {error}
        </p>
      ) : hideHint ? null : (
        <p className={styles.hintText} data-testid={`${testId}-hint`}>
          <InfoOutlinedIcon fontSize="inherit" aria-hidden />
          {t('collection_name_input.unique_hint')}
        </p>
      )}
    </div>
  )
}

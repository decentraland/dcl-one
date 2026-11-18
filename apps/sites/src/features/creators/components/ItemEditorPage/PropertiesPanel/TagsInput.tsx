import { useState, type KeyboardEvent, type RefObject } from 'react'
import { CloseIcon as RemoveIcon } from '@/features/creators/components/Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import styles from '../ItemEditorPage.module.css'

type Props = {
  tags: string[]
  disabled?: boolean
  onChange: (tags: string[]) => void
  /** The text field, for callers that add a tag from elsewhere and want focus to land here afterwards. */
  inputRef?: RefObject<HTMLInputElement>
  testId?: string
}

/**
 * Chip list plus a text field: Enter or comma adds a tag, Backspace on an empty field removes the last.
 * Disabled, it is a plain read-only list.
 */
export function TagsInput({ tags, disabled = false, onChange, inputRef, testId = 'tags-input' }: Props) {
  const { t } = useTranslation()
  const [value, setValue] = useState('')

  function commit() {
    const next = value.trim()
    setValue('')
    if (next) onChange([...tags, next])
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault()
      commit()
    } else if (event.key === 'Backspace' && value === '' && tags.length > 0) {
      onChange(tags.slice(0, -1))
    }
  }

  return (
    <div className={styles.field}>
      <div className={styles.chips} data-testid={testId}>
        {tags.map(tag => (
          <span
            className={styles.tagChip}
            key={tag}
            data-testid={`${testId}-tag`}
            data-readonly={disabled || undefined}
          >
            {tag}
            {!disabled && (
              <button
                type="button"
                aria-label={t('item_editor.tags.remove', { tag })}
                data-testid={`${testId}-remove-${tag}`}
                onClick={() => onChange(tags.filter(candidate => candidate !== tag))}
              >
                <RemoveIcon style={{ fontSize: 14 }} />
              </button>
            )}
          </span>
        ))}
      </div>
      {disabled && tags.length === 0 && (
        <p className={styles.sectionNote} data-testid={`${testId}-empty`}>
          {t('item_editor.tags.empty')}
        </p>
      )}
      {!disabled && (
        <input
          className={styles.textInput}
          ref={inputRef}
          value={value}
          placeholder={t('item_editor.tags.placeholder')}
          maxLength={32}
          aria-label={t('item_editor.tags.title')}
          data-testid={`${testId}-field`}
          onChange={event => setValue(event.target.value)}
          onKeyDown={onKeyDown}
          onBlur={commit}
        />
      )}
    </div>
  )
}

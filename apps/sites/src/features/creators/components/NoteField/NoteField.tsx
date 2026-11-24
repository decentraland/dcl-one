import { useId } from 'react'
import styles from './NoteField.module.css'

type Props = {
  label: string
  value: string
  onChange: (value: string) => void
  maxLength: number
  placeholder?: string
  disabled?: boolean
  /** For a caller that shows the length itself. */
  hideCount?: boolean
  /** Keeps the box at its height, without the drag handle. */
  fixedSize?: boolean
  testId?: string
}

/** A labelled free-text box with its length counter, for the messages creators and curators send each other. */
export function NoteField({
  label,
  value,
  onChange,
  maxLength,
  placeholder,
  disabled,
  hideCount,
  fixedSize,
  testId
}: Props) {
  const id = useId()
  return (
    <div className={styles.field}>
      <label htmlFor={id}>{label}</label>
      <textarea
        className={styles.textArea}
        id={id}
        value={value}
        maxLength={maxLength}
        placeholder={placeholder}
        disabled={disabled}
        data-fixed={fixedSize ? '' : undefined}
        data-testid={testId}
        onChange={event => onChange(event.target.value)}
      />
      {!hideCount && (
        <span className={styles.charCount}>
          {value.length}/{maxLength}
        </span>
      )}
    </div>
  )
}

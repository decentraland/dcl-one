import { useEffect, useId, useRef, type HTMLInputTypeAttribute, type ReactNode } from 'react'
import { Spinner } from '../Spinner/Spinner'
import styles from './SubmitField.module.css'

export type SubmitFieldProps = {
  value: string
  onChange: (value: string) => void
  /** Called by the button and by Enter, unless `canSubmit` is false or the field is busy or disabled. */
  onSubmit: () => void
  /** The button's text. */
  submitLabel: string
  /** Holds the button and Enter, for example while the value is not a valid address. */
  canSubmit: boolean
  /** Disables the field and draws a spinner in the button's place. */
  busy?: boolean
  /** Shown under the field as an alert that describes it. The ring turns red once the field loses the focus. */
  error?: string | null
  /** Drawn before the error text. */
  errorIcon?: ReactNode
  /** Drawn at the field's left edge, in the placeholder's colour. */
  icon?: ReactNode
  placeholder?: string
  type?: HTMLInputTypeAttribute
  autoComplete?: string
  maxLength?: number
  /** Focuses the field when it mounts. */
  autoFocus?: boolean
  disabled?: boolean
  'aria-label': string
}

/**
 * A light text field with its submit button inside its right edge, as an email box and its NEXT
 * button: a ring outside it while it has the focus, and an error under it.
 */
export function SubmitField({
  value,
  onChange,
  onSubmit,
  submitLabel,
  canSubmit,
  busy = false,
  error = null,
  errorIcon,
  icon,
  placeholder,
  type = 'text',
  autoComplete,
  maxLength,
  autoFocus = false,
  disabled = false,
  'aria-label': label
}: SubmitFieldProps) {
  const input = useRef<HTMLInputElement>(null)
  const errorId = useId()
  useEffect(() => {
    if (autoFocus) input.current?.focus()
  }, [autoFocus])
  const submit = () => {
    if (canSubmit && !busy && !disabled) onSubmit()
  }
  return (
    <div className={styles.root}>
      <div className={styles.field} data-invalid={error != null || undefined}>
        {icon != null && <span className={styles.icon}>{icon}</span>}
        <input
          ref={input}
          className={styles.input}
          type={type}
          value={value}
          placeholder={placeholder}
          autoComplete={autoComplete}
          maxLength={maxLength}
          disabled={disabled || busy}
          aria-label={label}
          aria-invalid={error != null || undefined}
          aria-describedby={error != null ? errorId : undefined}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') submit()
          }}
        />
        {busy ? (
          <span className={styles.busy}>
            <Spinner size={24} variant="chase" />
          </span>
        ) : (
          <button
            type="button"
            className={styles.submit}
            disabled={!canSubmit || disabled}
            onClick={submit}
          >
            {submitLabel}
          </button>
        )}
      </div>
      {error != null && (
        <p id={errorId} className={styles.error} role="alert">
          {errorIcon}
          {error}
        </p>
      )}
    </div>
  )
}

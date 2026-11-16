// SubmitField — a light field with its submit button inside its right edge (the sign-in screen's email
// box and NEXT): an icon on its left, a ring outside it while focused, and an error under it that turns
// the ring red. Enter submits too. While `busy` a spinner takes the button's place.
import { useEffect, useRef } from 'react'
import { MaskIcon } from './MaskIcon'
import { Spinner } from './Spinner'
import styles from './SubmitField.module.css'

interface SubmitFieldProps {
  value: string
  onChange: (value: string) => void
  onSubmit: () => void
  /** The button's label (NEXT). */
  submitLabel: string
  /** Holds the button (an address that isn't one yet); Enter does nothing either. */
  canSubmit: boolean
  busy?: boolean
  /** Shown under the field, with the ring red, after `errorIcon` when given. */
  error?: string | null
  errorIcon?: string
  /** A white-on-transparent image, tinted like the placeholder. */
  icon?: string
  placeholder?: string
  type?: React.HTMLInputTypeAttribute
  autoComplete?: string
  maxLength?: number
  autoFocus?: boolean
  disabled?: boolean
  'aria-label': string
}

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
}: SubmitFieldProps): React.JSX.Element {
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (autoFocus) input.current?.focus()
  }, [autoFocus])
  const submit = (): void => {
    if (canSubmit && !busy && !disabled) onSubmit()
  }
  return (
    <div className={styles.wrap}>
      <div className={`${styles.field} ${error != null ? styles.invalid : ''}`}>
        {icon != null && <MaskIcon src={icon} size={24} className={styles.icon} />}
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
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') submit()
          }}
        />
        {busy ? (
          <span className={styles.busy}>
            <Spinner size={24} variant="chase" />
          </span>
        ) : (
          <button type="button" className={styles.submit} disabled={!canSubmit || disabled} onClick={submit}>
            {submitLabel}
          </button>
        )}
      </div>
      {error != null && (
        <p className={styles.error} role="alert">
          {errorIcon != null && <MaskIcon src={errorIcon} size={15} />}
          {error}
        </p>
      )}
    </div>
  )
}

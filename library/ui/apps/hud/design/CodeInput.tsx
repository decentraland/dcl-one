// CodeInput — a one-time code typed into a row of slots, one digit each (the email sign-in's code).
// One real input sits over the row, so typing, deleting, pasting and the browser's one-time-code
// autofill all work as in any field; the slots only draw what it holds, with the next one to fill
// outlined and its caret blinking. `state` colours every slot: accepted, or refused (the row shakes).
import { useEffect, useRef } from 'react'
import styles from './CodeInput.module.css'

interface CodeInputProps {
  value: string
  onChange: (value: string) => void
  /** Called once all slots are filled. */
  onComplete?: (code: string) => void
  length?: number
  state?: 'normal' | 'success' | 'error'
  disabled?: boolean
  autoFocus?: boolean
  'aria-label'?: string
}

export function CodeInput({
  value,
  onChange,
  onComplete,
  length = 6,
  state = 'normal',
  disabled = false,
  autoFocus = false,
  'aria-label': label = 'Verification code'
}: CodeInputProps): React.JSX.Element {
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (autoFocus && !disabled) input.current?.focus()
  }, [autoFocus, disabled])
  const active = Math.min(value.length, length - 1)
  return (
    <div className={`${styles.row} ${styles[state]}`}>
      {Array.from({ length }, (_, i) => (
        <span key={i} className={`${styles.slot} ${i === active && !disabled ? styles.current : ''}`} aria-hidden="true">
          {value[i] ?? ''}
          {i === value.length && !disabled && <span className={styles.caret} />}
        </span>
      ))}
      <input
        ref={input}
        className={styles.input}
        value={value}
        disabled={disabled}
        inputMode="numeric"
        autoComplete="one-time-code"
        aria-label={label}
        aria-invalid={state === 'error' || undefined}
        onChange={(e) => {
          const next = e.target.value.replace(/\D/g, '').slice(0, length)
          onChange(next)
          if (next.length === length) onComplete?.(next)
        }}
      />
    </div>
  )
}

import { useEffect, useRef } from 'react'
import styles from './CodeInput.module.css'

export type CodeInputProps = {
  value: string
  /** Called with the digits typed so far, at most `length` of them. */
  onChange: (value: string) => void
  /** Called once every slot is filled. */
  onComplete?: (code: string) => void
  /** How many slots. Six by default. */
  length?: number
  /** `success` and `error` colour every slot; the row shakes on `error`. */
  state?: 'normal' | 'success' | 'error'
  disabled?: boolean
  /** Focuses the field when it mounts, and when it stops being disabled. */
  autoFocus?: boolean
  'aria-label'?: string
}

/**
 * A one-time code typed into a row of slots, one digit each. One real input sits over the row, so
 * typing, deleting, pasting and the browser's one-time-code autofill work as in any field; the slots
 * draw what it holds, with the next one to fill outlined and its caret blinking.
 */
export function CodeInput({
  value,
  onChange,
  onComplete,
  length = 6,
  state = 'normal',
  disabled = false,
  autoFocus = false,
  'aria-label': label = 'Verification code'
}: CodeInputProps) {
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (autoFocus && !disabled) input.current?.focus()
  }, [autoFocus, disabled])
  const active = Math.min(value.length, length - 1)
  return (
    <div className={styles.root} data-state={state}>
      {Array.from({ length }, (_, index) => (
        <span
          key={index}
          className={styles.slot}
          data-current={(index === active && !disabled) || undefined}
          aria-hidden="true"
        >
          {value[index] ?? ''}
          {index === value.length && !disabled && <span className={styles.caret} />}
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
        onChange={(event) => {
          const next = event.target.value.replace(/\D/g, '').slice(0, length)
          onChange(next)
          if (next.length === length) onComplete?.(next)
        }}
      />
    </div>
  )
}

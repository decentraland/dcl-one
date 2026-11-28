import { hasRenderableContent } from '../../content.helpers'
import { PasswordVisibilityIcon, InputErrorIcon } from './InputField.icons'
import {
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
  type InputHTMLAttributes,
  type ReactNode,
  type Ref
} from 'react'
import styles from './InputField.module.css'

export type InputFieldProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'size' | 'children' | 'dangerouslySetInnerHTML'
> & {
  /** The visible name above the field. A node lets a form put its own content in it, such as a required mark. */
  label: ReactNode
  ref?: Ref<HTMLInputElement>
  labelHidden?: boolean
  hint?: string
  hintIcon?: ReactNode
  action?: ReactNode
  description?: string
  error?: string
  counter?: string
  suffix?: string
  leadingIcon?: ReactNode
  trailingIcon?: ReactNode
  allowPasswordReveal?: boolean
  revealLabel?: string
  hideLabel?: string
}

export function InputField({
  label,
  ref: forwardedRef,
  labelHidden = false,
  hint,
  hintIcon,
  action,
  description,
  error,
  counter,
  suffix,
  leadingIcon,
  trailingIcon,
  allowPasswordReveal = false,
  revealLabel = 'Show password',
  hideLabel = 'Hide password',
  type = 'text',
  id,
  className,
  'aria-describedby': describedBy,
  ...input
}: InputFieldProps & { surface: 'explorer' | 'web' }) {
  const generatedId = useId()
  const inputId = id ?? generatedId
  const canReveal = type === 'password' && allowPasswordReveal && !input.disabled
  const [visibility, setVisibility] = useState({ eligible: canReveal, revealed: false })
  // Revoke visibility in this render instead of waiting for a post-paint effect.
  if (visibility.eligible !== canReveal) {
    setVisibility({ eligible: canReveal, revealed: false })
  }
  const revealed = canReveal && visibility.eligible && visibility.revealed
  const inputRef = useRef<HTMLInputElement>(null)
  useImperativeHandle(forwardedRef, () => inputRef.current!, [])
  const { surface, ...nativeInput } = input
  useEffect(() => {
    const form = inputRef.current?.form
    if (!form || !allowPasswordReveal || type !== 'password') return
    let active = true
    const completedDispatch = new MessageChannel()
    const pendingResets: Event[] = []
    completedDispatch.port1.onmessage = () => {
      if (!active) return
      const accepted = pendingResets.splice(0).some((event) => !event.defaultPrevented)
      if (accepted) setVisibility((current) => ({ ...current, revealed: false }))
    }
    const reset = (event: Event) => {
      // A task observes cancellation after native and React reset owners have run.
      pendingResets.push(event)
      completedDispatch.port2.postMessage(null)
    }
    form.addEventListener('reset', reset)
    return () => {
      active = false
      pendingResets.length = 0
      completedDispatch.port1.onmessage = null
      completedDispatch.port1.close()
      completedDispatch.port2.close()
      form.removeEventListener('reset', reset)
    }
  }, [allowPasswordReveal, type, nativeInput.form])
  const sanitized = nativeInput as InputHTMLAttributes<HTMLInputElement>
  delete sanitized.children
  delete sanitized.dangerouslySetInnerHTML
  const { descriptionId, errorId, counterId, hintId } = fieldMessageIds(inputId, {
    description,
    error,
    counter,
    hint
  })
  const suffixId = suffix?.trim() ? `${inputId}-suffix` : undefined
  const ids =
    [describedBy, descriptionId, errorId, suffixId, counterId, hintId].filter(Boolean).join(' ') ||
    undefined
  return (
    <FieldChrome
      surface={surface}
      inputId={inputId}
      label={label}
      labelHidden={surface === 'explorer' || labelHidden}
      description={description}
      error={error}
      counter={counter}
      hint={hint}
      hintIcon={hintIcon}
      className={className}
    >
      <div className={styles.fieldRow}>
        <div
          className={surface === 'web' ? `${styles.field} ${styles.webField}` : styles.field}
          data-invalid={!!errorId || undefined}
        >
          {hasRenderableContent(leadingIcon) ? (
            <span className={styles.icon} aria-hidden="true">
              {leadingIcon}
            </span>
          ) : null}
          <input
            {...sanitized}
            ref={inputRef}
            id={inputId}
            type={revealed ? 'text' : type}
            aria-invalid={errorId ? true : nativeInput['aria-invalid']}
            aria-describedby={ids}
            className={styles.input}
          />
          {suffixId ? (
            <span id={suffixId} className={styles.suffix}>
              {suffix}
            </span>
          ) : null}
          {hasRenderableContent(trailingIcon) ? (
            <span className={styles.icon} aria-hidden="true">
              {trailingIcon}
            </span>
          ) : null}
          {type === 'password' && allowPasswordReveal ? (
            <button
              type="button"
              className={styles.reveal}
              aria-label={revealed ? hideLabel : revealLabel}
              disabled={nativeInput.disabled}
              onClick={() =>
                setVisibility((current) => ({ ...current, revealed: !current.revealed }))
              }
            >
              <PasswordVisibilityIcon revealed={revealed} />
            </button>
          ) : null}
        </div>
        {hasRenderableContent(action) ? <div className={styles.action}>{action}</div> : null}
      </div>
    </FieldChrome>
  )
}

type FieldMessages = { description?: string; error?: string; counter?: string; hint?: string }

/** The ids of a field's description and status lines, set only for the lines that have text. */
export function fieldMessageIds(inputId: string, messages: FieldMessages) {
  const id = (part: keyof FieldMessages) =>
    messages[part]?.trim() ? `${inputId}-${part}` : undefined
  return {
    descriptionId: id('description'),
    errorId: id('error'),
    counterId: id('counter'),
    hintId: id('hint')
  }
}

/**
 * The frame every web and Explorer field shares: its label, description and status lines around the
 * control the caller passes as children.
 */
export function FieldChrome({
  surface,
  inputId,
  label,
  labelHidden,
  description,
  error,
  counter,
  hint,
  hintIcon,
  className,
  children
}: FieldMessages & {
  surface: 'explorer' | 'web'
  inputId: string
  label?: ReactNode
  labelHidden?: boolean
  hintIcon?: ReactNode
  className?: string
  children: ReactNode
}) {
  const { descriptionId, errorId, counterId, hintId } = fieldMessageIds(inputId, {
    description,
    error,
    counter,
    hint
  })
  return (
    <div className={[styles.root, className].filter(Boolean).join(' ')} data-surface={surface}>
      {label != null && typeof label !== 'boolean' ? (
        <label className={styles.label} htmlFor={inputId} data-hidden={labelHidden || undefined}>
          {label}
        </label>
      ) : null}
      {descriptionId ? (
        <div id={descriptionId} className={styles.description}>
          {description}
        </div>
      ) : null}
      {children}
      {errorId || counterId || hintId ? (
        <div className={styles.status}>
          {counterId ? <span id={counterId}>{counter}</span> : null}
          {errorId ? (
            <div className={styles.errorRow}>
              <InputErrorIcon surface={surface} />
              <div id={errorId} className={styles.error}>
                {error}
              </div>
            </div>
          ) : null}
          {hintId ? (
            <div className={styles.hint}>
              {hasRenderableContent(hintIcon) ? (
                <span className={styles.icon} aria-hidden="true">
                  {hintIcon}
                </span>
              ) : null}
              <span id={hintId}>{hint}</span>
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

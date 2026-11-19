import { useImperativeHandle, useLayoutEffect, useRef } from 'react'
import type { ChangeEvent, InputHTMLAttributes, Ref } from 'react'
import { GeneralCheck, GeneralRemove } from '../../foundations/icons/general'
import styles from './Checkbox.module.css'

export type CheckboxSize = 'sm' | 'md' | 'lg'
export type CheckboxVariant = 'outline' | 'filled'

type NativeCheckboxProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'type' | 'role' | 'size' | 'children' | 'dangerouslySetInnerHTML' | 'aria-checked'
>

export type CheckboxControlProps = NativeCheckboxProps & {
  /** Box size: 16, 20 or 24px. The target is 28, 32 or 36px, and 44px on a coarse pointer. */
  size?: CheckboxSize
  /** `outline`: a bordered box that fills with the primary action colour. `filled`: a snow box. */
  variant?: CheckboxVariant
  /** Shows a dash and reports the mixed state. Clicking it hands the next state to the caller. */
  indeterminate?: boolean
  ref?: Ref<HTMLInputElement>
  /** Called with the next checked state after the native `onChange`. */
  onCheckedChange?: (checked: boolean) => void
}

/** A checkbox needs a name: `aria-label`, or `aria-labelledby` naming visible text. */
export type CheckboxProps = CheckboxControlProps &
  (
    | { 'aria-label': string; 'aria-labelledby'?: string }
    | { 'aria-label'?: string; 'aria-labelledby': string }
  )

/**
 * The box shared by Checkbox and CheckboxLabel. It is a native checkbox, so Space toggles it, a
 * form submits it, a disabled box is really disabled and `indeterminate` is the native mixed state.
 */
export function CheckboxControl({
  size = 'md',
  variant = 'outline',
  indeterminate = false,
  className,
  ref: forwardedRef,
  onChange,
  onCheckedChange,
  ...input
}: CheckboxControlProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  useImperativeHandle(forwardedRef, () => inputRef.current!, [])
  // `indeterminate` exists only as a DOM property, so it is applied after every render.
  useLayoutEffect(() => {
    if (inputRef.current) inputRef.current.indeterminate = indeterminate
  })
  const change = (event: ChangeEvent<HTMLInputElement>) => {
    const target = event.currentTarget
    // A click clears the DOM's mixed state. Restore the caller's before its callbacks run, so a
    // render they commit, even synchronously, has the last word.
    target.indeterminate = indeterminate
    onChange?.(event)
    onCheckedChange?.(target.checked)
  }
  return (
    <span
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-size={size}
      data-variant={variant}
    >
      <input
        {...input}
        ref={inputRef}
        type="checkbox"
        className={styles.input}
        // Without a handler React keeps warning about a `checked` box that cannot change.
        onChange={onChange || onCheckedChange || indeterminate ? change : undefined}
      />
      <span className={styles.halo} aria-hidden="true" />
      <span className={styles.box} aria-hidden="true">
        <GeneralCheck className={styles.check} />
        <GeneralRemove className={styles.dash} />
      </span>
    </span>
  )
}

/** A standalone checkbox with no visible label of its own. Use CheckboxLabel for one with text. */
export function Checkbox(props: CheckboxProps) {
  return <CheckboxControl {...props} />
}

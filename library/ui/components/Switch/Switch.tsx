import type { ChangeEvent, InputHTMLAttributes, Ref } from 'react'
import styles from './Switch.module.css'

export type SwitchProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'type' | 'role' | 'children' | 'dangerouslySetInnerHTML' | 'size'
> & {
  /** The accessible name; shown before the switch unless `labelHidden`. */
  label: string
  labelHidden?: boolean
  ref?: Ref<HTMLInputElement>
  /** Called with the next state after the native `onChange`. */
  onCheckedChange?: (checked: boolean) => void
}

/**
 * An on/off control: a native checkbox with the `switch` role, so Space toggles it, a form submits
 * it and a disabled switch is really disabled. Without `checked` it keeps its own state.
 */
export function Switch({
  label,
  labelHidden = false,
  className,
  onChange,
  onCheckedChange,
  ...input
}: SwitchProps) {
  const change = (event: ChangeEvent<HTMLInputElement>) => {
    onChange?.(event)
    onCheckedChange?.(event.currentTarget.checked)
  }
  return (
    <label
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-disabled={input.disabled || undefined}
    >
      <span className={labelHidden ? styles.hiddenLabel : styles.label}>{label}</span>
      <span className={styles.control}>
        <input {...input} type="checkbox" role="switch" className={styles.input} onChange={change} />
        <span className={styles.track} aria-hidden="true" />
      </span>
    </label>
  )
}

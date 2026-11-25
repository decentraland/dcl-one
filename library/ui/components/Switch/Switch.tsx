import type { ChangeEvent, InputHTMLAttributes, KeyboardEvent, Ref } from 'react'
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
  /**
   * `hud` is the Explorer HUD's switch: a ruby pill with a white knob and no minimum height. A caller sizes
   * and tints it with custom properties set on it or on an ancestor: `--toggle-w`, `--toggle-h`,
   * `--toggle-pad`, `--toggle-off`, `--toggle-on`, `--toggle-knob`, `--toggle-knob-on` and
   * `--toggle-knob-shadow`. A visible label takes the caller's type.
   */
  appearance?: 'default' | 'hud'
  /** For `hud` only (ignored otherwise): `sm` is the compact 39 × 24 switch of the HUD's popovers. */
  size?: 'md' | 'sm'
  /** For `hud` only (ignored otherwise): `light` is a switch on a light panel, whose off track is dark. */
  surface?: 'dark' | 'light'
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
  appearance = 'default',
  size = 'md',
  surface = 'dark',
  ...input
}: SwitchProps) {
  const change = (event: ChangeEvent<HTMLInputElement>) => {
    onChange?.(event)
    onCheckedChange?.(event.currentTarget.checked)
  }
  // A HUD switch toggles on Enter as well as Space, like the HUD's buttons; a native checkbox only takes Space.
  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    input.onKeyDown?.(event)
    if (appearance === 'hud' && event.key === 'Enter' && !event.defaultPrevented) event.currentTarget.click()
  }
  return (
    <label
      className={[
        styles.root,
        appearance === 'hud' && styles.hud,
        appearance === 'hud' && size === 'sm' && styles.sm,
        appearance === 'hud' && surface === 'light' && styles.onLight,
        className
      ]
        .filter(Boolean)
        .join(' ')}
      data-disabled={input.disabled || undefined}
    >
      <span className={labelHidden ? styles.hiddenLabel : styles.label}>{label}</span>
      <span className={styles.control}>
        <input {...input} type="checkbox" role="switch" className={styles.input} onChange={change} onKeyDown={keyDown} />
        <span className={styles.track} aria-hidden="true" />
      </span>
    </label>
  )
}

import type { InputHTMLAttributes, ReactNode } from 'react'
import styles from './SearchBar.module.css'

export type SearchBarProps = {
  label: string
  value: string
  onValueChange: (value: string) => void
  /** Shows a clear button while the field has a value. */
  onClear?: () => void
  clearLabel?: string
  /** Class of the field itself; `className` styles the box around it. */
  inputClassName?: string
  /** A visual before the field, such as a magnifier. The `hud` field sizes it: 15px, or 19 by 18px when light. */
  leading?: ReactNode
  /** What the clear button draws. Defaults to a multiplication sign. */
  clearIcon?: ReactNode
  /**
   * Not in Figma. `hud` is the Explorer HUD's field: a pill whose whole box focuses the input, in a `dark` or a
   * `light` `variant` and an `md` or a 36px `sm` `fieldSize`.
   */
  appearance?: 'default' | 'hud'
  /** `hud` only. `light` is an off-white field with dark ink. */
  variant?: 'dark' | 'light'
  /** `hud` only. `sm` is a 36px field as wide as its container. */
  fieldSize?: 'md' | 'sm'
} & Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'type' | 'aria-label'>

/** A search field with an optional clear button, as decentraland.org's blog search draws it. */
export function SearchBar({
  label,
  value,
  onValueChange,
  onClear,
  clearLabel = 'Clear search',
  className,
  inputClassName,
  leading,
  clearIcon = '×',
  appearance = 'default',
  variant = 'dark',
  fieldSize = 'md',
  ...input
}: SearchBarProps) {
  const hud = appearance === 'hud'
  const Root = hud ? 'label' : 'div'
  return (
    <Root
      className={[hud ? styles.hud : styles.root, className].filter(Boolean).join(' ')}
      data-variant={hud ? variant : undefined}
      data-size={hud ? fieldSize : undefined}
    >
      {leading != null && (
        <span className={hud ? styles.hudLeading : styles.leading} aria-hidden="true">
          {leading}
        </span>
      )}
      <input
        autoComplete="off"
        {...input}
        type="text"
        className={[hud ? styles.hudInput : styles.input, inputClassName].filter(Boolean).join(' ')}
        aria-label={label}
        value={value}
        onChange={(event) => onValueChange(event.target.value)}
      />
      {onClear && value && (
        <button
          type="button"
          className={hud ? styles.hudClear : styles.clear}
          onClick={onClear}
          aria-label={clearLabel}
        >
          {clearIcon}
        </button>
      )}
    </Root>
  )
}

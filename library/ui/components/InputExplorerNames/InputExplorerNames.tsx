import type { CSSProperties, InputHTMLAttributes, ReactNode, Ref } from 'react'
import { InputField, type InputFieldProps } from '../InputField/InputField'
import styles from './InputExplorerNames.module.css'

type FigmaProps = Omit<InputFieldProps, 'labelHidden'> & { appearance?: 'default' }

/**
 * Not in Figma: the Explorer HUD's plain field. It is the input alone, so the caller lays it out and names it
 * with a label element or `label`.
 */
type HudProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'size' | 'children' | 'dangerouslySetInnerHTML'
> & {
  appearance: 'hud'
  ref?: Ref<HTMLInputElement>
  /** The field's accessible name, when no label element gives it one. */
  label?: string
  /** `light` is the pearl field of a solid panel. */
  variant?: 'dark' | 'light'
  /** `lg` is display-sized text, for a field that is the point of its page. */
  fieldSize?: 'md' | 'lg'
  /** A ruby border and `aria-invalid`, for a value the reader still has to fix. */
  invalid?: boolean
  /**
   * An icon button inside the field's right edge. `disabled` holds it while the field stays editable; `size` is
   * the icon's, in px (18 by default), and the field grows to fit a bigger one.
   */
  fieldAction?: {
    icon: ReactNode
    label: string
    onClick: () => void
    disabled?: boolean
    size?: number
  }
}

export type InputExplorerNamesProps = FigmaProps | HudProps

function HudInput({
  label,
  variant = 'dark',
  fieldSize = 'md',
  invalid = false,
  fieldAction,
  className,
  ...input
}: Omit<HudProps, 'appearance'>) {
  const field = (
    <input
      aria-label={label || undefined}
      {...input}
      className={[
        styles.hud,
        variant === 'light' ? styles.hudLight : styles.hudDark,
        fieldSize === 'lg' && styles.hudLarge,
        invalid && styles.hudInvalid,
        fieldAction && styles.hudWithAction,
        className
      ]
        .filter(Boolean)
        .join(' ')}
      aria-invalid={invalid || undefined}
    />
  )
  if (!fieldAction) return field
  const iconSize = fieldAction.size ?? 18
  return (
    <div className={styles.hudWrap} style={{ '--action-icon': `${iconSize}px` } as CSSProperties}>
      {field}
      <button
        type="button"
        className={styles.hudAction}
        aria-label={fieldAction.label}
        title={fieldAction.label}
        onClick={fieldAction.onClick}
        disabled={input.disabled || fieldAction.disabled}
      >
        {fieldAction.icon}
      </button>
    </div>
  )
}

export function InputExplorerNames({ appearance = 'default', ...props }: InputExplorerNamesProps) {
  if (appearance === 'hud') return <HudInput {...(props as Omit<HudProps, 'appearance'>)} />
  return <InputField {...(props as Omit<FigmaProps, 'appearance'>)} surface="explorer" />
}

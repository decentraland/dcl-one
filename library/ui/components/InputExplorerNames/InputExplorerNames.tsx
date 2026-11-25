import type {
  CSSProperties,
  InputHTMLAttributes,
  ReactElement,
  ReactNode,
  Ref,
  TextareaHTMLAttributes
} from 'react'
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
  multiline?: false
  ref?: Ref<HTMLInputElement>
  /** The field's accessible name, when no label element gives it one. */
  label?: string
  /** `light` is the pearl field of a solid panel. */
  variant?: 'dark' | 'light'
  /** `lg` is display-sized text, for a field that is the point of its page. */
  fieldSize?: 'md' | 'lg'
  /** A red border and `aria-invalid`, for a value the reader still has to fix. */
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

/** The `hud` field over several lines: a textarea the reader can resize downwards. */
type HudMultilineProps = Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  'children' | 'dangerouslySetInnerHTML'
> & {
  appearance: 'hud'
  multiline: true
  ref?: Ref<HTMLTextAreaElement>
  /** The field's accessible name, when no label element gives it one. */
  label?: string
  variant?: 'dark' | 'light'
}

export type InputExplorerNamesProps = FigmaProps | HudProps | HudMultilineProps

function HudInput({
  label,
  variant = 'dark',
  fieldSize = 'md',
  invalid = false,
  fieldAction,
  className,
  ...input
}: Omit<HudProps, 'appearance' | 'multiline'>) {
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

function HudTextArea({
  label,
  variant = 'dark',
  rows = 4,
  className,
  ...textarea
}: Omit<HudMultilineProps, 'appearance' | 'multiline'>) {
  return (
    <textarea
      aria-label={label || undefined}
      {...textarea}
      rows={rows}
      className={[styles.hudArea, variant === 'light' ? styles.hudLight : styles.hudDark, className]
        .filter(Boolean)
        .join(' ')}
    />
  )
}

export function InputExplorerNames(props: HudMultilineProps): ReactElement
export function InputExplorerNames(props: HudProps): ReactElement
export function InputExplorerNames(props: FigmaProps): ReactElement
export function InputExplorerNames({ appearance = 'default', ...props }: InputExplorerNamesProps) {
  if (appearance === 'hud') {
    const { multiline = false, ...field } = props as Omit<HudProps | HudMultilineProps, 'appearance'>
    if (multiline)
      return <HudTextArea {...(field as Omit<HudMultilineProps, 'appearance' | 'multiline'>)} />
    return <HudInput {...(field as Omit<HudProps, 'appearance' | 'multiline'>)} />
  }
  return <InputField {...(props as Omit<FigmaProps, 'appearance'>)} surface="explorer" />
}

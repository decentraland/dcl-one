// TextInput — the shared InputExplorerNames in its HUD look: the plain single-line field (names, link titles,
// dates). `SearchField` covers search. Controlled or uncontrolled, like the other input primitives here.

import { useState } from 'react'
import { Icon, type IconName } from './icons'
import { InputExplorerNames } from '@dcl/one-ui'

export interface TextInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'value' | 'size'> {
  value?: string
  defaultValue?: string
  onChange?: (value: string) => void
  /** Light = the white field used on solid panels (Settings, the passport's edit mode). */
  variant?: 'dark' | 'light'
  /** lg: display-sized text, for a field that is the page's point (a name the player picks). */
  size?: 'md' | 'lg'
  /** Red border + `aria-invalid`, for a value the user still has to fix. */
  invalid?: boolean
  /** An icon action inside the field's right edge (e.g. a dice that picks another value); `disabled`
   *  holds it while the field stays editable; `size` is the icon's, in px (18 by default): the field
   *  grows to fit a bigger one. */
  action?: { icon: IconName; label: string; onClick: () => void; disabled?: boolean; size?: number }
}

export function TextInput({
  value,
  defaultValue = '',
  onChange,
  variant = 'dark',
  size = 'md',
  invalid = false,
  action,
  className,
  ...rest
}: TextInputProps): React.JSX.Element {
  const [internal, setInternal] = useState(defaultValue)
  const isControlled = value !== undefined
  const v = isControlled ? value : internal

  return (
    <InputExplorerNames
      appearance="hud"
      {...rest}
      className={className}
      variant={variant}
      fieldSize={size}
      invalid={invalid}
      fieldAction={
        action && {
          icon: <Icon name={action.icon} size={action.size ?? 18} />,
          label: action.label,
          onClick: action.onClick,
          disabled: action.disabled,
          size: action.size
        }
      }
      value={v}
      onChange={(e) => {
        if (!isControlled) setInternal(e.target.value)
        onChange?.(e.target.value)
      }}
    />
  )
}

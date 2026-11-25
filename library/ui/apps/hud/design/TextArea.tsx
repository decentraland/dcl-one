// TextArea — the shared InputExplorerNames in its HUD look, over several lines (the passport's About Me), with
// the kit's uncontrolled mode. Pairs with CharCounter when a `maxLength` is set: the counter is the only
// warning a user gets before the browser silently stops accepting characters. Its stylesheet only stacks the two.

import { useState } from 'react'
import { CharCounter } from './CharCounter'
import { InputExplorerNames } from '@dcl/one-ui'
import styles from './TextArea.module.css'

export interface TextAreaProps extends Omit<React.TextareaHTMLAttributes<HTMLTextAreaElement>, 'onChange' | 'value'> {
  value?: string
  defaultValue?: string
  onChange?: (value: string) => void
  variant?: 'dark' | 'light'
  /** Show a `current/max` counter under the field. Needs `maxLength` to have a max to count to. */
  counter?: boolean
}

export function TextArea({
  value,
  defaultValue = '',
  onChange,
  variant = 'dark',
  counter = false,
  className,
  rows = 4,
  ...rest
}: TextAreaProps): React.JSX.Element {
  const [internal, setInternal] = useState(defaultValue)
  const isControlled = value !== undefined
  const v = isControlled ? value : internal

  return (
    <div className={styles.wrap}>
      <InputExplorerNames
        appearance="hud"
        multiline
        {...rest}
        rows={rows}
        className={className}
        variant={variant}
        value={v}
        onChange={(e) => {
          if (!isControlled) setInternal(e.target.value)
          onChange?.(e.target.value)
        }}
      />
      {counter && rest.maxLength != null && <CharCounter className={styles.counter} current={v.length} max={rest.maxLength} />}
    </div>
  )
}

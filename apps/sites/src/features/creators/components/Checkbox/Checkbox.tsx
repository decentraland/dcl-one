import { type ComponentPropsWithoutRef, type ReactNode } from 'react'
import { CheckIcon } from '~/components/Icons'
import { cx } from '~/lib/cx'
import styles from './Checkbox.module.css'

type Props = {
  checked: boolean
  onChange: (checked: boolean) => void
  disabled?: boolean
  children: ReactNode
  testId: string
}

export function Checkbox({ checked, onChange, disabled, children, testId }: Props) {
  return (
    <label className={styles.row} data-disabled={disabled || undefined} data-testid="checkbox-label">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        data-testid={testId}
        onChange={event => onChange(event.target.checked)}
      />
      <CheckboxBox aria-hidden>
        <CheckIcon />
      </CheckboxBox>
      <span>{children}</span>
    </label>
  )
}

/** The drawn box, for a custom control that sits right after its own checked `input`. */
export function CheckboxBox({ className, ...props }: ComponentPropsWithoutRef<'span'>) {
  return <span className={cx(styles.box, className)} {...props} />
}

import type { ComponentProps } from 'react'

import styles from './InputField.module.css'

export type WebSelectProps = ComponentProps<'select'>
export type WebTextareaProps = ComponentProps<'textarea'>

const frameClass = `${styles.field} ${styles.webField} ${styles.nativeField}`

function isInvalid(value: WebSelectProps['aria-invalid']) {
  return Boolean(value && value !== 'false')
}

export function WebSelect({ className, ...props }: WebSelectProps) {
  return (
    <div className={frameClass} data-invalid={isInvalid(props['aria-invalid']) || undefined}>
      <select
        {...props}
        className={[styles.input, styles.nativeSelect, className].filter(Boolean).join(' ')}
      />
    </div>
  )
}

export function WebTextarea({ className, ...props }: WebTextareaProps) {
  return (
    <div className={frameClass} data-invalid={isInvalid(props['aria-invalid']) || undefined}>
      <textarea
        {...props}
        className={[styles.input, styles.nativeTextarea, className].filter(Boolean).join(' ')}
      />
    </div>
  )
}

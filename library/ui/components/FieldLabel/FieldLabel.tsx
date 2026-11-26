import type { LabelHTMLAttributes, ReactNode } from 'react'
import styles from './FieldLabel.module.css'

export type FieldLabelProps = LabelHTMLAttributes<HTMLLabelElement> & {
  /** A helper line under the label. */
  sublabel?: ReactNode
  /** A short superscript after the label, for example "Optional". */
  notice?: ReactNode
}

/** A form field's label, with an optional notice beside it and a helper line under it. */
export function FieldLabel({ children, sublabel, notice, className, ...rest }: FieldLabelProps) {
  const label = (
    <label {...rest} className={[styles.root, className].filter(Boolean).join(' ')}>
      {children}
      {notice != null && <sup className={styles.notice}>{notice}</sup>}
    </label>
  )
  if (sublabel == null) return label
  return (
    <span className={styles.group}>
      {label}
      <span className={styles.sub}>{sublabel}</span>
    </span>
  )
}

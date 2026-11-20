import type { HTMLAttributes, ReactNode } from 'react'
import styles from './Tag.module.css'

export type TagVariant = 'role'

export type TagProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  variant: TagVariant
  children: ReactNode
  appearance?: 'emphasized'
}

/** A text label for a community role. The text carries the meaning in both appearances. */
export function Tag({ variant, children, appearance, className, ...rest }: TagProps) {
  return (
    <span
      {...rest}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-variant={variant}
      data-appearance={appearance}
    >
      {children}
    </span>
  )
}

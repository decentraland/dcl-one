import type { HTMLAttributes } from 'react'
import styles from './Spinner.module.css'

export type SpinnerProps = {
  /** `dark` draws the track for a light surface, `light` for a dark one (Figma "Property 1"). */
  variant?: 'dark' | 'light'
  size?: 16 | 20 | 24 | 30 | 40
  label?: string
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'>

export function Spinner({ variant = 'dark', label = 'Loading', size = 30, className, ...rest }: SpinnerProps) {
  return (
    <span
      role="progressbar"
      aria-label={label}
      data-size={size}
      {...rest}
      className={[styles.root, variant === 'light' ? styles.light : styles.dark, className].filter(Boolean).join(' ')}
    />
  )
}

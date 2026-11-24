import type { HTMLAttributes } from 'react'
import styles from './Spinner.module.css'

export type SpinnerProps = {
  /**
   * `dark` draws the track for a light surface, `light` for a dark one (Figma "Property 1"). `thin` is the
   * Explorer HUD's ring: a faint track a tenth of its size thick and a rounded arc in the primary colour.
   */
  variant?: 'dark' | 'light' | 'thin'
  /** The ring's diameter in px. Without it the ring takes its variant's size: the Figma 30, or 28 for `thin`. */
  size?: number
  label?: string
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'>

export function Spinner({ variant = 'dark', size, label = 'Loading', className, style, ...rest }: SpinnerProps) {
  const classes = [styles.root, styles[variant], className].filter(Boolean).join(' ')
  if (variant === 'thin') {
    const sized = size === undefined ? style : { ...style, width: size, height: size }
    return (
      <span role="progressbar" aria-label={label} {...rest} style={sized} className={classes}>
        <svg viewBox="0 0 50 50" aria-hidden="true">
          <circle className={styles.thinTrack} cx="25" cy="25" r="20" fill="none" strokeWidth="5" />
          <circle
            className={styles.thinArc}
            cx="25"
            cy="25"
            r="20"
            fill="none"
            strokeWidth="5"
            strokeLinecap="round"
            strokeDasharray="90 160"
          />
        </svg>
      </span>
    )
  }
  // The Figma ring is 30 px with a 4 px track; a sized ring keeps that proportion.
  const sized = size === undefined ? style : { ...style, width: size, height: size, borderWidth: (size * 2) / 15 }
  return <span role="progressbar" aria-label={label} {...rest} style={sized} className={classes} />
}

import type { HTMLAttributes } from 'react'
import styles from './Spinner.module.css'

type RingSpinner = {
  /** `dark` draws the track for a light surface, `light` for a dark one (Figma "Property 1"). */
  variant?: 'dark' | 'light'
  size?: 16 | 20 | 24 | 30 | 40
}

type ThinSpinner = {
  /** The Explorer HUD's ring: a faint track a tenth of its size thick and a rounded arc in the primary colour. */
  variant: 'thin'
  /** The ring's diameter in px. Without it the ring is 28 px. */
  size?: number
}

export type SpinnerProps = (RingSpinner | ThinSpinner) & {
  label?: string
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'>

export function Spinner({ variant = 'dark', label = 'Loading', size, className, style, ...rest }: SpinnerProps) {
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
  return <span role="progressbar" aria-label={label} data-size={size ?? 30} {...rest} style={style} className={classes} />
}

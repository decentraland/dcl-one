import type { CSSProperties, HTMLAttributes } from 'react'
import styles from './Spinner.module.css'

type RingSpinner = {
  /** `dark` draws the track for a light surface, `light` for a dark one (Figma "Property 1"). */
  variant?: 'dark' | 'light'
  size?: 16 | 20 | 24 | 30 | 40
}

type HudSpinner = {
  /**
   * The Explorer HUD's rings, for dark surfaces. `thin`: a faint track a tenth of its size thick and a rounded
   * arc in the primary colour. `thick`: the Figma ring stroked on the edge of its box, so it spills half its
   * stroke past the size. `chase`: no track; an arc grows to a circle, its tail chases its head, and it turns
   * orange, ruby and purple, a second each.
   */
  variant: 'thin' | 'thick' | 'chase'
  /** The ring's diameter in px. Without it the ring is 28 px. */
  size?: number
}

export type SpinnerProps = (RingSpinner | HudSpinner) & {
  label?: string
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'>

function sized(style: CSSProperties | undefined, size: number | undefined): CSSProperties | undefined {
  return size === undefined ? style : { ...style, width: size, height: size }
}

export function Spinner({ variant = 'dark', size, label = 'Loading', className, style, ...rest }: SpinnerProps) {
  const classes = [styles.root, styles[variant], className].filter(Boolean).join(' ')
  // The HUD's rings are a live region, announced as they appear; the Figma ring is a progress bar with no value.
  const role = variant === 'thin' || variant === 'thick' || variant === 'chase' ? 'status' : 'progressbar'
  const named = { role, 'aria-label': label, ...rest, className: classes } as const
  if (variant === 'thin')
    return (
      <span {...named} style={sized(style, size)}>
        <svg viewBox="0 0 50 50" aria-hidden="true">
          <circle className={styles.thinTrack} cx="25" cy="25" r="20" fill="none" strokeWidth="5" />
          <circle
            className={styles.arc}
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
  if (variant === 'thick')
    return (
      <span {...named} style={sized(style, size)}>
        {/* The 64px ring: radius 32 on the box's edge, an 8.53 stroke spilling 4.27 past it. */}
        <svg viewBox="-4.2667 -4.2667 72.5333 72.5333" aria-hidden="true">
          <circle className={styles.thickTrack} cx="32" cy="32" r="32" fill="none" strokeWidth="8.5333" />
          <path className={styles.arc} d="M32 0A32 32 0 0 1 64 32" fill="none" strokeWidth="8.5333" strokeLinecap="round" />
        </svg>
      </span>
    )
  if (variant === 'chase')
    return (
      <span {...named} style={sized(style, size)}>
        <svg viewBox="0 0 50 50" aria-hidden="true">
          <circle className={styles.chaseArc} cx="25" cy="25" r="20" fill="none" strokeWidth="4" strokeLinecap="round" pathLength="100" />
        </svg>
      </span>
    )
  return <span {...named} data-size={size ?? 30} style={style} />
}

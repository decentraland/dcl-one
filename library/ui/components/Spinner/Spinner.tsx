import type { CSSProperties, HTMLAttributes } from 'react'
import styles from './Spinner.module.css'

export type SpinnerProps = {
  /**
   * `dark` draws the track for a light surface, `light` for a dark one (Figma "Property 1"). The others are
   * the Explorer HUD's rings, for dark surfaces. `thin`: a faint track a tenth of its size thick and a rounded
   * arc in the primary colour. `thick`: the Figma ring stroked on the edge of its box, so it spills half its
   * stroke past the size. `chase`: no track; an arc grows to a circle, its tail chases its head, and it turns
   * orange, ruby and purple, a second each.
   */
  variant?: 'dark' | 'light' | 'thin' | 'thick' | 'chase'
  /** The ring's diameter in px. Without it the ring takes its variant's size: the Figma 30, or 28 for the HUD's. */
  size?: number
  label?: string
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'role'>

function sized(style: CSSProperties | undefined, size: number | undefined, extra?: CSSProperties): CSSProperties | undefined {
  return size === undefined ? style : { ...style, width: size, height: size, ...extra }
}

export function Spinner({ variant = 'dark', size, label = 'Loading', className, style, ...rest }: SpinnerProps) {
  const classes = [styles.root, styles[variant], className].filter(Boolean).join(' ')
  const named = { role: 'progressbar', 'aria-label': label, ...rest, className: classes } as const
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
  // The Figma ring is 30 px with a 4 px track; a sized ring keeps that proportion.
  return <span {...named} style={sized(style, size, size === undefined ? undefined : { borderWidth: (size * 2) / 15 })} />
}

// Tooltip — wraps a trigger and shows a label (with an optional keyboard shortcut)
// on hover. Explorer 2.0 style: Shadow surface, 12px radius, Inter 600 14/17 Snow.
// Used by the sidebar IconButton; reusable anywhere a hover hint is needed.

import styles from './Tooltip.module.css'


interface TooltipBase {
  label: string
  /** Single-key shortcut shown dimmed, e.g. 'L' → "Friends [L]". */
  shortcut?: string
  /** rail: the sidebar's tooltip (tighter radius, 34px from the button centre, no fade). */
  variant?: 'default' | 'rail'
  /** A paragraph rather than a name: wraps, keeps line breaks, hangs from the trigger's left edge. */
  multiline?: boolean
  /** Keep the wrapper but show no tip (the trigger's own panel is open in its place). */
  quiet?: boolean
  className?: string
  children: React.ReactNode
}

/** `align` exists for a tip above or below its trigger: `end` lines the tip's right edge up with
 *  the trigger's, so a trigger at a container's right edge keeps its tip inside. */
type TooltipProps = TooltipBase & ({ side?: 'right' | 'left'; align?: never } | { side: 'top' | 'bottom'; align?: 'center' | 'end' })

export function Tooltip({
  label,
  shortcut,
  side = 'right',
  align = 'center',
  variant = 'default',
  multiline = false,
  quiet = false,
  className = '',
  children
}: TooltipProps): React.JSX.Element {
  const tip = [styles.tip, styles[side], align === 'end' && variant !== 'rail' && !multiline && styles.end, variant === 'rail' && styles.rail, multiline && styles.multiline].filter(Boolean).join(' ')
  return (
    <span className={`${styles.wrap} ${className}`.trim()}>
      {children}
      {!quiet && (
        <span className={tip} role="tooltip">
          {label}
          {shortcut && <span className={styles.shortcut}>[{shortcut}]</span>}
        </span>
      )}
    </span>
  )
}

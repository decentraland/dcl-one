import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { Tooltip } from '../Tooltip/Tooltip'
import styles from './RailButton.module.css'

export type RailButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'title'> & {
  /** What the circle holds: a 24px icon, or a framed 42px ProfilePic with `portrait`. */
  icon: ReactNode
  /** The content is a profile picture: it sits inset in the circle, under a ring of its own. */
  portrait?: boolean
  /** The selected look, for the button whose panel is open; it is the button's pressed state. */
  active?: boolean
  /** A count on the top right corner. Nothing shows at zero or without it. */
  badge?: number
  /** Above it the badge reads `+max`. Without it, a count above 99 reads `99+`. */
  badgeMax?: number
  /** The badge's colour: ruby, or lavender. */
  badgeTone?: 'ruby' | 'lavender'
  /** The button's accessible name, and its tip unless `tooltip` is given. */
  label: string
  /** The tip's text in place of `label`, for a button whose tip is a status. */
  tooltip?: string
  /** A single key shown dimmed in the tip. */
  shortcut?: string
  /** No tip: the button's own panel is open beside it and takes the tip's place. */
  quiet?: boolean
  /** A green dot on the top right edge, for example a connected voice chat. */
  indicator?: boolean
  /** A 14px picture over the top right corner, announced as `label`. */
  cornerArt?: { src: string; label: string }
}

/**
 * A round button of the Explorer HUD's navigation rail: a 42px dark circle with a selected ring, a count, a
 * status dot and a corner picture, inside the HUD's Tooltip. It is a native toggle button.
 */
export function RailButton({
  icon,
  portrait = false,
  active = false,
  badge,
  badgeMax,
  badgeTone = 'ruby',
  label,
  tooltip,
  shortcut,
  quiet = false,
  indicator = false,
  cornerArt,
  className,
  type = 'button',
  children,
  ...rest
}: RailButtonProps) {
  const classes = [styles.root, portrait && styles.portrait, active && styles.active, className]
    .filter(Boolean)
    .join(' ')
  return (
    <Tooltip
      appearance="hud"
      label={tooltip ?? label}
      shortcut={shortcut}
      side="right"
      variant="rail"
      disabled={quiet}
    >
      <button type={type} className={classes} aria-label={label} aria-pressed={active} {...rest}>
        {icon}
        {children}
        {indicator && <span className={styles.indicator} data-indicator />}
        {cornerArt != null && (
          <img
            className={styles.cornerArt}
            src={cornerArt.src}
            alt={cornerArt.label}
            width={14}
            height={14}
            draggable={false}
          />
        )}
        {badge != null && badge > 0 && (
          <span
            className={[styles.badge, badgeTone === 'lavender' && styles.badgeLavender]
              .filter(Boolean)
              .join(' ')}
          >
            {badgeText(badge, badgeMax)}
          </span>
        )}
      </button>
    </Tooltip>
  )
}

function badgeText(count: number, max: number | undefined): string | number {
  if (max != null) return count > max ? `+${max}` : count
  return count > 99 ? '99+' : count
}

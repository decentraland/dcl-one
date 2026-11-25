import type { CSSProperties, ReactNode } from 'react'
import styles from './EmptyState.module.css'

export type EmptyStateActionVariant = 'solid' | 'outline' | 'ghost'

export type EmptyStateAction = {
  label: ReactNode
  onClick?: () => void
  /** Renders a link in place of a button. */
  href?: string
  /** `solid` is a ruby fill, `outline` a ruby pill outline and `ghost` a faint outline. */
  variant?: EmptyStateActionVariant
  icon?: ReactNode
}

export type EmptyStateProps = {
  icon?: ReactNode
  /** Draws the icon inside a 76px translucent circle. */
  iconWash?: boolean
  title?: ReactNode
  subtitle?: ReactNode
  /** A list of action descriptors, or the caller's own action nodes. */
  actions?: EmptyStateAction[] | ReactNode
  /** `inline` sits inside a list, `card` is a bordered box and `screen` fills its page. */
  variant?: 'inline' | 'screen' | 'card'
  /** `error` enlarges the title and moves the actions 40px away. */
  tone?: 'error'
  /** The space above the actions. A number is in pixels. */
  actionsGap?: number | string
  className?: string
  style?: CSSProperties
}

const ACTION_CLASS: Record<EmptyStateActionVariant, string | undefined> = {
  solid: undefined,
  outline: styles.ctaOutline,
  ghost: styles.ctaGhost
}

function Action({ label, onClick, href, variant = 'solid', icon }: EmptyStateAction) {
  const className = [styles.cta, ACTION_CLASS[variant]].filter(Boolean).join(' ')
  const content = (
    <>
      {icon != null && (
        <span className={styles.ctaIcon} aria-hidden="true">
          {icon}
        </span>
      )}
      {label}
    </>
  )
  if (href != null)
    return (
      <a className={className} data-variant={variant} href={href} onClick={onClick}>
        {content}
      </a>
    )
  return (
    <button type="button" className={className} data-variant={variant} onClick={onClick}>
      {content}
    </button>
  )
}

/** A centred icon, title and subtitle with optional actions, for an empty or a failed surface. */
export function EmptyState({
  icon,
  iconWash = false,
  title,
  subtitle,
  actions,
  variant,
  tone,
  actionsGap,
  className,
  style
}: EmptyStateProps) {
  const gap = actionsGap ?? (tone === 'error' ? 40 : undefined)
  const merged =
    gap != null
      ? ({ ...style, '--es-actions-gap': typeof gap === 'number' ? `${gap}px` : gap } as CSSProperties)
      : style
  return (
    <div
      className={[
        styles.root,
        variant != null && styles[variant],
        tone === 'error' && styles.error,
        className
      ]
        .filter(Boolean)
        .join(' ')}
      data-variant={variant}
      data-tone={tone}
      style={merged}
    >
      {icon != null && (
        <div
          className={[styles.icon, iconWash && styles.iconWash].filter(Boolean).join(' ')}
          aria-hidden="true"
        >
          {icon}
        </div>
      )}
      {title != null && <p className={styles.title}>{title}</p>}
      {subtitle != null && <p className={styles.sub}>{subtitle}</p>}
      {Array.isArray(actions)
        ? actions.length > 0 && (
            <div className={styles.actions}>
              {actions.map((action: EmptyStateAction, index: number) => (
                <Action key={index} {...action} />
              ))}
            </div>
          )
        : actions != null && <div className={styles.actions}>{actions}</div>}
    </div>
  )
}

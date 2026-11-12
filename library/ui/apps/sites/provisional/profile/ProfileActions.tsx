// TODO(figma-ds): replace with the profile site's header buttons, until the Figma-generated Button from library/ui/components fits them.
import type { ReactNode } from 'react'
import { Icon, type IconName } from './icons'
import styles from './ProfileActions.module.css'

type ProfileButtonProps = {
  variant?: 'contained' | 'outlined'
  /** The icon before the label. */
  icon?: IconName
  /** A link when given, a button otherwise. */
  href?: string
  onClick?: () => void
  /** The profile site's sizes: `friend` (180px, 40px tall), `drawer` (fills its row, 40px) or `name` (fills, 40px). */
  size?: 'friend' | 'drawer' | 'name'
  /** A button with nothing it can do yet; its `title` says why. */
  disabled?: boolean
  title?: string
  children: ReactNode
}

/** decentraland-ui2's medium MUI Button: 36px, 14px/600 on a 24px line, 0.4px letter spacing, 6px radius. */
function ProfileButton({
  variant = 'contained',
  icon,
  href,
  onClick,
  size,
  disabled,
  title,
  children
}: ProfileButtonProps) {
  const className = [
    variant === 'outlined' ? styles.outlinedButton : styles.containedButton,
    size ? styles[`${size}Cta`] : undefined
  ]
    .filter(Boolean)
    .join(' ')
  const content = (
    <>
      {icon ? (
        <span className={styles.startIcon}>
          <Icon name={icon} />
        </span>
      ) : null}
      {children}
    </>
  )
  return href ? (
    <a className={className} href={href} onClick={onClick}>
      {content}
    </a>
  ) : (
    <button type="button" className={className} onClick={onClick} disabled={disabled} title={title}>
      {content}
    </button>
  )
}

/** The header's more-actions button, disabled for a visitor who is not signed in. */
function ProfileMoreButton({ disabled }: { disabled?: boolean }) {
  return (
    <button
      type="button"
      className={styles.moreButton}
      aria-label="More actions"
      disabled={disabled}
    >
      <Icon name="MoreVert" />
    </button>
  )
}

export { ProfileButton, ProfileMoreButton }
export type { ProfileButtonProps }

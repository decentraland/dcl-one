import type { HTMLAttributes } from 'react'
import styles from './NavProfileButton.module.css'

export type NavProfileButtonProps = {
  /** The account's face picture; without one the disc shows `fallbackSrc`. */
  imageUrl?: string
  /** The disc colour, as the reference takes it from the display name. */
  color?: string
  fallbackSrc: string
  /** Called when the face picture fails to load, so the caller can drop it. */
  onImageError?: () => void
} & Omit<HTMLAttributes<HTMLSpanElement>, 'children' | 'color'>

/**
 * The avatar disc of the site bar's profile control. The button that opens the profile menu belongs to the
 * caller, which owns the menu, its keyboard handling and the account actions.
 */
export function NavProfileButton({
  imageUrl,
  color,
  fallbackSrc,
  onImageError,
  className,
  style,
  ...rest
}: NavProfileButtonProps) {
  return (
    <span
      {...rest}
      className={className ? `${styles.root} ${className}` : styles.root}
      style={color ? { ...style, backgroundColor: color } : style}
    >
      {imageUrl ? (
        <img
          className={styles.image}
          src={imageUrl}
          alt=""
          width="40"
          height="40"
          loading="lazy"
          onError={onImageError}
        />
      ) : (
        <img className={styles.fallback} src={fallbackSrc} alt="" width="40" height="40" />
      )}
    </span>
  )
}

// TODO(figma-ds): replace with the Figma-generated avatar from library/ui/components.
import type { CSSProperties } from 'react'
import { useBrokenImage } from '../useBrokenImage'
import styles from './ProfileAvatar.module.css'

type ProfileAvatarProps = {
  /** The face snapshot; without one, or when it fails to load, the initial shows instead. */
  imageUrl?: string
  /** Display name, for the image's alt text and the fallback initial. */
  name: string
  /** Fallback background behind the initial, the profile's name color. */
  color: string
  /** Pixels; without it the size comes from `--avatar-size` (76px by default), so CSS can change it per breakpoint. */
  size?: number
  className?: string
}

function firstGrapheme(value: string): string {
  for (const ch of value) return ch
  return ''
}

/** A round avatar face with a soft border; the source's ProfileAvatar. */
function ProfileAvatar({ imageUrl, name, color, size, className }: ProfileAvatarProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)
  const showImage = Boolean(imageUrl) && !broken
  const style = {
    ...(size === undefined ? {} : { '--avatar-size': `${size}px` }),
    '--avatar-color': color
  } as CSSProperties

  return (
    <span className={[styles.root, className].filter(Boolean).join(' ')} style={style}>
      {showImage ? (
        <img ref={ref} className={styles.image} src={imageUrl} alt={name} onError={onError} />
      ) : (
        <span className={styles.initial} aria-hidden="true">
          {(firstGrapheme(name) || '?').toUpperCase()}
        </span>
      )}
    </span>
  )
}

export { ProfileAvatar }
export type { ProfileAvatarProps }

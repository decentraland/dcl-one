import { useEffect, useRef, useState } from 'react'
import type { CSSProperties, HTMLAttributes } from 'react'
import { ConnectionStatus, type ConnectionStatusValue } from './ConnectionStatus'
import styles from './ProfilePic.module.css'

export type ProfilePicSize = 76 | 72 | 60 | 48 | 40 | 32 | 28 | 24 | 20

export type ProfilePicProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  /** The member's name: the picture's text alternative, and the source of the fallback initial. */
  name: string
  /** The face picture. Without one, or when it fails to load, the initial shows. */
  src?: string
  /** The text alternative when it is not the name. `""` marks a picture shown beside the name as decoration. */
  alt?: string
  size?: ProfilePicSize
  /** Uses the 48px portrait below the desktop breakpoint. */
  compactOnMobile?: boolean
  /** The connection dot is shown from 32px up. */
  status?: ConnectionStatusValue
  statusLabel?: string
  /** The member's own name colour, behind the picture and the initial. It is member data, not a palette choice. */
  nameColor?: string
}

function initialOf(name: string): string {
  for (const character of name.trim()) return character.toUpperCase()
  return '?'
}

/**
 * A member's round face picture. The picture is an `img` whose `alt` is the member's name (or
 * `alt`); the fallback, the name's initial, carries the same text alternative.
 */
export function ProfilePic({
  name,
  src,
  alt = name,
  size = 72,
  compactOnMobile = false,
  status,
  statusLabel,
  nameColor,
  className,
  style,
  ...rest
}: ProfilePicProps) {
  const image = useRef<HTMLImageElement>(null)
  const [failed, setFailed] = useState<string>()
  // On a server-rendered page a picture can fail before React attaches `onError`.
  useEffect(() => {
    const element = image.current
    if (src && element?.complete && element.naturalWidth === 0) setFailed(src)
  }, [src])
  const shown = src !== undefined && src !== '' && failed !== src
  return (
    <span
      {...rest}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-size={size}
      data-compact-on-mobile={compactOnMobile || undefined}
      style={nameColor ? ({ ...style, '--profile-pic-name-color': nameColor } as CSSProperties) : style}
    >
      <span className={styles.picture}>
        {shown ? (
          <img
            ref={image}
            className={styles.image}
            src={src}
            alt={alt}
            draggable={false}
            onError={() => setFailed(src)}
          />
        ) : (
          <span
            className={styles.initial}
            {...(alt ? { role: 'img', 'aria-label': alt } : { 'aria-hidden': true })}
          >
            {initialOf(name)}
          </span>
        )}
      </span>
      {status && size >= 32 && (
        <ConnectionStatus className={styles.status} status={status} label={statusLabel} />
      )}
    </span>
  )
}

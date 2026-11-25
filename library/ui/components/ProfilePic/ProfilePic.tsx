import { useEffect, useRef, useState } from 'react'
import type { CSSProperties, HTMLAttributes } from 'react'
import { ConnectionStatus, type ConnectionStatusValue } from './ConnectionStatus'
import styles from './ProfilePic.module.css'

export type ProfilePicSize = 76 | 72 | 60 | 48 | 40 | 32 | 28 | 24 | 20

type Common = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  /** The member's name: the picture's text alternative, and the source of the fallback initial. */
  name: string
  /** The face picture. Without one, or when it fails to load, the initial shows. */
  src?: string
  /** The text alternative when it is not the name. `""` marks a picture shown beside the name as decoration. */
  alt?: string
  /** The member's own name colour, behind the picture and the initial. It is member data, not a palette choice. */
  nameColor?: string
  /** Called when the picture loads. */
  onPictureLoad?: () => void
  /** Called when the picture fails to load. */
  onPictureError?: () => void
}

type DefaultLook = {
  appearance?: 'default'
  size?: ProfilePicSize
  /** Uses the 48px portrait below the desktop breakpoint. */
  compactOnMobile?: boolean
  /** The connection dot is shown from 32px up. */
  status?: ConnectionStatusValue
  statusLabel?: string
}

/**
 * Not in Figma: the Explorer HUD's picture, at any diameter. The caller decides when a failed picture is tried
 * again: it passes no `src` while the initial should show. Its border, ring and dot read the `--avatar-*`
 * properties of the element or an ancestor.
 */
type HudLook = {
  appearance: 'hud'
  /** Diameter in px (default 32). */
  size?: number
  status?: ConnectionStatusValue | 'offline'
  /** The picture inset in its colour disc under a thin ring, as on the sidebar rail. */
  framed?: boolean
  /** The status dot's corner: bottom right (default), or a smaller dot at the top right. */
  dotPosition?: 'bottom' | 'top'
}

export type ProfilePicProps = Common & (DefaultLook | HudLook)

function initialOf(name: string): string {
  for (const character of name.trim()) return character.toUpperCase()
  return '?'
}

/** The HUD's initial: the first letter or digit, past a leading `@` or `#`. */
function hudInitialOf(name: string): string {
  const match = name.replace(/^[@#]/, '').match(/[a-z0-9]/i)
  return (match ? match[0] : name[0] || '?').toUpperCase()
}

const HUD_STATUS = { online: styles.hudOnline, away: styles.hudAway, offline: styles.hudOffline }

function HudProfilePic({
  name,
  src,
  alt = name,
  size = 32,
  status,
  framed = false,
  dotPosition = 'bottom',
  nameColor,
  onPictureLoad,
  onPictureError,
  className,
  style,
  ...rest
}: Omit<Extract<ProfilePicProps, { appearance: 'hud' }>, 'appearance'>) {
  const dot = Math.max(6, Math.round(size * (dotPosition === 'top' ? 0.15 : 0.18)))
  return (
    <span
      {...rest}
      className={[styles.hud, framed && styles.hudFramed, className].filter(Boolean).join(' ')}
      style={{
        ...style,
        width: size,
        height: size,
        fontSize: Math.round(size * 0.42),
        ...(nameColor ? { background: nameColor } : null)
      }}
    >
      {src ? (
        <img className={styles.hudImage} src={src} alt={alt} onLoad={onPictureLoad} onError={onPictureError} />
      ) : (
        hudInitialOf(name)
      )}
      {status && (
        <span
          className={[styles.hudDot, HUD_STATUS[status], dotPosition === 'top' && styles.hudDotTop]
            .filter(Boolean)
            .join(' ')}
          style={{ width: dot, height: dot }}
        />
      )}
    </span>
  )
}

/**
 * A member's round face picture. The picture is an `img` whose `alt` is the member's name (or
 * `alt`); the fallback, the name's initial, carries the same text alternative.
 */
export function ProfilePic({ appearance = 'default', ...props }: ProfilePicProps) {
  if (appearance === 'hud')
    return <HudProfilePic {...(props as Omit<Extract<ProfilePicProps, { appearance: 'hud' }>, 'appearance'>)} />
  return <DefaultProfilePic {...(props as Common & Omit<DefaultLook, 'appearance'>)} />
}

function DefaultProfilePic({
  name,
  src,
  alt = name,
  size = 72,
  compactOnMobile = false,
  status,
  statusLabel,
  nameColor,
  onPictureLoad,
  onPictureError,
  className,
  style,
  ...rest
}: Common & Omit<DefaultLook, 'appearance'>) {
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
            onLoad={onPictureLoad}
            onError={() => {
              setFailed(src)
              onPictureError?.()
            }}
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

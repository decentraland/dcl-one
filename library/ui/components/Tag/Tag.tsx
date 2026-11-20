import type { HTMLAttributes, ReactNode } from 'react'
import {
  GeneralFeatured,
  GeneralListen,
  GeneralLive,
  GeneralMapPin,
  GeneralPlayers,
  GeneralPrivate,
  GeneralPublic
} from '../../foundations/icons/general'
import styles from './Tag.module.css'

/**
 * Status: `live`, `online`, `streaming`, `listening`, `voiceStream`, `featured`. Location:
 * `location` (coordinates or a World's name). Community: `public`, `private` and `role` (owner,
 * moderator). `realm`. Age rating: `teen`, `adult`, `restricted`.
 */
export type TagVariant =
  | 'live'
  | 'online'
  | 'streaming'
  | 'listening'
  | 'voiceStream'
  | 'featured'
  | 'location'
  | 'public'
  | 'private'
  | 'role'
  | 'realm'
  | 'teen'
  | 'adult'
  | 'restricted'

export type TagProps = Omit<HTMLAttributes<HTMLSpanElement>, 'children'> & {
  variant: TagVariant
  /**
   * The text. `live`, `listening`, `voiceStream`, `featured`, `public`, `private` and the age
   * ratings have an English default; pass the localized word. `location`, `role` and `realm` show
   * what the caller gives; `online` and `streaming` show only their count.
   */
  children?: ReactNode
  /** The number after the players mark of `online`, `streaming`, `voiceStream` and `realm`. */
  count?: ReactNode
  /** What the count means, for assistive technology: "24 players". Without it the number is read alone. */
  countLabel?: string
}

const DEFAULT_TEXT: Partial<Record<TagVariant, string>> = {
  live: 'Live',
  listening: 'Listening',
  voiceStream: 'Live Voice Stream',
  featured: 'Featured',
  public: 'Public',
  private: 'Private',
  teen: 'T',
  adult: 'A',
  restricted: 'R'
}

/** The age rating shows one letter; this is the word assistive technology reads for it. */
const RATING_NAMES: Partial<Record<TagVariant, string>> = {
  teen: 'Teen',
  adult: 'Adult',
  restricted: 'Restricted'
}

const WITH_COUNT = new Set<TagVariant>(['online', 'streaming', 'voiceStream', 'realm'])
const TEXTLESS = new Set<TagVariant>(['online', 'streaming'])

const hidden = { focusable: 'false', 'aria-hidden': true } as const

/** The capture's `general/stream` mark, which the foundations do not export. */
function StreamMark({ size }: { size: number }) {
  return (
    <svg className={styles.icon} width={size} height={size} viewBox="0 0 24 24" fill="none" {...hidden}>
      <path
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        d="M6 19.789C4.159 18.106 3 15.66 3 12.939 3 7.864 7.03 3.75 12 3.75s9 4.114 9 9.189c0 2.722-1.159 5.167-3 6.85"
      />
      <path
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        d="M9.235 16.75C7.888 15.871 7 14.367 7 12.659 7 9.948 9.239 7.75 12 7.75s5 2.198 5 4.909c0 1.708-.888 3.212-2.235 4.091"
      />
      <circle cx="12" cy="13" r="2" fill="currentColor" />
    </svg>
  )
}

/** Three bars for a voice that is heard. The capture's own mark is not in it; this one stands in. */
function ListeningMark() {
  return (
    <svg className={styles.icon} width={10} height={10} viewBox="0 0 10 10" fill="none" {...hidden}>
      <path stroke="currentColor" strokeWidth="2" strokeLinecap="round" d="M1.5 4v2M5 1.5v7M8.5 3v4" />
    </svg>
  )
}

function mark(variant: TagVariant): ReactNode {
  switch (variant) {
    case 'live':
      return <GeneralLive className={styles.icon} size={18} {...hidden} />
    case 'online':
      return <span className={styles.dot} aria-hidden="true" />
    case 'streaming':
      return <StreamMark size={18} />
    case 'voiceStream':
      return (
        <span className={styles.halo} aria-hidden="true">
          <StreamMark size={22} />
        </span>
      )
    case 'listening':
      return <ListeningMark />
    case 'featured':
      return <GeneralFeatured className={styles.icon} size={14} {...hidden} />
    case 'location':
      return <GeneralMapPin className={styles.icon} size={14} {...hidden} />
    case 'public':
      return <GeneralPublic className={styles.icon} size={10} {...hidden} />
    case 'private':
      return <GeneralPrivate className={styles.icon} size={10} {...hidden} />
    default:
      return null
  }
}

/**
 * A small label for a state, a place or a kind: text with a mark before it and, for some variants,
 * a count after it. It is text, not a control. Every mark is decorative, so the text, the count
 * label or the age rating's word carries the meaning; colour is never the only sign.
 */
export function Tag({ variant, children, count, countLabel, className, ...rest }: TagProps) {
  const text = TEXTLESS.has(variant) ? null : (children ?? DEFAULT_TEXT[variant])
  const ratingName = RATING_NAMES[variant]
  // The one letter of an age rating is read as its word, unless the caller names the tag.
  const named =
    ratingName !== undefined && rest['aria-label'] === undefined && rest['aria-labelledby'] === undefined
      ? ({ role: 'img', 'aria-label': ratingName } as const)
      : undefined
  return (
    <span
      {...named}
      {...rest}
      className={[styles.root, className].filter(Boolean).join(' ')}
      data-variant={variant}
    >
      {mark(variant)}
      {text != null ? <span className={styles.text}>{text}</span> : null}
      {WITH_COUNT.has(variant) && count != null ? (
        <span className={styles.count}>
          {variant === 'voiceStream' ? (
            <GeneralListen className={styles.icon} size={12} {...hidden} />
          ) : (
            <GeneralPlayers className={styles.icon} size={variant === 'realm' ? 10 : 16} {...hidden} />
          )}
          {countLabel !== undefined ? (
            <>
              <span aria-hidden="true">{count}</span>
              <span className={styles.visuallyHidden}>{countLabel}</span>
            </>
          ) : (
            count
          )}
        </span>
      ) : null}
    </span>
  )
}

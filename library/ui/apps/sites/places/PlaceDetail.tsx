import type { CSSProperties, KeyboardEvent as ReactKeyboardEvent, ReactNode } from 'react'
import { useEffect, useId, useRef, useState } from 'react'
import { Button } from '../../../components/Button/Button'
import { CardActionButton } from '../../../components/CardActionButton/CardActionButton'
import { CloseButton } from '../../../components/CloseButton/CloseButton'
import { WebSelect } from '../../../components/InputField/InputFieldWebControls'
import { JumpInCTA } from '../../../components/JumpInCTA/JumpInCTA'
import { textClasses } from '../../../text-styles'
import {
  GeneralCopy,
  GeneralDislike,
  GeneralDislikeFilled,
  GeneralFavorites,
  GeneralFavoritesFilled,
  GeneralLike,
  GeneralLikeFilled
} from '../../../foundations/icons'
import foundationLogo from './foundation.svg'
import type { PlaceCardCreator } from './cards/types'
import styles from './PlaceDetail.module.css'

type PlaceDetailData = {
  id: string
  title: string
  description: string
  image?: string | null
  coords: string
  world?: boolean
  worldName?: string | null
  /** The share of the ratings that are likes, 0 to 100; null while nobody has rated the place. */
  approval?: number | null
  /** How many accounts saved the place. */
  favorites?: number
}

type PlaceDetailProps = {
  place?: PlaceDetailData
  creator?: PlaceDetailCreator
  notFound?: boolean
  /** Players in the scene now; with none (or on a phone) the page is the jump-in card. */
  livePlayers?: number
  mobile?: boolean
  jumpHref?: string
  onJumpIn?: () => void
  onClose?: () => void
  /** Opens the creator's profile; without it the creator's name is plain text. */
  onCreator?: (address: string) => void
  detailSlot?: ReactNode
  /** A world's scenes: with more than one, the live view's info panel picks the one to jump into. */
  scenePicker?: ScenePickerProps
  /** The live view's In-World Chat; until the host knows the room it is loading. */
  chat?: SceneChatView
  /** The signed-in account's own like and save; null until the host has read them. */
  viewer?: { liked: boolean | null; favorite: boolean } | null
  /** The account `viewer` and the writes belong to; a choice made under another account is forgotten. */
  account?: string | null
  /** Saves a like (true), a dislike (false) or neither (null); when it rejects, the button goes back. */
  onLike?: (placeId: string, like: boolean | null) => Promise<unknown>
  /** Saves or removes the favorite; when it rejects, the button goes back. */
  onFavorite?: (placeId: string, favorite: boolean) => Promise<unknown>
}

type PlaceDetailCreator = Pick<
  PlaceCardCreator,
  'name' | 'avatarUrl' | 'avatarBackground' | 'foundation'
> & {
  address?: string
}

type SceneChatMessageView = {
  /** Stable sender identity while the display name loads. */
  sender: string
  timestamp: number
  message: string
  name: string
  avatarUrl: string
  avatarBackground: string
}

type SceneChatView = {
  status: 'loading' | 'ready' | 'unavailable'
  messages: SceneChatMessageView[]
}

const CHAT_LOADING: SceneChatView = { status: 'loading', messages: [] }

function PinGlyph() {
  return (
    <svg className={styles.pin} viewBox="0 0 15 15" fill="none" aria-hidden="true">
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        fill="currentColor"
        d="M11.082 10.394C10.3068 11.565 9.41232 12.6461 8.90834 13.2307C8.21939 14.0298 6.99075 14.0298 6.3018 13.2307C5.79782 12.6461 4.90333 11.565 4.12816 10.394C3.74091 9.80902 3.36728 9.1775 3.08668 8.55678C2.81567 7.95727 2.58267 7.26354 2.58267 6.58317C2.58267 3.79361 4.81361 1.5 7.60507 1.5C10.3965 1.5 12.6275 3.79361 12.6275 6.58317C12.6275 7.26354 12.3945 7.95727 12.1235 8.55678C11.8429 9.1775 11.4692 9.80902 11.082 10.394ZM8.00374 12.4508C7.79124 12.6973 7.4189 12.6973 7.2064 12.4508C6.22605 11.3137 3.77702 8.30103 3.77702 6.58317C3.77702 4.43544 5.4909 2.69436 7.60507 2.69436C9.71924 2.69436 11.4331 4.43544 11.4331 6.58317C11.4331 8.30103 8.98409 11.3137 8.00374 12.4508Z"
      />
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        fill="currentColor"
        d="M7.6043 7.95758C8.35176 7.95758 8.9577 7.34202 8.9577 6.58269C8.9577 5.82336 8.35176 5.20781 7.6043 5.20781C6.85683 5.20781 6.25089 5.82336 6.25089 6.58269C6.25089 7.34202 6.85683 7.95758 7.6043 7.95758Z"
      />
    </svg>
  )
}

function Avatar({ creator }: { creator: PlaceDetailCreator }) {
  const avatar = creator.foundation ? foundationLogo : creator.avatarUrl
  if (!avatar) return null
  return (
    <img
      className={styles.avatar}
      src={avatar}
      alt=""
      style={{ '--avatar-bg': creator.avatarBackground } as CSSProperties}
    />
  )
}

/** The shared Jump in: a link to the scene, or a button while there is no link yet (the host retries travel). */
function JumpIn({
  href,
  onClick,
  size,
  className
}: {
  href?: string
  onClick?: () => void
  size: 'xl' | 'md'
  className?: string
}) {
  return href ? (
    <JumpInCTA size={size} className={className} href={href} onClick={onClick} />
  ) : (
    <JumpInCTA size={size} className={className} onClick={onClick} />
  )
}

type RatingProps = Pick<PlaceDetailProps, 'place' | 'viewer' | 'account' | 'onLike' | 'onFavorite'>
/** What the visitor pressed under one account; a field they did not press follows the account's saved state. */
type OwnRating = { account: string | null; liked?: boolean | null; favorite?: boolean }

const glyph = { focusable: false, 'aria-hidden': true } as const

/**
 * The like, dislike and favorite buttons with the place's counts. A press shows at once and is then saved by the
 * host; a save that fails (signed out included) puts the button back. Without the host's writes there are no buttons.
 */
function usePlaceRating({
  place,
  viewer = null,
  account = null,
  onLike,
  onFavorite
}: RatingProps): ReactNode {
  const [own, setOwn] = useState<OwnRating>({ account })
  if (!place || !onLike || !onFavorite) return null
  const mine = own.account === account ? own : { account }
  const liked = mine.liked !== undefined ? mine.liked : (viewer?.liked ?? null)
  const favorite = mine.favorite ?? viewer?.favorite ?? false
  const id = place.id
  const like = (value: boolean) => {
    const next = liked === value ? null : value
    setOwn({ ...mine, liked: next })
    onLike(id, next).catch(() =>
      setOwn((now) => (now.account === account ? { ...now, liked: mine.liked } : now))
    )
  }
  const save = () => {
    setOwn({ ...mine, favorite: !favorite })
    onFavorite(id, !favorite).catch(() =>
      setOwn((now) => (now.account === account ? { ...now, favorite: mine.favorite } : now))
    )
  }
  const favorites = place.favorites ?? 0
  return (
    <div className={styles.rating} role="group" aria-label="Rate and save this place">
      <CardActionButton
        aria-label="Like"
        aria-pressed={liked === true}
        onClick={() => like(true)}
        icon={liked === true ? <GeneralLikeFilled {...glyph} /> : <GeneralLike {...glyph} />}
      />
      <CardActionButton
        aria-label="Dislike"
        aria-pressed={liked === false}
        onClick={() => like(false)}
        icon={liked === false ? <GeneralDislikeFilled {...glyph} /> : <GeneralDislike {...glyph} />}
      />
      <CardActionButton
        aria-label="Favorite"
        aria-pressed={favorite}
        onClick={save}
        icon={favorite ? <GeneralFavoritesFilled {...glyph} /> : <GeneralFavorites {...glyph} />}
      />
      <p className={styles.ratingCounts}>
        {place.approval != null && <span>{place.approval}% likes</span>}
        <span>
          {favorites.toLocaleString('en-US')} {favorites === 1 ? 'favorite' : 'favorites'}
        </span>
      </p>
    </div>
  )
}

// A quote or a backslash is escaped and a line break is removed, so the address cannot end the string and add CSS.
const coverStyle = (cover?: string | null) =>
  cover
    ? ({
        '--cover': `url("${cover.replace(/[\\"]/g, '\\$&').replace(/[\r\n\f]/g, '')}")`
      } as CSSProperties)
    : undefined

// The "copied" note of the share button stays for 2 s, the time to read it, then the button shows its label again.
const COPIED_MS = 2000

function SceneJumpIn({
  place,
  creator,
  livePlayers,
  jumpHref,
  onJumpIn,
  onClose,
  rating,
  detailSlot
}: {
  place: PlaceDetailData
  creator?: PlaceDetailCreator
  livePlayers: number
  jumpHref?: string
  onJumpIn?: () => void
  onClose?: () => void
  rating?: ReactNode
  detailSlot?: ReactNode
}) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dialog = useRef<HTMLDivElement>(null)
  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    []
  )
  // A modal dialog takes the focus when it opens and gives it back to the control that opened it.
  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    dialog.current?.querySelector<HTMLElement>('button')?.focus()
    return () => opener?.focus()
  }, [])
  // Tab stays in the dialog: it goes from the last control to the first, and Shift+Tab the other way.
  const keepFocus = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.key !== 'Tab') return
    const controls = dialog.current?.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled])'
    )
    if (!controls?.length) return
    const first = controls[0]!
    const last = controls[controls.length - 1]!
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }
  useEffect(() => {
    if (!onClose) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])
  const copyLink = () => {
    navigator.clipboard
      ?.writeText(window.location.href)
      .then(() => {
        setCopied(true)
        if (timer.current) clearTimeout(timer.current)
        timer.current = setTimeout(() => setCopied(false), COPIED_MS)
      })
      .catch(() => undefined)
  }
  const location = place.world ? place.worldName || undefined : place.coords || undefined
  return (
    <div
      className={styles.backdrop}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose?.()
      }}
    >
      <div
        ref={dialog}
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={place.title}
        onKeyDown={keepFocus}
      >
        <CloseButton
          variant="filled"
          className={styles.close}
          aria-label="Close"
          onClick={onClose}
        />
        <div className={styles.heroWrap}>
          <div className={styles.hero} style={coverStyle(place.image)}>
            <div className={styles.heroFade} />
            {livePlayers > 0 && (
              <div className={styles.heroBadges}>
                <span className={styles.liveBadge}>
                  <svg
                    className={styles.badgeIcon}
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                    data-testid="SensorsRoundedIcon"
                  >
                    <path d="M8.54 8.54c.35.35.37.88.1 1.29C8.24 10.45 8 11.2 8 12c0 .8.24 1.55.64 2.17.27.41.24.95-.11 1.29-.43.43-1.17.4-1.51-.11C6.38 14.4 6 13.24 6 12c0-1.21.36-2.33.97-3.28.36-.54 1.11-.64 1.57-.18m6.92 6.92c.43.43 1.17.4 1.51-.11C17.62 14.4 18 13.24 18 12c0-1.24-.38-2.4-1.03-3.36-.34-.5-1.08-.54-1.51-.11-.35.35-.37.88-.11 1.29.41.63.65 1.38.65 2.18 0 .8-.24 1.55-.64 2.17-.27.41-.24.95.1 1.29M12 10c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2m6.32 8.32c.42.42 1.12.39 1.5-.08C21.18 16.53 22 14.36 22 12s-.82-4.53-2.18-6.24c-.37-.47-1.07-.5-1.5-.08-.36.36-.4.92-.08 1.32 1.1 1.37 1.76 3.11 1.76 5s-.66 3.63-1.76 5c-.32.39-.28.96.08 1.32M5.68 5.68c-.42-.42-1.12-.39-1.5.08C2.82 7.47 2 9.64 2 12s.82 4.53 2.18 6.24c.37.47 1.07.5 1.5.08.36-.36.4-.92.08-1.32C4.66 15.63 4 13.89 4 12s.66-3.63 1.76-5c.32-.39.28-.96-.08-1.32" />
                  </svg>
                  Live
                </span>
                <span className={styles.countBadge}>
                  <span className={styles.countDot} />
                  <svg
                    className={styles.badgeIcon}
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                    data-testid="PermIdentityRoundedIcon"
                  >
                    <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4m0-6c1.1 0 2 .9 2 2s-.9 2-2 2-2-.9-2-2 .9-2 2-2m0 7c-2.67 0-8 1.34-8 4v2c0 .55.45 1 1 1h14c.55 0 1-.45 1-1v-2c0-2.66-5.33-4-8-4m6 5H6v-.99c.2-.72 3.3-2.01 6-2.01s5.8 1.29 6 2z" />
                  </svg>
                  {livePlayers}
                </span>
              </div>
            )}
          </div>
          <div className={styles.heroText}>
            <h1 className={styles.modalTitle}>{place.title}</h1>
            <div className={styles.metaRow}>
              {creator?.name && (
                <div className={styles.creatorRow}>
                  <Avatar creator={creator} />
                  <p className={styles.modalBy}>
                    By <span className={styles.modalCreatorName}>{creator.name}</span>
                  </p>
                </div>
              )}
              {location && (
                <div className={styles.modalLocation}>
                  <PinGlyph />
                  {location}
                </div>
              )}
            </div>
            <div className={styles.ctaRow}>
              <JumpIn href={jumpHref} onClick={onJumpIn} size="xl" className={styles.modalJump} />
              <span className={styles.copyWrap}>
                <Button
                  iconOnly
                  variant="outline"
                  size="lg"
                  aria-label="Copy link"
                  onClick={copyLink}
                >
                  <GeneralCopy size={20} aria-hidden="true" />
                </Button>
                {/* Outside the Button, so the note keeps its own text style instead of the button's. */}
                <span role="status" className={copied ? styles.copied : styles.visuallyHidden}>
                  {copied ? 'Copied!' : ''}
                </span>
              </span>
            </div>
            {rating}
          </div>
        </div>
        {detailSlot && <div className={styles.about}>{detailSlot}</div>}
        {place.description && (
          <div className={styles.about}>
            <p className={styles.infoLabel}>What to expect</p>
            <p className={styles.infoText}>{place.description}</p>
          </div>
        )}
      </div>
    </div>
  )
}

const formatTime = (timestamp: number) =>
  new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })

function ChatMessage({ message }: { message: SceneChatMessageView }) {
  const { name, avatarUrl, avatarBackground } = message
  return (
    <div className={styles.message}>
      <div className={styles.messageHeader}>
        <img
          className={styles.messageAvatar}
          src={avatarUrl}
          alt=""
          style={{ '--avatar-bg': avatarBackground } as CSSProperties}
        />
        <span className={styles.messageName}>{name}</span>
        <span className={styles.messageTime}>{formatTime(message.timestamp)}</span>
      </div>
      <div className={styles.messageText}>{message.message}</div>
    </div>
  )
}

/** A key for each line that stays with the line when older lines leave the list: its time and sender, with a count for lines of one sender in one millisecond. */
function messageKeys(messages: SceneChatMessageView[]): string[] {
  const seen = new Map<string, number>()
  return messages.map((m) => {
    const line = `${m.timestamp}:${m.sender}`
    const n = seen.get(line) ?? 0
    seen.set(line, n + 1)
    return `${line}:${n}`
  })
}

/**
 * The In-World Chat column: always the same shell (header, messages, jump-in footer); while the room is being
 * asked for it shows the empty state, and when there is no room it says so. Read-only: viewers take part from the client.
 */
function SceneChatPanel({
  chat,
  title,
  jumpHref
}: {
  chat: SceneChatView
  title: string
  jumpHref?: string
}) {
  const keys = messageKeys(chat.messages)
  const endRef = useRef<HTMLDivElement>(null)
  const count = chat.messages.length
  useEffect(() => {
    endRef.current?.scrollIntoView?.({ behavior: 'smooth', block: 'nearest', inline: 'nearest' })
  }, [count])
  const empty = chat.status !== 'ready' || count === 0
  return (
    <div className={styles.chat}>
      <div className={styles.chatHeader}>
        <h2 className={styles.chatTitle}>In-World Chat</h2>
      </div>
      <div
        className={
          empty ? styles.chatMessages : `${styles.chatMessages} ${styles.chatMessagesList}`
        }
      >
        {empty ? (
          <p className={`${textClasses.bodyMd} ${styles.chatEmpty}`}>
            {chat.status === 'unavailable' ? 'Chat is unavailable right now.' : 'No messages yet.'}
          </p>
        ) : (
          <>
            {chat.messages.map((m, i) => (
              <ChatMessage key={keys[i]} message={m} />
            ))}
            <div ref={endRef} />
          </>
        )}
      </div>
      <p className={styles.chatFooter}>
        Jump into{' '}
        {jumpHref ? (
          <a className={styles.chatLink} href={jumpHref} target="_blank" rel="noopener noreferrer">
            {title}
          </a>
        ) : (
          title
        )}{' '}
        in Decentraland to participate in the chat.
      </p>
    </div>
  )
}

function SceneWatcher({
  place,
  jumpHref,
  onJumpIn,
  rating
}: {
  place: PlaceDetailData
  jumpHref?: string
  onJumpIn?: () => void
  rating?: ReactNode
}) {
  const [launched, setLaunched] = useState(false)
  return (
    <div className={styles.watcher}>
      <div className={styles.video}>
        {launched && jumpHref ? (
          <iframe
            className={styles.frame}
            src={jumpHref}
            title={place.title}
            allow="fullscreen; clipboard-read; clipboard-write"
          />
        ) : (
          <div className={styles.launch} style={coverStyle(place.image)} />
        )}
      </div>
      <div className={styles.controls}>
        {rating}
        <div className={styles.controlButtons}>
          {launched ? (
            <Button variant="inverse" size="md" onClick={() => setLaunched(false)}>
              Stop
            </Button>
          ) : (
            <Button
              variant="inverse"
              size="md"
              disabled={!jumpHref}
              onClick={() => setLaunched(true)}
            >
              Explore the scene
            </Button>
          )}
          <JumpIn href={jumpHref} onClick={onJumpIn} size="md" />
        </div>
      </div>
    </div>
  )
}

type ScenePickerProps = {
  options: { value: string; label: string }[]
  value: string
  onChange: (value: string) => void
}

function ScenePicker({ options, value, onChange }: ScenePickerProps) {
  const id = useId()
  return (
    <div className={styles.pickerRow}>
      <label htmlFor={id} className={styles.pickerLabel}>
        Scene
      </label>
      <div className={styles.pickerField}>
        <WebSelect id={id} value={value} onChange={(e) => onChange(e.target.value)}>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </WebSelect>
      </div>
    </div>
  )
}

function PlaceDetail({
  place,
  creator,
  notFound = false,
  livePlayers = 0,
  mobile = false,
  jumpHref,
  onJumpIn,
  onClose,
  onCreator,
  detailSlot,
  scenePicker,
  chat = CHAT_LOADING,
  viewer,
  account,
  onLike,
  onFavorite
}: PlaceDetailProps) {
  // One state for both layouts: a press stays when the page changes between the card and the live view.
  const rating = usePlaceRating({ place, viewer, account, onLike, onFavorite })
  const creatorAddress = creator?.address
  if (notFound || !place) {
    return (
      <div className={styles.page}>
        <div className={styles.notFound}>
          <h1 className={styles.title}>Scene not found</h1>
          <p className={styles.notFoundHint}>
            We couldn&apos;t load that place or world. It may have been removed.
          </p>
        </div>
      </div>
    )
  }
  if (mobile || livePlayers === 0) {
    return (
      <div className={styles.page}>
        <SceneJumpIn
          place={place}
          creator={creator}
          livePlayers={livePlayers}
          jumpHref={jumpHref}
          onJumpIn={onJumpIn}
          onClose={onClose}
          rating={rating}
          detailSlot={detailSlot}
        />
      </div>
    )
  }
  const location = place.world ? place.worldName || '' : place.coords
  const showPicker = scenePicker !== undefined && scenePicker.options.length > 1
  return (
    <div className={styles.page}>
      <div className={styles.content}>
        <section className={styles.viewer} aria-label={place.title}>
          <div className={styles.viewerHeader}>
            <h1 className={styles.title}>{place.title}</h1>
            <div className={styles.headerRight}>
              {creator?.name && (
                <div className={styles.creatorRow}>
                  <Avatar creator={creator} />
                  <p className={styles.by}>
                    By{' '}
                    {creatorAddress && onCreator ? (
                      <button
                        type="button"
                        className={styles.creatorName}
                        onClick={() => onCreator(creatorAddress)}
                      >
                        {creator.name}
                      </button>
                    ) : (
                      <span className={styles.creatorName}>{creator.name}</span>
                    )}
                  </p>
                </div>
              )}
              {location && (
                <a
                  className={styles.location}
                  href={jumpHref}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={jumpHref ? `Jump into ${location} (opens in a new tab)` : undefined}
                >
                  <PinGlyph />
                  {location}
                </a>
              )}
            </div>
          </div>
          <SceneWatcher place={place} jumpHref={jumpHref} onJumpIn={onJumpIn} rating={rating} />
        </section>

        <div className={styles.chatColumn}>
          <SceneChatPanel chat={chat} title={place.title} jumpHref={jumpHref} />
        </div>

        {(place.description || detailSlot || showPicker) && (
          <div className={styles.info}>
            {detailSlot}
            {showPicker && <ScenePicker {...scenePicker} />}
            {place.description && (
              <>
                <p className={styles.infoLabel}>What to expect</p>
                <p className={styles.infoText}>{place.description}</p>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

export { PlaceDetail }
export type { PlaceDetailCreator, PlaceDetailProps, SceneChatView }

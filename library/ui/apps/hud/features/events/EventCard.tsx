// The cards an event shows as: small and big in a calendar column, big in a day's grid, the
// banner in the highlighted carousel, and the empty slot that offers to create one.

import { useState } from 'react'
import { TintButton, Avatar, ContextMenu, MaskIcon, Skeleton, Tooltip, openPopup } from '../../design'
import type { EventData, Friend } from '@explorer-core/engine/protocol'
import type { EventCommunity } from '../session/useEngineSession'
import addGradientIcon from '../../assets/events/add-icn-gradient.png'
import calendarAddIcon from '../../assets/events/add-to-calendar-icn.png'
import calendarIcon from '../../assets/events/calendar-icn.png'
import defaultImage from '../../assets/events/default-image-place.png'
import jumpInIcon from '../../assets/events/jump-in-outline.png'
import linkIcon from '../../assets/events/link-icn.png'
import liveEventIcon from '../../assets/events/live-event.png'
import liveIcon from '../../assets/events/live-icn.png'
import playersIcon from '../../assets/events/players-icn.png'
import reminderOffIcon from '../../assets/events/reminder-off.png'
import reminderOnIcon from '../../assets/events/reminder-on.png'
import shareIcon from '../../assets/events/share-icn.png'
import timeIcon from '../../assets/events/time-icn.png'
import xIcon from '../../assets/events/x-icn.png'
import { eventDayText, eventTimeText } from './eventFormat'
import type { EventActions } from './useEventActions'
import styles from './EventCard.module.css'

/** How many friends' faces a card shows before "+N". */
const FRIEND_FACES = 3

/** The user's friends who are at the event now. */
export function friendsAtEvent(event: EventData, friends: Friend[]): Friend[] {
  if (event.connected_addresses.length === 0 || friends.length === 0) return []
  const here = new Set(event.connected_addresses.map((a) => a.toLowerCase()))
  return friends.filter((f) => here.has(f.address.toLowerCase()))
}

export interface EventCardProps {
  event: EventData
  /** The user's friends at the event now. */
  friends: Friend[]
  /** The event's community, when the user is a member of it. */
  community?: EventCommunity
  actions: EventActions
  /** Open the event's detail. */
  onOpen: () => void
}

// ---- pieces ----------------------------------------------------------------

/** The event's picture over a loading sweep; the place art when it has none or it fails. */
export function EventImage({ src, className = '' }: { src: string | null; className?: string }): React.JSX.Element {
  const [state, setState] = useState<'loading' | 'loaded' | 'failed'>('loading')
  const [shown, setShown] = useState(src)
  if (shown !== src) {
    setShown(src)
    setState('loading')
  }
  const failed = src == null || src === '' || state === 'failed'
  return (
    <span className={`${styles.image} ${className}`.trim()}>
      {!failed && state === 'loading' && <Skeleton tone="band" className={styles.imageLoading} />}
      <img
        className={`${styles.imageArt} ${state === 'loaded' || failed ? styles.imageShown : ''}`.trim()}
        src={failed ? defaultImage : src}
        alt=""
        draggable={false}
        onLoad={() => setState((s) => (s === 'failed' ? s : 'loaded'))}
        onError={() => setState('failed')}
      />
    </span>
  )
}

function OnlineCounter({ event }: { event: EventData }): React.JSX.Element | null {
  const online = event.connected_addresses.length
  if (!event.live || online === 0) return null
  return (
    <span className={styles.online} aria-label={`${online} here now`}>
      <span className={styles.onlineDot} />
      <MaskIcon src={playersIcon} size={16} />
      {online}
    </span>
  )
}

function FriendsHere({ event, friends }: { event: EventData; friends: Friend[] }): React.JSX.Element | null {
  if (!event.live || friends.length === 0) return null
  return (
    <span className={styles.friends}>
      {friends.slice(0, FRIEND_FACES).map((f) => (
        <Tooltip key={f.address} label={f.name} side="top" className={styles.friend}>
          <Avatar src={f.picture} name={f.name} size={26} />
        </Tooltip>
      ))}
      {friends.length > FRIEND_FACES && <span className={styles.friendsMore}>+{friends.length - FRIEND_FACES}</span>}
    </span>
  )
}

/** The LIVE badge, who is there now and which friends: a big card's and the banner's top row. */
function SocialInfo({ event, friends }: { event: EventData; friends: Friend[] }): React.JSX.Element {
  return (
    <span className={styles.social}>
      {event.live && (
        <span className={styles.liveBadge}>
          <MaskIcon src={liveIcon} size={18} />
          LIVE
        </span>
      )}
      <OnlineCounter event={event} />
      <FriendsHere event={event} friends={friends} />
    </span>
  )
}

function TimePill({ event }: { event: EventData }): React.JSX.Element {
  return (
    <span className={styles.time}>
      <MaskIcon src={timeIcon} size={20} />
      {eventTimeText(event, true)}
    </span>
  )
}

function openShareMenu(anchor: HTMLElement, event: EventData, actions: EventActions): void {
  const rect = anchor.getBoundingClientRect()
  // the menu's layer takes the pointer from the card: the card keeps its hovered look meanwhile
  const card = anchor.closest('article')
  card?.setAttribute('data-menu-open', '')
  openPopup(
    (dismiss) => (
      <div className={styles.shareMenu} style={{ left: rect.right, top: rect.top }} onClick={(e) => e.stopPropagation()}>
        <ContextMenu
          items={[
            {
              label: 'Share on X',
              icon: <MaskIcon src={xIcon} size={18} />,
              onClick: () => {
                dismiss()
                actions.shareOnX(event)
              }
            },
            {
              label: 'Copy link',
              icon: <MaskIcon src={linkIcon} size={18} />,
              onClick: () => {
                dismiss()
                actions.copyLink(event)
              }
            }
          ]}
        />
      </div>
    ),
    { dim: false, onClose: () => card?.removeAttribute('data-menu-open') }
  )
}

type ActionsSize = 'small' | 'large'

/** A card's buttons: the reminder and the calendar while the event is to come, the jump while
 *  it is live, and share. `remindLabel` writes "REMIND ME" beside the bell. */
export function EventActionButtons({
  event,
  actions,
  size,
  remindLabel = false,
  className = ''
}: {
  event: EventData
  actions: EventActions
  size: ActionsSize
  remindLabel?: boolean
  className?: string
}): React.JSX.Element {
  const radius = size === 'small' ? 9.6 : 12
  const icon = 24
  const box = size === 'small' ? styles.actionSmall : styles.actionLarge
  return (
    <span className={`${styles.actions} ${size === 'small' ? styles.actionsSmall : styles.actionsLarge} ${className}`.trim()}>
      {!event.live && (
        <TintButton
          radius={radius}
          tone="faint"
          className={`${remindLabel ? styles.remindWide : box} ${styles.remind}`}
          aria-label="Remind me"
          aria-pressed={event.attending}
          aria-busy={actions.reminding(event)}
          disabled={actions.reminding(event)}
          onClick={() => actions.remind(event)}
        >
          <MaskIcon src={event.attending ? reminderOnIcon : reminderOffIcon} size={icon} />
          {remindLabel && <span>REMIND ME</span>}
        </TintButton>
      )}
      {!event.live && (
        <TintButton radius={radius} tone="faint" className={box} aria-label="Add to calendar" onClick={() => actions.addToCalendar(event)}>
          <MaskIcon src={calendarAddIcon} size={icon} />
        </TintButton>
      )}
      {event.live && (
        <TintButton radius={radius} tone="brand" className={styles.jumpIn} onClick={() => actions.jumpIn(event)}>
          <span>JUMP IN</span>
          <MaskIcon src={jumpInIcon} size={24} />
        </TintButton>
      )}
      <TintButton radius={radius} tone="faint" className={box} aria-label="Share" aria-haspopup="menu" onClick={(e) => openShareMenu(e.currentTarget, event, actions)}>
        <MaskIcon src={shareIcon} size={icon} />
      </TintButton>
    </span>
  )
}

const cardState = (event: EventData): string => (event.live ? styles.live : event.attending ? styles.reminded : '')

/** The card's own click target, under its buttons: opens the detail. */
function OpenButton({ event, onOpen }: { event: EventData; onOpen: () => void }): React.JSX.Element {
  return <button type="button" className={styles.open} aria-label={`Event: ${event.name}`} onClick={onOpen} />
}

// ---- cards -----------------------------------------------------------------

export function EventCardSmall({ event, actions, onOpen }: EventCardProps): React.JSX.Element {
  return (
    <article className={`${styles.card} ${styles.small} ${cardState(event)}`.trim()} data-event={event.id}>
      <span className={styles.surface}>
        <span className={styles.glow} />
        <EventImage src={event.image} className={styles.smallImage} />
        {event.live && <span className={styles.liveStripe} />}
      </span>
      <OpenButton event={event} onOpen={onOpen} />
      {event.live && (
        <span className={styles.smallMarks}>
          <span className={styles.liveMark}>
            <MaskIcon src={liveEventIcon} size={18} />
          </span>
          <OnlineCounter event={event} />
        </span>
      )}
      <span className={styles.smallTitle}>
        <span className={styles.name}>{event.name}</span>
        <span className={styles.host}>
          By <b>{event.user_name}</b>
        </span>
      </span>
      <span className={styles.smallTime}>
        <TimePill event={event} />
      </span>
      <EventActionButtons event={event} actions={actions} size="small" className={styles.smallActions} />
    </article>
  )
}

export function EventCardBig({ event, friends, community, actions, onOpen, wide = false }: EventCardProps & { wide?: boolean }): React.JSX.Element {
  return (
    <article className={`${styles.card} ${styles.big} ${wide ? styles.wide : ''} ${cardState(event)}`.trim()} data-event={event.id}>
      <span className={styles.surface}>
        <span className={styles.glow} />
        <span className={styles.watermark} />
        <EventImage src={event.image} className={styles.bigImage} />
        {event.live && <span className={styles.liveStripe} />}
      </span>
      <OpenButton event={event} onOpen={onOpen} />
      <span className={styles.bigSocial}>
        <SocialInfo event={event} friends={friends} />
      </span>
      {community != null && (
        <Tooltip label={community.name} side="top" className={styles.community}>
          <span className={styles.communityThumb}>
            {community.thumbnail !== '' && <img src={community.thumbnail} alt="" draggable={false} onError={(e) => (e.currentTarget.style.visibility = 'hidden')} />}
          </span>
        </Tooltip>
      )}
      <span className={styles.bigFooter}>
        <span className={styles.name}>{event.name}</span>
        <span className={styles.host}>
          By <b>{event.user_name}</b>
        </span>
        <span className={styles.bigTime}>
          <TimePill event={event} />
        </span>
        <EventActionButtons event={event} actions={actions} size="large" remindLabel className={styles.bigActions} />
      </span>
    </article>
  )
}

export function EventCardBanner({ event, friends, actions, onOpen, withDots }: EventCardProps & { withDots: boolean }): React.JSX.Element {
  return (
    <article className={`${styles.banner} ${withDots ? styles.bannerDots : ''}`.trim()} data-event={event.id}>
      <span className={styles.bannerSurface}>
        <EventImage src={event.image_vertical ?? event.image} className={styles.bannerImage} />
        <span className={styles.bannerShade} />
        {event.live && <span className={styles.liveStripe} />}
      </span>
      <OpenButton event={event} onOpen={onOpen} />
      <span className={styles.bannerContent}>
        <SocialInfo event={event} friends={friends} />
        {!event.live && (
          <span className={styles.bannerDate}>
            <MaskIcon src={calendarIcon} size={18} />
            {eventDayText(event)}
            <MaskIcon src={timeIcon} size={18} />
            {eventTimeText(event, true)}
          </span>
        )}
        <span className={styles.bannerName}>{event.name}</span>
        <span className={styles.bannerHost}>
          Organized by <b>{event.user_name}</b>
        </span>
        <EventActionButtons event={event} actions={actions} size="large" remindLabel className={styles.bannerActions} />
      </span>
    </article>
  )
}

/** A free slot in a column: shows "CREATE EVENT" under the pointer. */
export function EventCardEmptySlot({ onClick }: { onClick: () => void }): React.JSX.Element {
  return (
    <button type="button" className={styles.emptySlot} aria-label="Create event on this day" onClick={onClick}>
      <span className={styles.emptySlotOutline} />
      <img src={addGradientIcon} alt="" width={50} height={50} draggable={false} />
      <span>CREATE EVENT</span>
    </button>
  )
}

/** A card's place while its column loads. */
export function EventCardSkeleton({ wide = false }: { wide?: boolean }): React.JSX.Element {
  return (
    <span className={`${styles.skeletonCard} ${wide ? styles.wide : ''}`.trim()} aria-hidden="true">
      <Skeleton tone="band" className={styles.skeletonImage} />
      <Skeleton tone="band" className={styles.skeletonTitle} />
      <Skeleton tone="band" className={styles.skeletonText} />
    </span>
  )
}

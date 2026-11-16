import { Children, useEffect, useState } from 'react'
import friendsEmpty from '../../assets/lobby/friends-empty.webp'
import players from '../../assets/lobby/players.png'
import { Avatar, Button, MaskIcon, VerifiedBadge } from '../../design'
import {
  GeneralAddToCalendar,
  GeneralHome,
  GeneralJumpIn,
  GeneralLeftArrow,
  GeneralLive,
  GeneralMapPin,
  GeneralRightArrow,
  GeneralTime
} from '../../../../foundations/icons/general'
import { MenusShare } from '../../../../foundations/icons/menus'
import { NavbarFriends } from '../../../../foundations/icons/navbar'
import { eventStartAt, type DclEvent } from '@explorer-core/features/events/eventsApi'
import { FriendFaces } from '../events/EventCard'
import { useCreatorName } from '../places/useCreatorName'
import type { Friend } from '@explorer-core/engine/protocol'
import styles from './LobbyCards.module.css'

function OnlineCounter({ count }: { count: number }): React.JSX.Element {
  return (
    <span className={styles.counter}>
      <span className={styles.onlineDot} />
      <MaskIcon src={players} size={16} />
      {count}
    </span>
  )
}

function JumpIn({ disabled, onClick }: { disabled?: boolean; onClick: () => void }): React.JSX.Element {
  return (
    <button type="button" className={styles.jump} disabled={disabled} onClick={onClick}>
      Jump in
      <GeneralJumpIn size={24} aria-hidden="true" />
    </button>
  )
}

/** A section's title row: the title on the left, VIEW ALL (when the section has a page to open) on the right. */
export function SectionHead({ title, onViewAll, children }: { title: string; onViewAll?: () => void; children?: React.ReactNode }): React.JSX.Element {
  return (
    <div className={styles.head}>
      <h2 className={styles.headTitle}>{title}</h2>
      {children}
      {onViewAll && (
        <button type="button" className={styles.viewAll} onClick={onViewAll}>
          <span>View all</span>
          <GeneralRightArrow size={24} className={styles.viewAllArrow} aria-hidden="true" />
        </button>
      )}
    </div>
  )
}

/**
 * Cards a page at a time, with the lobby's pager under them: a dot per page, and Previous and Next
 * arrows that show while the section is hovered or holds focus (they wrap at the ends). A single
 * page has no pager; `reserve` keeps its row, so the widget is as tall with one page as with many.
 */
export function Pages({ perPage, gap, reserve = false, children }: { perPage: number; gap: number; reserve?: boolean; children: React.ReactNode }): React.JSX.Element {
  const items = Children.toArray(children)
  const pages = Math.max(1, Math.ceil(items.length / perPage))
  const [page, setPage] = useState(0)
  useEffect(() => setPage((p) => Math.min(p, pages - 1)), [pages])
  const shown = Math.min(page, pages - 1)
  const step = (by: number): void => setPage((shown + by + pages) % pages)
  return (
    <div className={styles.pages}>
      <div className={styles.pageRow} style={{ gap }}>
        {items.slice(shown * perPage, shown * perPage + perPage)}
      </div>
      {pages > 1 ? (
        <div className={styles.pager}>
          <button type="button" aria-label="Previous page" className={styles.pagerArrow} onClick={() => step(-1)}>
            <GeneralLeftArrow size={32} aria-hidden="true" />
          </button>
          <div className={styles.dots}>
            {Array.from({ length: pages }, (_, i) => (
              <button
                key={i}
                type="button"
                aria-current={i === shown ? 'true' : undefined}
                aria-label={`Page ${i + 1}`}
                className={`${styles.dot} ${i === shown ? styles.dotActive : ''}`.trim()}
                onClick={() => setPage(i)}
              />
            ))}
          </div>
          <button type="button" aria-label="Next page" className={styles.pagerArrow} onClick={() => step(1)}>
            <GeneralRightArrow size={32} aria-hidden="true" />
          </button>
        </div>
      ) : (
        reserve && <PagerSpace />
      )}
    </div>
  )
}

/** The pager's row with nothing in it. */
function PagerSpace(): React.JSX.Element {
  return <div className={styles.pagerSpace} aria-hidden="true" />
}

export function LandingCard({
  title,
  creator,
  image,
  count,
  loading,
  disabled,
  onJumpIn,
  onOpen
}: {
  title: string
  creator: string | null
  image: string | null
  count: number | null
  loading: boolean
  disabled: boolean
  onJumpIn: () => void
  /** A click anywhere but Jump In: the place's detail card, once the place is known. */
  onOpen?: () => void
}): React.JSX.Element {
  const creatorName = useCreatorName(creator)
  const open = loading ? undefined : onOpen
  return (
    <div
      className={`${styles.landing} ${open ? styles.landingOpen : ''}`.trim()}
      aria-busy={loading}
      title={creatorName ?? undefined}
      onClick={open && ((e) => !(e.target as HTMLElement).closest('button') && open())}
    >
      <div className={`${styles.thumb} ${loading ? styles.skeleton : ''}`.trim()}>{image && <img src={image} alt="" />}</div>
      <div className={styles.shade} />
      {count != null && !loading && (
        <div className={styles.landingBadges}>
          <OnlineCounter count={count} />
        </div>
      )}
      <div className={styles.landingTitle}>
        <GeneralHome size={24} />
        <span>{title}</span>
      </div>
      <div className={styles.landingJump}>
        <JumpIn disabled={loading || disabled} onClick={onJumpIn} />
      </div>
    </div>
  )
}

export function PlaceCard({
  title,
  creator,
  image,
  count,
  disabled,
  onJumpIn,
  onOpen
}: {
  title: string
  creator: string | null
  image: string | null
  count: number
  disabled: boolean
  onJumpIn: () => void
  /** A click anywhere but Jump In: the place's detail card. */
  onOpen: () => void
}): React.JSX.Element {
  const creatorName = useCreatorName(creator)
  return (
    <div className={styles.place} onClick={(e) => !(e.target as HTMLElement).closest('button') && onOpen()}>
      <div className={styles.thumb}>{image && <img src={image} alt="" />}</div>
      <div className={styles.shade} />
      {count > 0 && (
        <div className={styles.placeBadges}>
          <OnlineCounter count={count} />
        </div>
      )}
      <div className={styles.placeText}>
        <div className={styles.placeTitle}>{title}</div>
        <div className={`${styles.placeFoot} ${creatorName ? '' : styles.placeFootBare}`.trim()}>
          {creatorName && <div className={styles.placeCreator}>{creatorName}</div>}
          <div className={styles.placeJump}>
            <JumpIn disabled={disabled} onClick={onJumpIn} />
          </div>
        </div>
      </div>
    </div>
  )
}

const HOUR = 3_600_000

/** "In 1 hour", "In 3 days", as the lobby's event card says it. */
export function startsIn(startAt: string, now: number): string {
  const ms = new Date(startAt).getTime() - now
  if (!Number.isFinite(ms) || ms <= 0) return 'Live now'
  const plural = (n: number, unit: string): string => `In ${n} ${unit}${n === 1 ? '' : 's'}`
  if (ms < HOUR) return plural(Math.max(1, Math.round(ms / 60_000)), 'minute')
  if (ms < 24 * HOUR) return plural(Math.round(ms / HOUR), 'hour')
  return plural(Math.round(ms / (24 * HOUR)), 'day')
}

/**
 * An event that is on now: its picture, the LIVE badge with who is there, its name and its host.
 * Hovered or focused, Jump in takes the host's line.
 */
export function LiveEventCard({
  event,
  friends,
  disabled,
  onJumpIn,
  onOpen
}: {
  event: DclEvent
  /** The player's friends at the event. */
  friends: Friend[]
  disabled: boolean
  onJumpIn: () => void
  /** A click anywhere but Jump in: the event's detail card, when the event has a place to go to. */
  onOpen?: () => void
}): React.JSX.Element {
  const here = event.connected_addresses?.length ?? 0
  return (
    <div
      className={`${styles.liveCard} ${onOpen ? styles.landingOpen : ''}`.trim()}
      onClick={onOpen && ((e) => !(e.target as HTMLElement).closest('button') && onOpen())}
    >
      <div className={styles.thumb}>{event.image && <img src={event.image} alt="" />}</div>
      <div className={styles.liveBadges}>
        <span className={styles.liveBadge}>
          <GeneralLive size={18} aria-hidden="true" />
          Live
        </span>
        {here > 0 && <OnlineCounter count={here} />}
        {friends.length > 0 && (
          <span className={styles.liveFriends}>
            {/* under the faces: the card clips what is drawn above them */}
            <FriendFaces friends={friends} tip="bottom" />
          </span>
        )}
      </div>
      <div className={styles.liveText}>
        <div className={styles.liveTitle}>{event.name}</div>
        <div className={`${styles.liveFoot} ${event.user_name ? '' : styles.liveFootBare}`.trim()}>
          {event.user_name && <div className={styles.liveBy}>{event.user_name}</div>}
          <div className={styles.liveJump}>
            <JumpIn disabled={disabled} onClick={onJumpIn} />
          </div>
        </div>
      </div>
    </div>
  )
}

/**
 * An event to come: its name, host and how long until it starts, beside its picture. Hovered or
 * focused, Add to calendar and Share take the host's line.
 */
export function UpcomingEventCard({
  event,
  onOpen,
  onAddToCalendar,
  onShare
}: {
  event: DclEvent
  /** null when the event has no place to go to. */
  onOpen: (() => void) | null
  onAddToCalendar: () => void
  /** Opens the share menu beside the button. */
  onShare: (button: HTMLElement) => void
}): React.JSX.Element {
  return (
    <article className={styles.event}>
      <button type="button" className={styles.eventOpen} disabled={onOpen == null} onClick={onOpen ?? undefined}>
        <span className={styles.eventTitle}>{event.name}</span>
        {event.user_name && (
          <span className={styles.eventBy}>
            By <b>{event.user_name}</b>
          </span>
        )}
        <span className={styles.eventWhen}>
          <GeneralTime size={20} aria-hidden="true" />
          {startsIn(eventStartAt(event), Date.now())}
        </span>
      </button>
      <span className={styles.eventImage}>{event.image && <img src={event.image} alt="" />}</span>
      <span className={styles.eventActions}>
        <Button size="bare" radius="button" variant="tint" className={styles.eventAction} aria-label="Add to calendar" onClick={onAddToCalendar}>
          <GeneralAddToCalendar size={24} aria-hidden="true" />
        </Button>
        <Button size="bare" radius="button" variant="tint" className={styles.eventAction} aria-label="Share" aria-haspopup="menu" onClick={(e) => onShare(e.currentTarget)}>
          <MenusShare size={24} aria-hidden="true" />
        </Button>
      </span>
    </article>
  )
}

/** No friends yet: how to make some. The pager's row stays under the card, as in the design. */
export function FriendsEmpty(): React.JSX.Element {
  return (
    <div className={styles.pages}>
      <div className={styles.friendsEmpty}>
        <span className={styles.friendsEmptyGlow} aria-hidden="true" />
        <img className={styles.friendsEmptyArt} src={friendsEmpty} alt="" />
        <div className={styles.friendsEmptyText}>
          <div className={styles.friendsEmptyTitle}>Time to make some friends!</div>
          <div className={styles.friendsEmptyBody}>Visit someone's profile or click their name in Chat to add them as a friend. They'll show up here when they're online.</div>
        </div>
      </div>
      <PagerSpace />
    </div>
  )
}

/** Friends, none of them online. */
export function FriendsNoneOnline(): React.JSX.Element {
  return (
    <div className={styles.pages}>
      <div className={styles.friendsNone}>
        <NavbarFriends size={34} aria-hidden="true" />
        <span>No Friends Online</span>
      </div>
      <PagerSpace />
    </div>
  )
}

export function FriendCard({ friend, color, where, onOpen }: { friend: Friend; color: string; where: string; onOpen: () => void }): React.JSX.Element {
  return (
    <button type="button" className={styles.friend} onClick={onOpen}>
      <Avatar src={friend.picture} name={friend.name} color={color} framed size={40} dotPosition="top" status={friend.status === 'away' ? 'away' : 'online'} />
      <span className={styles.friendText}>
        <span className={styles.friendName} style={{ color }}>
          <span>{friend.name}</span>
          {friend.claimed && <VerifiedBadge size={12} />}
        </span>
        <span className={styles.friendWhere}>
          <GeneralMapPin size={14} aria-hidden="true" />
          <span>{where}</span>
        </span>
      </span>
    </button>
  )
}

// The lobby's cards: the landing (home) card, places, live and upcoming events, online friends,
// communities and gallery photos, and the row of friend pictures a place or an event shows. Each
// card is drawn from plain values; the widgets (LobbyWidgets) feed and wire them.

import { useState } from 'react'
import addToCalendar from '../../assets/lobby/add-to-calendar-icn.png'
import defaultPlace from '../../assets/lobby/default-image-place.png'
import defaultProfile from '../../assets/lobby/default-profile-thumbnail.png'
import defaultCommunity from '../../assets/lobby/thumbnail-default.png'
import homeIcon from '../../assets/lobby/home-icon.png'
import jumpIn from '../../assets/lobby/jump-in-outline.png'
import liveIcon from '../../assets/lobby/live-icn.png'
import locationIcon from '../../assets/lobby/location-icn.png'
import playersIcon from '../../assets/lobby/players-icn.png'
import privateIcon from '../../assets/lobby/private-icn.png'
import publicIcon from '../../assets/lobby/public-icn.png'
import reminderOff from '../../assets/lobby/reminder-off.png'
import reminderOn from '../../assets/lobby/reminder-on.png'
import shareIcon from '../../assets/lobby/share-icn.png'
import timeIcon from '../../assets/lobby/time-icn.png'
import verifiedIcon from '../../assets/lobby/icon-verified.png'
import { MaskIcon } from '../../design'
import { useCreatorName } from '../places/useCreatorName'
import styles from './LobbyCards.module.css'

const cx = (...names: Array<string | false | null | undefined>): string => names.filter(Boolean).join(' ')

/** A card's picture: a shimmering bone until it has loaded, then fading in over the default picture. */
function Thumb({ src, fallback, className }: { src: string | null | undefined; fallback: string; className: string }): React.JSX.Element {
  const [loaded, setLoaded] = useState<string | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const url = src && src !== failed ? src : fallback
  const waiting = src != null && src !== '' && src !== failed && loaded !== src
  return (
    <span className={className}>
      <img className={styles.thumbImage} src={url} alt="" draggable={false} onLoad={() => setLoaded(url)} onError={() => src && setFailed(src)} />
      <span className={cx(styles.thumbSkeleton, waiting && styles.thumbSkeletonShown)} aria-hidden="true" />
    </span>
  )
}

/** The online counter pill: a green dot, the players icon and the count. Hidden at 0. */
function OnlineBadge({ count, className }: { count: number | null | undefined; className?: string }): React.JSX.Element | null {
  if (count == null || count <= 0) return null
  return (
    <span className={cx(styles.online, className)}>
      <span className={styles.onlineDot} />
      <MaskIcon src={playersIcon} size={16} className={styles.onlineIcon} />
      <span className={styles.onlineCount}>{count}</span>
    </span>
  )
}

export interface ConnectedFriend {
  address: string
  name: string
  picture?: string | null
}

const MAX_FRIEND_PICTURES = 3

/** The friends at a place or an event: up to three overlapping pictures, then "+N". */
export function ConnectedFriends({ friends, onHover }: { friends: ConnectedFriend[]; onHover?: (friend: ConnectedFriend | null, e?: React.MouseEvent) => void }): React.JSX.Element | null {
  if (friends.length === 0) return null
  const shown = friends.slice(0, MAX_FRIEND_PICTURES)
  const more = friends.length - shown.length
  return (
    <span className={styles.friendsRow} aria-label={`${friends.length} ${friends.length === 1 ? 'friend' : 'friends'} here`}>
      {shown.map((f, i) => (
        <span
          key={f.address}
          className={cx(styles.friendPicture, i > 0 && styles.overlapping)}
          style={{ backgroundImage: `url("${f.picture || defaultProfile}")` }}
          onMouseMove={(e) => onHover?.(f, e)}
          onMouseLeave={() => onHover?.(null)}
        />
      ))}
      {more > 0 && <span className={cx(styles.friendsOverflow, styles.overlapping)}>+{more}</span>}
    </span>
  )
}

function JumpInButton({ size, className, disabled, onClick, label }: { size: 'large' | 'small'; className: string; disabled?: boolean; onClick: () => void; label: string }): React.JSX.Element {
  return (
    <button
      type="button"
      className={cx(styles.jumpIn, size === 'large' ? styles.jumpInLarge : styles.jumpInSmall, className)}
      disabled={disabled}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
    >
      <span className={styles.jumpInText}>JUMP IN</span>
      <MaskIcon src={jumpIn} size={size === 'large' ? 24 : 22} />
    </button>
  )
}

/** The landing card: where Jump In goes, with its picture, the people there and the friends among them. */
export function LandingCard({
  title,
  image,
  online,
  friends,
  canOpen,
  disabled,
  onOpen,
  onJumpIn,
  onFriendHover
}: {
  title: string
  image: string | null
  online: number | null
  friends: ConnectedFriend[]
  /** The place is known (not an offline stand-in): the card opens it. */
  canOpen: boolean
  disabled: boolean
  onOpen: () => void
  onJumpIn: () => void
  onFriendHover?: (friend: ConnectedFriend | null, e?: React.MouseEvent) => void
}): React.JSX.Element {
  return (
    <div className={cx(styles.landing, !canOpen && styles.static)}>
      <span className={styles.outline} aria-hidden="true" />
      <div className={styles.landingBody}>
        <Thumb src={image} fallback={defaultPlace} className={styles.landingThumb} />
        <span className={cx(styles.gradient, styles.landingGradient)} />
        {canOpen && <button type="button" className={styles.cover} aria-label={`Open ${title}`} onClick={onOpen} />}
        <span className={cx(styles.badges, styles.landingBadges)}>
          <OnlineBadge count={online} />
          <ConnectedFriends friends={friends} onHover={onFriendHover} />
        </span>
        <span className={styles.landingFooter}>
          <MaskIcon src={homeIcon} size={24} className={styles.homeIcon} />
          <span className={styles.landingTitle}>{title}</span>
        </span>
        <JumpInButton size="large" className={styles.landingJumpIn} disabled={disabled} onClick={onJumpIn} label="Jump In" />
      </div>
    </div>
  )
}

/** A place: its picture, name and creator; hovering raises the Jump In button. */
export function PlaceCard({
  title,
  creator,
  image,
  online,
  friends,
  narrow,
  disabled,
  onOpen,
  onJumpIn,
  onFriendHover
}: {
  title: string
  creator: string | null
  image: string | null
  online: number | null
  friends: ConnectedFriend[]
  narrow?: boolean
  disabled: boolean
  onOpen: () => void
  onJumpIn: () => void
  onFriendHover?: (friend: ConnectedFriend | null, e?: React.MouseEvent) => void
}): React.JSX.Element {
  const creatorName = useCreatorName(creator)
  return (
    <div className={cx(styles.place, narrow && styles.narrow)}>
      <Thumb src={image} fallback={defaultPlace} className={styles.zoomThumb} />
      <span className={cx(styles.gradient, styles.placeGradient)} />
      <button type="button" className={styles.cover} aria-label={`Open ${title}`} onClick={onOpen} />
      <span className={cx(styles.badges, styles.placeBadges)}>
        <OnlineBadge count={online} />
        <ConnectedFriends friends={friends} onHover={onFriendHover} />
      </span>
      <span className={styles.placeFooter}>
        <span className={styles.placeTitle}>{title}</span>
        {creatorName && <span className={styles.placeCreator}>{creatorName}</span>}
      </span>
      <JumpInButton size="small" className={styles.placeJumpIn} disabled={disabled} onClick={onJumpIn} label={`Jump in to ${title}`} />
    </div>
  )
}

/** A live event: LIVE, the people there, the title and host; hovering raises Jump In. */
export function LiveEventCard({
  title,
  host,
  image,
  attendees,
  friends,
  disabled,
  onOpen,
  onJumpIn,
  onFriendHover
}: {
  title: string
  host: string | null
  image: string | null
  attendees: number
  friends: ConnectedFriend[]
  disabled: boolean
  onOpen: () => void
  onJumpIn: (() => void) | null
  onFriendHover?: (friend: ConnectedFriend | null, e?: React.MouseEvent) => void
}): React.JSX.Element {
  return (
    <div className={styles.liveEvent}>
      <span className={styles.liveShadow} aria-hidden="true" />
      <div className={styles.liveBody}>
        <Thumb src={image} fallback={defaultPlace} className={styles.zoomThumb} />
        <span className={cx(styles.gradient, styles.liveGradient)} />
        <button type="button" className={styles.cover} aria-label={`Open ${title}`} onClick={onOpen} />
        <span className={cx(styles.badges, styles.liveBadges)}>
          <span className={styles.liveBadge}>
            <MaskIcon src={liveIcon} size={16} />
            LIVE
          </span>
          <OnlineBadge count={attendees} />
          <ConnectedFriends friends={friends} onHover={onFriendHover} />
        </span>
        <span className={styles.liveTitle}>{title}</span>
        {host && <span className={styles.liveHost}>By {host}</span>}
        {onJumpIn && <JumpInButton size="large" className={styles.liveJumpIn} disabled={disabled} onClick={onJumpIn} label={`Jump in to ${title}`} />}
      </div>
    </div>
  )
}

/** An upcoming event: when it starts and, on hover, interested / add to calendar / share. */
export function UpcomingEventCard({
  title,
  host,
  image,
  startsIn,
  interested,
  onOpen,
  onInterested,
  onCalendar,
  onShare
}: {
  title: string
  host: string | null
  image: string | null
  startsIn: string
  interested: boolean
  onOpen: () => void
  onInterested: () => void
  onCalendar: () => void
  onShare: (anchor: HTMLElement) => void
}): React.JSX.Element {
  return (
    <div className={cx(styles.upcoming, interested && styles.interested)}>
      <span className={styles.upcomingThumbFrame}>
        <Thumb src={image} fallback={defaultPlace} className={styles.zoomThumb} />
      </span>
      <button type="button" className={styles.cover} aria-label={`Open ${title}`} onClick={onOpen} />
      <span className={styles.upcomingInfo}>
        <span className={styles.upcomingTexts}>
          <span className={styles.upcomingTitle}>{title}</span>
          {host && (
            <span className={styles.upcomingHostRow}>
              <span className={styles.upcomingHostPrefix}>by</span>
              <span className={styles.upcomingHost}>{host}</span>
            </span>
          )}
        </span>
        <span className={styles.schedule}>
          <MaskIcon src={timeIcon} size={20} />
          <span className={styles.scheduleText}>{startsIn}</span>
        </span>
      </span>
      <span className={styles.actions}>
        <button type="button" className={styles.action} aria-label={interested ? 'Not interested' : 'Interested'} aria-pressed={interested} onClick={onInterested}>
          <img src={interested ? reminderOn : reminderOff} alt="" width={24} height={24} />
        </button>
        <button type="button" className={styles.action} aria-label="Add to calendar" onClick={onCalendar}>
          <img src={addToCalendar} alt="" width={24} height={24} />
        </button>
        <button type="button" className={styles.action} aria-label="Share" onClick={(e) => onShare(e.currentTarget)}>
          <img src={shareIcon} alt="" width={24} height={24} />
        </button>
      </span>
    </div>
  )
}

export type FriendPresence = 'online' | 'away' | 'offline'

/** An online friend: picture with presence, name, where they are; a joinable friend shows JOIN on hover. */
export function FriendCard({
  name,
  walletTag,
  verified,
  color,
  picture,
  presence,
  location,
  joinable,
  onOpen,
  onJoin
}: {
  name: string
  walletTag: string | null
  verified: boolean
  color: string
  picture: string | null
  presence: FriendPresence
  location: string
  joinable: boolean
  onOpen: () => void
  onJoin: () => void
}): React.JSX.Element {
  return (
    <div className={cx(styles.friend, joinable && styles.joinable, styles[presence])}>
      <button type="button" className={styles.cover} aria-label={`${name}'s profile`} onClick={onOpen} />
      <span className={styles.friendPictureLarge} style={{ backgroundImage: `url("${picture || defaultProfile}")` }} />
      <span className={styles.presence} />
      <span className={styles.friendName}>
        <span className={styles.friendNameText} style={{ color }}>
          {name}
        </span>
        {verified ? <img className={styles.verified} src={verifiedIcon} alt="Verified" width={12} height={12} /> : walletTag && <span className={styles.walletTag}>{walletTag}</span>}
      </span>
      <span className={styles.friendLocation}>
        <MaskIcon src={locationIcon} width={12} height={14} className={styles.locationIcon} />
        <span className={styles.friendLocationText}>{location}</span>
      </span>
      {joinable && (
        <button type="button" className={styles.join} aria-label={`Join ${name}`} onClick={onJoin}>
          <span className={styles.joinText}>JOIN</span>
          <MaskIcon src={jumpIn} size={16} />
        </button>
      )}
    </div>
  )
}

/** A community the player belongs to; hovering offers VIEW. */
export function CommunityCard({
  title,
  image,
  isPrivate,
  members,
  narrow,
  onView
}: {
  title: string
  image: string | null
  isPrivate: boolean
  members: string
  narrow?: boolean
  onView: () => void
}): React.JSX.Element {
  return (
    <div className={cx(styles.community, narrow && styles.narrow)}>
      <Thumb src={image} fallback={defaultCommunity} className={styles.zoomThumb} />
      <span className={cx(styles.gradient, styles.communityGradient)} />
      <span className={styles.communityFooter}>
        <span className={styles.communityTitle}>{title}</span>
        <span className={styles.membersRow}>
          <MaskIcon src={isPrivate ? privateIcon : publicIcon} size={10} className={styles.visibilityIcon} />
          <span className={styles.membersText}>{isPrivate ? 'Private' : 'Public'}</span>
          <span className={styles.separator} />
          <span className={cx(styles.membersText, styles.members)}>{members}</span>
        </span>
      </span>
      <button type="button" className={styles.view} aria-label={`View ${title}`} onClick={onView}>
        VIEW
      </button>
    </div>
  )
}

/** One of the player's photos. Square corners. */
export function GalleryCard({ image, label, narrow, onOpen }: { image: string; label: string; narrow?: boolean; onOpen: () => void }): React.JSX.Element {
  return (
    <div className={cx(styles.photo, narrow && styles.narrow)}>
      <span className={cx(styles.outline, styles.photoOutline)} aria-hidden="true" />
      <div className={styles.photoBody}>
        <Thumb src={image} fallback={defaultPlace} className={styles.zoomThumb} />
        <button type="button" className={styles.cover} aria-label={label} onClick={onOpen} />
      </div>
    </div>
  )
}

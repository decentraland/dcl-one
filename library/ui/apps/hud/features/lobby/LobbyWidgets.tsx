// The lobby's widget sections, one per widget of the layout: the landing card, live and upcoming
// events, online friends, the recent and recommended places, the player's communities and photos,
// and Go to. Each fills the cells its placement spans; LobbyHome places them.

import { useEffect, useRef, useState } from 'react'
import arrowRight from '../../assets/lobby/arrow-right.png'
import communitiesBanner from '../../assets/lobby/communities-banner-img.png'
import friendsBannerGlow from '../../assets/lobby/friends-banner-background.png'
import friendsBanner from '../../assets/lobby/friends-banner-img.png'
import friendsIcon from '../../assets/lobby/friends-icon.png'
import galleryBanner from '../../assets/lobby/gallery-banner-image.png'
import goIcon from '../../assets/lobby/jump-in-rounded.png'
import locationIcon from '../../assets/lobby/location-icn.png'
import warningIcon from '../../assets/lobby/warning-icn.png'
import { MaskIcon, Spinner } from '../../design'
import { userColor } from '@explorer-core/lib/identity'
import { isDestination } from '@explorer-core/features/lobby/goto'
import { eventAttendees, startsInText, type LobbyEvent } from '@explorer-core/features/lobby/lobbyEvents'
import { placeOnline, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
import { widgetCardCount, type LobbyWidgetId } from '@explorer-core/features/lobby/layout'
import type { CardOrigin } from '@explorer-core/features/lobby/lobbyAnalytics'
import { placeCreator } from '@explorer-core/features/places/placesApi'
import type { Community, Friend, GalleryPhoto } from '@explorer-core/engine/protocol'
import { CommunityCard, FriendCard, GalleryCard, LandingCard, LiveEventCard, PlaceCard, UpcomingEventCard, type ConnectedFriend } from './LobbyCards'
import { LobbyRail } from './LobbyRail'
import type { FriendLocation, LandingState } from './useLobbyData'
import styles from './LobbyWidgets.module.css'

const cx = (...names: Array<string | false | null | undefined>): string => names.filter(Boolean).join(' ')

/** Everything the widgets read and do; LobbyHome builds it once per render. */
export interface LobbyWidgetContext {
  /** The layout is being edited: the sections show, but VIEW ALL steps aside for the frames. */
  editing?: boolean
  welcomeName: string | null
  landing: LandingState & { title: string }
  recent: LobbyPlace[] | null
  recommended: LobbyPlace[] | null
  live: LobbyEvent[]
  upcoming: LobbyEvent[]
  eventsLoaded: boolean
  online: Friend[]
  /** No friend list yet, or the social service is still starting. */
  friendsLoading: boolean
  totalFriends: number
  locations: Map<string, FriendLocation>
  communities: Community[] | null
  photos: GalleryPhoto[] | null
  /** Picks wait while sign-in is finishing. */
  busy: boolean
  /** The online friends at a set of wallets, for a card's pictures. */
  friendsAt: (addresses: string[] | null | undefined) => ConnectedFriend[]
  onFriendHover: (friend: ConnectedFriend | null, e?: React.MouseEvent) => void
  openLanding: () => void
  jumpLanding: () => void
  openPlace: (p: LobbyPlace, origin: CardOrigin) => void
  jumpPlace: (p: LobbyPlace, origin: CardOrigin) => void
  openEvent: (e: LobbyEvent, origin: CardOrigin) => void
  jumpEvent: (e: LobbyEvent, origin: CardOrigin) => (() => void) | null
  toggleInterest: (e: LobbyEvent) => void
  addToCalendar: (e: LobbyEvent) => void
  shareEvent: (e: LobbyEvent, anchor: HTMLElement) => void
  openFriend: (f: Friend) => void
  joinFriend: (f: Friend) => void
  viewCommunity: (c: Community) => void
  openPhoto: (index: number) => void
  goTo: (text: string) => Promise<boolean>
  viewAll: (target: 'recent' | 'recommended' | 'live' | 'upcoming' | 'communities' | 'gallery') => void
}

function Header({ title, count, onViewAll, variant }: { title: string; count?: string | null; onViewAll?: () => void; variant: 'live' | 'upcoming' | 'places' | 'friends' }): React.JSX.Element {
  return (
    <div className={cx(styles.header, styles[`header_${variant}`])}>
      <h2 className={cx(styles.title, count != null && styles.titleShrink)}>{title}</h2>
      {count != null && <span className={styles.count}>{count}</span>}
      {onViewAll && (
        <button type="button" className={styles.viewAll} onClick={onViewAll}>
          <span className={styles.viewAllLabel}>VIEW ALL</span>
          <span className={styles.viewAllIcon}>
            <MaskIcon src={arrowRight} width={7} height={12} />
          </span>
        </button>
      )}
    </div>
  )
}

function Skeleton({ bones, kind }: { bones: number; kind: 'friend' | 'card' | 'photo' }): React.JSX.Element {
  return (
    <div className={cx(styles.skeleton, styles[`skeleton_${kind}`])} aria-busy="true" aria-label="Loading">
      {Array.from({ length: bones }, (_, i) => (
        <span key={i} className={styles.bone} />
      ))}
    </div>
  )
}

const compact = (n: number): string => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1).replace(/\.0$/, '')}k` : String(n))

function Landing({ ctx }: { ctx: LobbyWidgetContext }): React.JSX.Element {
  const l = ctx.landing
  return (
    <>
      <h1 className={styles.welcome}>{ctx.welcomeName ? `Welcome ${ctx.welcomeName}!` : 'Welcome!'}</h1>
      <LandingCard
        title={l.title}
        image={l.place?.image ?? null}
        online={l.online}
        friends={ctx.friendsAt(l.connected)}
        canOpen={l.place != null}
        disabled={ctx.busy || l.dest == null}
        onOpen={ctx.openLanding}
        onJumpIn={ctx.jumpLanding}
        onFriendHover={ctx.onFriendHover}
      />
    </>
  )
}

function LiveEvents({ ctx }: { ctx: LobbyWidgetContext }): React.JSX.Element | null {
  if (ctx.live.length === 0) return null
  return (
    <>
      <Header title="Live Events" variant="live" onViewAll={ctx.editing ? undefined : () => ctx.viewAll('live')} />
      <LobbyRail perPage={1} label="Live Events" className={styles.liveRail}>
        {ctx.live.map((e) => (
          <LiveEventCard
            key={e.id}
            title={e.name}
            host={e.user_name ?? null}
            image={e.image ?? null}
            attendees={eventAttendees(e)}
            friends={ctx.friendsAt(e.connected_addresses)}
            disabled={ctx.busy}
            onOpen={() => ctx.openEvent(e, 'live_events')}
            onJumpIn={ctx.jumpEvent(e, 'live_events')}
            onFriendHover={ctx.onFriendHover}
          />
        ))}
      </LobbyRail>
    </>
  )
}

function UpcomingEvents({ ctx }: { ctx: LobbyWidgetContext }): React.JSX.Element | null {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000)
    return () => clearInterval(id)
  }, [])
  if (ctx.upcoming.length === 0) return null
  return (
    <>
      <Header title="Upcoming Events" variant="upcoming" onViewAll={ctx.editing ? undefined : () => ctx.viewAll('upcoming')} />
      <LobbyRail perPage={1} label="Upcoming Events" className={styles.upcomingRail}>
        {ctx.upcoming.map((e) => (
          <UpcomingEventCard
            key={e.id}
            title={e.name}
            host={e.user_name ?? null}
            image={e.image ?? null}
            startsIn={startsInText(e, now)}
            interested={e.attending === true}
            onOpen={() => ctx.openEvent(e, 'upcoming_events')}
            onInterested={() => ctx.toggleInterest(e)}
            onCalendar={() => ctx.addToCalendar(e)}
            onShare={(anchor) => ctx.shareEvent(e, anchor)}
          />
        ))}
      </LobbyRail>
    </>
  )
}

function FriendsOnline({ ctx }: { ctx: LobbyWidgetContext }): React.JSX.Element {
  const n = ctx.online.length
  return (
    <>
      <Header title="Friends" variant="friends" count={n > 0 ? `${n} Online` : null} />
      {n > 0 ? (
        <LobbyRail perPage={4} label="Friends online" className={styles.friendsRail}>
          {ctx.online.map((f) => {
            const location = ctx.locations.get(f.address.toLowerCase())
            const joinable = location != null && location.where.kind !== 'unknown'
            return (
              <FriendCard
                key={f.address}
                name={f.name}
                walletTag={f.claimed ? null : `#${f.address.slice(-4)}`}
                verified={f.claimed === true}
                color={userColor(f.address, f.name, f.claimed, f.nameColor)}
                picture={f.picture ?? null}
                presence={f.status === 'away' ? 'away' : f.status === 'online' ? 'online' : 'offline'}
                location={location?.title ?? 'Locating…'}
                joinable={joinable}
                onOpen={() => ctx.openFriend(f)}
                onJoin={() => ctx.joinFriend(f)}
              />
            )
          })}
        </LobbyRail>
      ) : ctx.friendsLoading ? (
        <Skeleton bones={4} kind="friend" />
      ) : ctx.totalFriends === 0 ? (
        <div className={cx(styles.banner, styles.bannerFriends)}>
          <img className={styles.friendsGlow} src={friendsBannerGlow} alt="" />
          <img className={styles.friendsImage} src={friendsBanner} alt="" />
          <div className={styles.bannerText}>
            <p className={cx(styles.bannerTitle, styles.bannerTitleSmall)}>Time to make some friends!</p>
            <p className={cx(styles.bannerBody, styles.bannerBodySmall)}>
              Visit someone&apos;s profile or click their name in Chat to add them as a friend. They&apos;ll show up here when they&apos;re online.
            </p>
          </div>
        </div>
      ) : (
        <div className={cx(styles.banner, styles.bannerFriendsOnline)}>
          <MaskIcon src={friendsIcon} size={34} className={styles.bannerIcon} />
          <p className={styles.caption}>No Friends Online</p>
        </div>
      )}
    </>
  )
}

function Places({ ctx, widget, kind }: { ctx: LobbyWidgetContext; widget: LobbyWidgetId; kind: 'recent' | 'recommended' }): React.JSX.Element | null {
  const list = kind === 'recent' ? ctx.recent : ctx.recommended
  if (list == null || list.length === 0) return null
  const narrow = widget.endsWith('4By2')
  const origin: CardOrigin = kind
  const cards = (kind === 'recent' ? list.slice(0, widgetCardCount(widget)) : list).map((p) => (
    <PlaceCard
      key={p.id}
      title={p.title}
      creator={placeCreator(p) || null}
      image={p.image}
      online={placeOnline(p)}
      friends={ctx.friendsAt(p.connected_addresses)}
      narrow={narrow}
      disabled={ctx.busy}
      onOpen={() => ctx.openPlace(p, origin)}
      onJumpIn={() => ctx.jumpPlace(p, origin)}
      onFriendHover={ctx.onFriendHover}
    />
  ))
  return (
    <>
      <Header title={kind === 'recent' ? 'Jump Back In' : 'Recommended Places'} variant="places" onViewAll={ctx.editing ? undefined : () => ctx.viewAll(kind)} />
      {kind === 'recent' ? (
        <div className={styles.placesRow}>{cards}</div>
      ) : (
        <LobbyRail perPage={widgetCardCount(widget)} arrowsMoveOneCard label="Recommended Places" className={cx(styles.placesRail, narrow && styles.narrowRail)}>
          {cards}
        </LobbyRail>
      )}
    </>
  )
}

function CommunitiesBanner({ narrow }: { narrow: boolean }): React.JSX.Element {
  return (
    <div className={cx(styles.banner, styles.bannerWide, styles.bannerCommunities, narrow && styles.bannerNarrow)}>
      <img className={styles.communitiesImage} src={communitiesBanner} alt="" />
      <div className={styles.bannerText}>
        <p className={cx(styles.bannerTitle, styles.bannerTitleNowrap)}>✨ Discover and Join Communities ✨</p>
        <div className={styles.tags}>
          <span className={styles.tag}>💬 Chat in a shared channel</span>
          <span className={styles.tag}>🎯 Connect over shared interests</span>
          <span className={styles.tag}>🔔 Get notified about community events</span>
        </div>
      </div>
    </div>
  )
}

function Communities({ ctx, widget }: { ctx: LobbyWidgetContext; widget: LobbyWidgetId }): React.JSX.Element {
  const narrow = widget.endsWith('4By2')
  const perPage = widgetCardCount(widget)
  return (
    <>
      <Header title="My Communities" variant="places" onViewAll={ctx.editing ? undefined : () => ctx.viewAll('communities')} />
      {ctx.communities == null ? (
        <Skeleton bones={perPage} kind="card" />
      ) : ctx.communities.length === 0 ? (
        <CommunitiesBanner narrow={narrow} />
      ) : (
        <LobbyRail perPage={perPage} arrowsMoveOneCard label="My Communities" className={cx(styles.placesRail, narrow && styles.narrowRail)}>
          {ctx.communities.map((c) => (
            <CommunityCard
              key={c.id}
              title={c.name}
              image={c.thumbnail ?? null}
              isPrivate={c.privacy === 'private'}
              members={`${compact(c.membersCount)} Members`}
              narrow={narrow}
              onView={() => ctx.viewCommunity(c)}
            />
          ))}
        </LobbyRail>
      )}
    </>
  )
}

function Gallery({ ctx, widget }: { ctx: LobbyWidgetContext; widget: LobbyWidgetId }): React.JSX.Element {
  const narrow = widget.endsWith('4By2')
  const perPage = widgetCardCount(widget)
  return (
    <>
      <Header title="Gallery" variant="places" onViewAll={ctx.editing ? undefined : () => ctx.viewAll('gallery')} />
      {ctx.photos == null ? (
        <Skeleton bones={perPage} kind="photo" />
      ) : ctx.photos.length === 0 ? (
        <div className={cx(styles.banner, styles.bannerWide, styles.bannerGallery, narrow && styles.bannerNarrow)}>
          <img className={styles.galleryImage} src={galleryBanner} alt="" />
          <div className={styles.bannerText}>
            <p className={styles.bannerTitle}>You Haven&apos;t Taken Any Photos Yet</p>
            <p className={styles.bannerBody}>Your photos will appear here. Jump in now and start capturing moments with your friends using the Camera!</p>
          </div>
        </div>
      ) : (
        <LobbyRail perPage={perPage} arrowsMoveOneCard label="Gallery" className={cx(styles.placesRail, styles.galleryRail, narrow && styles.narrowRail)}>
          {ctx.photos.map((p, i) => (
            <GalleryCard key={p.id} image={p.thumbnailUrl ?? p.url} label={`Photo ${i + 1}`} narrow={narrow} onOpen={() => ctx.openPhoto(i)} />
          ))}
        </LobbyRail>
      )}
    </>
  )
}

function GoTo({ ctx }: { ctx: LobbyWidgetContext }): React.JSX.Element {
  const [text, setText] = useState('')
  const [invalid, setInvalid] = useState(false)
  const [checking, setChecking] = useState(false)
  const [focused, setFocused] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const submit = async (): Promise<void> => {
    const destination = text.trim()
    if (destination === '' || checking) return
    if (!isDestination(destination)) {
      setInvalid(true)
      return
    }
    setChecking(true)
    const ok = await ctx.goTo(destination).catch(() => false)
    setChecking(false)
    if (ok) setText('')
    else setInvalid(true)
  }
  return (
    <>
      <p className={styles.gotoTitle}>Jump to any Genesis City coordinate or a World</p>
      <div className={cx(styles.goto, focused && styles.gotoFocused, invalid && styles.gotoInvalid)} onClick={() => input.current?.focus()}>
        <MaskIcon src={locationIcon} width={13} height={15} className={styles.gotoIcon} />
        <span className={styles.gotoCommand}>/goto</span>
        <input
          ref={input}
          className={styles.gotoInput}
          value={text}
          placeholder="e.g. 3,-5 or mana.dcl.eth"
          aria-label="Go to a coordinate or a World"
          aria-invalid={invalid}
          readOnly={checking}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          onChange={(e) => {
            setText(e.target.value)
            setInvalid(false)
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              void submit()
            }
          }}
        />
        {checking ? (
          <span className={styles.gotoSpinner}><Spinner size={26} /></span>
        ) : (
          text.trim() !== '' && (
            <button type="button" className={styles.gotoGo} aria-label="Go" onClick={() => void submit()}>
              <img src={goIcon} alt="" width={26} height={26} />
            </button>
          )
        )}
        {invalid && (
          <span className={styles.gotoError} role="alert">
            <MaskIcon src={warningIcon} size={12} />
            Coordinates or World name doesn&apos;t exist
          </span>
        )}
      </div>
    </>
  )
}

/** The section for a widget, filling its cells. */
export function LobbyWidget({ widget, ctx }: { widget: LobbyWidgetId; ctx: LobbyWidgetContext }): React.JSX.Element | null {
  switch (widget) {
    case 'Landing4By3':
      return <Landing ctx={ctx} />
    case 'LiveEvents3By3':
      return <LiveEvents ctx={ctx} />
    case 'UpcomingEvents3By2':
      return <UpcomingEvents ctx={ctx} />
    case 'FriendsOnline4By2':
      return <FriendsOnline ctx={ctx} />
    case 'JumpBackIn7By2':
    case 'JumpBackIn4By2':
      return <Places ctx={ctx} widget={widget} kind="recent" />
    case 'RecommendedPlaces7By2':
    case 'RecommendedPlaces4By2':
      return <Places ctx={ctx} widget={widget} kind="recommended" />
    case 'Communities7By2':
    case 'Communities4By2':
      return <Communities ctx={ctx} widget={widget} />
    case 'Gallery7By2':
    case 'Gallery4By2':
      return <Gallery ctx={ctx} widget={widget} />
    case 'GoTo4By1':
      return <GoTo ctx={ctx} />
  }
}

/** Whether a widget shows anything right now: events and places hide while they have nothing to list. */
export function widgetShown(widget: LobbyWidgetId, ctx: LobbyWidgetContext): boolean {
  if (widget === 'LiveEvents3By3') return ctx.live.length > 0
  if (widget === 'UpcomingEvents3By2') return ctx.upcoming.length > 0
  if (widget.startsWith('JumpBackIn')) return (ctx.recent?.length ?? 0) > 0
  if (widget.startsWith('RecommendedPlaces')) return (ctx.recommended?.length ?? 0) > 0
  return true
}

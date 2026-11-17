// The lobby: shown after sign-in, before entering the world (and reopened from the menu in-world).
// The stage and the avatar are drawn by the engine behind this transparent page.

import { useCallback, useEffect, useRef, useState } from 'react'
import backdrop from '../../assets/lobby/background.jpg'
import podium from '../../assets/lobby/podium.png'
import logo from '../../assets/lobby/logo.png'
import { Close } from '../../design'
import {
  GeneralCalendar,
  GeneralCheck,
  GeneralEditFilled,
  GeneralHistory,
  GeneralHome,
  GeneralLeftArrow,
  GeneralLive,
  GeneralMapPin,
  GeneralStar
} from '../../../../foundations/icons/general'
import type { IconProps } from '../../../../foundations/icons/types'
import { NavbarEvents, NavbarFriends, NavbarMap, NavbarNotifications, NavbarPlaces } from '../../../../foundations/icons/navbar'
import { keyHintFor, useBindingsSnapshot } from '../../lib/bindingLabels'
import { BASE, podiumRect } from '@explorer-core/engine/lobbyStage'
import { clickButton } from '@explorer-core/lib/analytics'
import { userColor } from '@explorer-core/lib/identity'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import { clip } from '../backpack/BackpackModal'
import { holeClip } from '@explorer-core/lib/holeClip'
import { EngineViewport } from '../engine/EngineViewport'
import { eventDestination, eventStartAt, fetchEvents, type DclEvent, type EventDestination } from '@explorer-core/features/events/eventsApi'
import { friendsAtEvent, openShareMenu } from '../events/EventCard'
import type { EventLinkSource } from '../events/eventFormat'
import { useEventActions } from '../events/useEventActions'
import { eventInfo, openDestinationCard, placeInfo } from '../places/DestinationCard'
import { placeCreator, placePlayers, placeTeleport, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { openPassport } from '../profile/Passport'
import { useSession } from '../session/SessionContext'
import type { Destination } from '../session/useEngineSession'
import { FriendCard, FriendsEmpty, FriendsNoneOnline, LandingCard, LiveEventCard, Pages, PlaceCard, SectionHead, UpcomingEventCard } from './LobbyCards'
import { LobbyGoto, type GotoTarget } from './LobbyGoto'
import { LobbyProfile } from './LobbyProfile'
import { useAvatarTurn } from './useAvatarTurn'
import { canExit, exitApplication } from '../gate/exitApplication'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { fetchHighlighted, fetchHomePlace, fetchRecents } from '@explorer-core/features/lobby/lobbyApi'
import styles from './Lobby.module.css'
import type { EngineViewportRegion } from '@explorer-core/engine/protocol'

type Rect = { x: number; y: number; width: number; height: number }

// A failed fetch shows its section empty (hidden), as on parity.
function useLoad<T>(load: () => Promise<T>, fallback: T): { data: T; loading: boolean } {
  const [state, setState] = useState<{ data: T; loading: boolean }>({ data: fallback, loading: true })
  const fallbackRef = useRef(fallback)
  useEffect(() => {
    let live = true
    setState({ data: fallbackRef.current, loading: true })
    load()
      .then((data) => live && setState({ data, loading: false }))
      .catch(() => live && setState({ data: fallbackRef.current, loading: false }))
    return () => {
      live = false
    }
  }, [load])
  return state
}

const UPCOMING_SHOWN = 10
const LIVE_SHOWN = 10

const eventPeople = (e: DclEvent): number => e.connected_addresses?.length ?? 0

/** The events on now with who is there, busiest first; equal ones keep the service's order. */
async function fetchLive(): Promise<DclEvent[]> {
  const events = await fetchEvents('live', { connected: true })
  return events
    .filter((e) => e.live !== false)
    .sort((a, b) => eventPeople(b) - eventPeople(a))
    .slice(0, LIVE_SHOWN)
}

/** The next events to start, soonest first. A series counts from its next date, as the list does. */
async function fetchUpcoming(): Promise<DclEvent[]> {
  const now = Date.now()
  const startsAt = (e: DclEvent): number => new Date(eventStartAt(e)).getTime()
  const events = await fetchEvents('upcoming')
  return events
    .filter((e) => !e.live && startsAt(e) > now)
    .sort((a, b) => startsAt(a) - startsAt(b))
    .slice(0, UPCOMING_SHOWN)
}

/** What the calendar and share links read of an event from the public list. */
function eventLinks(e: DclEvent): EventLinkSource {
  return {
    id: e.id,
    name: e.name,
    description: e.description ?? null,
    live: e.live === true,
    world: e.world === true,
    server: e.server ?? null,
    x: e.x,
    y: e.y,
    next_start_at: eventStartAt(e),
    next_finish_at: e.next_finish_at ?? e.finish_at ?? null,
    duration: e.duration ?? null,
    highlighted: e.highlighted === true
  }
}

// The widgets a player can take off the lobby (Customize Lobby), remembered on this device.
const WIDGETS = {
  home: 'Welcome',
  goto: 'Go To',
  friends: 'Friends',
  live: 'Live Events',
  events: 'Upcoming Events',
  recents: 'Jump Back In',
  recommended: 'Recommended Places'
} as const
type Widget = keyof typeof WIDGETS
// a widget's icon in its Customize Lobby row
const WIDGET_ICONS: Record<Widget, (props: IconProps) => React.JSX.Element> = {
  home: GeneralHome,
  goto: GeneralMapPin,
  friends: NavbarFriends,
  live: GeneralLive,
  events: GeneralCalendar,
  recents: GeneralHistory,
  recommended: GeneralStar
}
const WIDGET_IDS = Object.keys(WIDGETS) as Widget[]

function readHidden(): Widget[] {
  try {
    const raw = JSON.parse(getPref(PREF.lobbyHiddenWidgets) ?? '[]') as unknown
    return Array.isArray(raw) ? WIDGET_IDS.filter((w) => raw.includes(w)) : []
  } catch {
    return []
  }
}

function writeHidden(hidden: Widget[]): void {
  setPref(PREF.lobbyHiddenWidgets, JSON.stringify(hidden))
}

// The Backpack's preview runs past its modal's edge; only the part inside the modal is a hole.
function clipToModal(rect: Rect | null): Rect | null {
  const frame = document.querySelector('[role="dialog"][aria-label="Backpack"]')?.getBoundingClientRect()
  return rect == null || frame == null ? rect : clip(rect, frame)
}

const BASE_COLOR = `rgb(${BASE.r}, ${BASE.g}, ${BASE.b})`

// Until the engine has drawn the stage (and for good without an engine), the page paints the same
// backdrop and podium, so the lobby never shows a half-loaded stage. The snapshot `body` stands in
// for the avatar where the engine may still be downloading.
export function StandInStage({ hidden, body }: { hidden: boolean; body?: string }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ w: 1920, h: 1080 })
  useEffect(() => {
    const el = ref.current
    if (el == null) return
    const ro = new ResizeObserver(() => setSize({ w: el.offsetWidth, h: el.offsetHeight }))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  return (
    <div ref={ref} className={`${styles.standIn} ${hidden ? styles.standInHidden : ''}`.trim()} style={{ background: BASE_COLOR }} aria-hidden="true">
      <img className={styles.standInBackdrop} src={backdrop} alt="" />
      <img className={styles.standInPodium} src={podium} alt="" style={podiumRect(size.w, size.h)} />
      {body && <img className={styles.standInAvatar} src={body} alt="" draggable={false} />}
    </div>
  )
}

/** The hover chip's mouse: the left button lit. */
function MouseLeft(): React.JSX.Element {
  return (
    <svg viewBox="0 0 22 22" width="28" height="28" fill="none" aria-hidden="true">
      <path d="M11 4.500h1.200a4.800 4.800 0 0 1 4.800 4.800v4.400a5.300 5.300 0 0 1-10.600 0V11" stroke="#fff" strokeWidth="1.800" strokeLinecap="round" />
      <path d="M11 4.500v6.500H6.400V9.300A4.800 4.800 0 0 1 11 4.500Z" fill="#ff7439" stroke="#ff7439" strokeWidth="1.800" strokeLinejoin="round" />
      <path d="M6.400 11H17" stroke="#fff" strokeWidth="1.800" />
      <path d="M3.200 6.200A6.600 6.600 0 0 1 6.600 2.400" stroke="#fff" strokeWidth="1.500" strokeLinecap="round" />
    </svg>
  )
}

export function LobbyHome({
  onPick,
  onClose,
  setEngineViewport
}: {
  onPick: (dest: Destination) => void
  /** In-world only: the lobby can be closed back to the world (at startup Jump In is the way out). */
  onClose?: () => void
  setEngineViewport: (region: EngineViewportRegion, rect: Rect | null, dpr?: number) => void
}): React.JSX.Element {
  const session = useSession()
  const profile = session.profile.data
  const home = session.homeScene()
  const homeRealm = home?.realm ?? null
  const homeParcel = home?.parcel
  const loadHome = useCallback(
    () => homeParcel == null ? Promise.resolve(null) : fetchHomePlace({ realm: homeRealm, parcel: homeParcel }),
    [homeRealm, homeParcel]
  )
  const landing = useLoad(loadHome, null as DiscoverPlace | null)
  const recents = useLoad(fetchRecents, [] as DiscoverPlace[])
  const recommended = useLoad(fetchHighlighted, [] as DiscoverPlace[])
  const liveEvents = useLoad(fetchLive, [] as DclEvent[])
  const upcoming = useLoad(fetchUpcoming, [] as DclEvent[])
  const eventActions = useEventActions()
  const online = session.friends.list
    .filter((f) => f.status !== 'offline')
    .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }))
  const rootRef = useRef<HTMLDivElement>(null)
  const [tooltip, setTooltip] = useState(false)
  // A page's key shows after its name where the key acts: the world's hotkeys wait for the world,
  // so the startup lobby shows none.
  const bindings = useBindingsSnapshot()
  const keyHint = (action: string): React.ReactNode => {
    const key = onClose ? keyHintFor(bindings, action) : undefined
    return key && <span className={styles.key} aria-hidden="true">[{key}]</span>
  }
  const avatarTurn = useAvatarTurn(session.turnLobbyAvatar)
  const [hidden, setHidden] = useState(readHidden)
  // Customize Lobby: the widgets taken off so far, applied on Save
  const [draft, setDraft] = useState<Widget[] | null>(null)
  const editing = draft != null
  // a pick releases the held world, so none until sign-in has finished
  const waiting = session.login.busy
  // a detail card stays open across renders: its Jump In reads the sign-in state of the press
  const waitingRef = useRef(waiting)
  waitingRef.current = waiting
  const openPage = (button: string, open: () => void) => (): void => {
    clickButton(button, 'LOBBY')
    open()
  }
  const openMap = openPage('MAP', () => session.map.toggle())
  const openPlaces = openPage('PLACES', () => session.places.toggle())
  const openEvents = openPage('EVENTS', () => session.events.toggle('start_menu'))
  const pickPlace = (p: DiscoverPlace): void => {
    clickButton('JUMP_IN', 'LOBBY', { kind: 'place', place_id: p.id })
    onPick(placeTeleport(p))
  }
  const placeProps = (p: DiscoverPlace) => ({
    title: p.title,
    creator: placeCreator(p),
    image: p.image,
    count: placePlayers(p),
    disabled: waiting,
    onJumpIn: () => pickPlace(p),
    onOpen: () => openDestinationCard(placeInfo(p), () => !waitingRef.current && pickPlace(p))
  })
  const pickEvent = (e: DclEvent, dest: EventDestination): void => {
    clickButton('JUMP_IN', 'LOBBY', { kind: 'event', event_id: e.id })
    onPick(dest.kind === 'world' ? { kind: 'world', realm: dest.realm } : dest)
  }
  const landingPlace = landing.data
  const jumpLanding = (): void => {
    clickButton('JUMP_IN', 'LOBBY', { kind: 'landing', place_id: landingPlace?.id ?? homeParcel ?? '' })
    onPick(null)
  }
  const pickGoto = (target: GotoTarget): void => {
    clickButton('JUMP_IN', 'LOBBY', { kind: 'goto' })
    onPick(target)
  }
  const landingTitle = landing.data?.title ?? (landing.loading || home == null ? '' : home.realm ?? (home.parcel === '0,0' ? 'Genesis Plaza' : home.parcel))

  // Escape leaves the edit before anything under it
  useEffect(() => {
    if (editing) return registerCancelLayer(() => setDraft(null))
  }, [editing])

  const off = draft ?? hidden
  const shows = (w: Widget): boolean => !off.includes(w)
  // Friends stand under what the left column still shows above them
  const friendsPlace = shows('home')
    ? shows('goto') ? '' : styles.friendsUnderHome
    : shows('goto') ? styles.friendsUnderGoto : styles.friendsTop
  // Upcoming Events stands under Live Events, and in its place when nothing is live or the widget is
  // taken off; while the live list loads it keeps its place.
  const liveShown = shows('live') && (liveEvents.loading || liveEvents.data.length > 0)
  const changed = draft != null && (draft.length !== hidden.length || draft.some((w) => !hidden.includes(w)))
  const save = (): void => {
    if (draft == null) return
    writeHidden(draft)
    setHidden(draft)
    setDraft(null)
  }
  const widget = (className: string): { className: string } => ({ className: `${className} ${editing ? styles.widgetEditing : ''}`.trim() })
  const remove = (w: Widget): React.ReactNode =>
    editing && (
      <button type="button" className={styles.widgetRemove} aria-label={`Remove ${WIDGETS[w]}`} onClick={() => setDraft([...off, w])}>
        <Close size={20} />
      </button>
    )

  // Under the Backpack modal the lobby shows, but with a hole where the engine draws the
  // Backpack's avatar, which is behind it.
  const hole = session.backpack.open ? clipToModal(session.avatarPreviewRect) : null
  const scale = rootRef.current != null ? rootRef.current.getBoundingClientRect().width / rootRef.current.offsetWidth || 1 : 1
  const backpackHole =
    hole == null ? undefined : holeClip({ x: hole.x / scale, y: hole.y / scale, width: hole.width / scale, height: hole.height / scale })

  return (
    <div ref={rootRef} className={styles.root} style={backpackHole} data-lobby={onClose ? 'world' : 'startup'}>
      {/* the Backpack modal takes the avatar preview while it's open; the stage comes back after */}
      <div className={styles.stage}>{!session.backpack.open && <EngineViewport region="lobby" report={setEngineViewport} />}</div>
      <StandInStage hidden={session.lobbyStageReady && !session.backpack.open} />
      <div className={styles.scrim} aria-hidden="true" />

      <button
        type="button"
        className={styles.avatarHit}
        aria-label="Customize"
        disabled={!session.playerReady || editing}
        {...avatarTurn.handlers}
        onClick={(e) => {
          if (avatarTurn.wasDrag(e)) return
          clickButton('CUSTOMIZE', 'LOBBY')
          setTooltip(false)
          session.backpack.toggle()
        }}
        onMouseEnter={() => setTooltip(true)}
        onMouseLeave={() => setTooltip(false)}
      />

      {editing && <div className={styles.editDim} />}

      <header className={styles.header}>
        <div className={styles.headerLeft}>
          <img className={styles.brand} src={logo} alt="Decentraland" />
          <span className={styles.logoLobby}>Lobby</span>
          {editing ? (
            <button type="button" className={`${styles.pill} ${styles.back}`} onClick={() => setDraft(null)}>
              <GeneralLeftArrow size={32} aria-hidden="true" />
              Back <span className={styles.key}>[Esc]</span>
            </button>
          ) : (
            <button
              type="button"
              className={`${styles.pill} ${styles.round} ${styles.edit}`}
              aria-label="Customize lobby"
              onClick={() => {
                clickButton('CUSTOMIZE_LOBBY', 'LOBBY')
                setTooltip(false)
                setDraft(hidden)
              }}
            >
              <GeneralEditFilled size={24} />
            </button>
          )}
        </div>
        <div className={styles.headerRight}>
          <button type="button" className={styles.pill} onClick={openMap}>
            <NavbarMap size={32} />
            Map
            {keyHint('Map')}
          </button>
          <button type="button" className={styles.pill} onClick={openPlaces}>
            <NavbarPlaces size={32} />
            Places
            {keyHint('Places')}
          </button>
          <button type="button" className={styles.pill} onClick={openEvents}>
            <NavbarEvents size={32} />
            Events
            {keyHint('Events')}
          </button>
          <button
            type="button"
            className={`${styles.pill} ${styles.round}`}
            aria-label="Notifications"
            onClick={() => {
              clickButton('NOTIFICATIONS', 'LOBBY')
              session.notifications.toggle()
            }}
          >
            <NavbarNotifications size={32} />
          </button>
          <LobbyProfile
            name={profile?.name ?? ''}
            picture={profile?.picture}
            address={profile?.address}
            claimed={profile?.hasClaimedName === true}
            onViewProfile={() => profile && openPassport(profile.address)}
            onSettings={openPage('SETTINGS', () => session.settings.toggle())}
            onSignOut={onClose ? undefined : session.logout}
            onExit={onClose ?? (canExit() ? exitApplication : undefined)}
          />
          {onClose && (
            <button
              type="button"
              className={`${styles.pill} ${styles.round}`}
              aria-label="Close"
              onClick={() => {
                clickButton('CLOSE', 'LOBBY')
                onClose()
              }}
            >
              <Close size={12} />
            </button>
          )}
        </div>
      </header>

      {shows('home') && (
        <section {...widget(styles.quickJump)}>
          {remove('home')}
          <h1 className={styles.welcome}>{profile?.name ? `Welcome ${profile.name}` : 'Welcome'}</h1>
          <LandingCard
            title={landingTitle}
            creator={landing.data ? placeCreator(landing.data) : null}
            image={landing.data?.image ?? null}
            count={landing.data ? placePlayers(landing.data) : null}
            loading={landing.loading || home == null}
            disabled={waiting}
            onJumpIn={jumpLanding}
            onOpen={landingPlace ? () => openDestinationCard(placeInfo(landingPlace), () => !waitingRef.current && jumpLanding()) : undefined}
          />
        </section>
      )}

      {shows('goto') && (
        <section {...widget(`${styles.goto} ${shows('home') ? '' : styles.gotoTop}`.trim())}>
          {remove('goto')}
          <LobbyGoto disabled={waiting} onPick={pickGoto} />
        </section>
      )}

      {shows('friends') && (
        <section {...widget(`${styles.friends} ${friendsPlace}`.trim())}>
          {remove('friends')}
          <SectionHead title="Friends">{online.length > 0 && !editing && <span className={styles.friendsCount}>{online.length} Online</span>}</SectionHead>
          {online.length === 0 ? (
            session.friends.list.length === 0 ? (
              <FriendsEmpty />
            ) : (
              <FriendsNoneOnline />
            )
          ) : (
            <Pages perPage={4} gap={10} reserve>
              {online.map((f) => (
                <FriendCard
                  key={f.address}
                  friend={f}
                  color={userColor(f.address, f.name, f.claimed, f.nameColor)}
                  where={f.status === 'away' ? 'Away' : 'Online'}
                  onOpen={() => openPassport(f.address)}
                />
              ))}
            </Pages>
          )}
        </section>
      )}

      {shows('live') && liveEvents.data.length > 0 && (
        <section {...widget(styles.live)}>
          {remove('live')}
          <SectionHead title="Live Events" onViewAll={editing ? undefined : openEvents} />
          <Pages perPage={1} gap={0}>
            {liveEvents.data.map((e) => {
              const dest = eventDestination(e)
              const here = e.connected_addresses ?? []
              return (
                <LiveEventCard
                  key={e.id}
                  event={e}
                  friends={friendsAtEvent({ connected_addresses: here }, session.friends.list)}
                  disabled={dest == null || waiting}
                  onJumpIn={() => dest != null && pickEvent(e, dest)}
                  onOpen={dest == null ? undefined : () => openDestinationCard(eventInfo(e, here.length), () => !waitingRef.current && pickEvent(e, dest))}
                />
              )
            })}
          </Pages>
        </section>
      )}

      {shows('events') && upcoming.data.length > 0 && (
        <section {...widget(`${styles.events} ${liveShown ? '' : styles.eventsTop}`.trim())}>
          {remove('events')}
          <SectionHead title="Upcoming Events" onViewAll={editing ? undefined : openEvents} />
          <Pages perPage={1} gap={0}>
            {upcoming.data.map((e) => {
              const dest = eventDestination(e)
              const go = dest == null || waiting ? null : () => openDestinationCard(eventInfo(e), () => !waitingRef.current && pickEvent(e, dest))
              return (
                <UpcomingEventCard
                  key={e.id}
                  event={e}
                  onOpen={go}
                  onAddToCalendar={() => eventActions.addToCalendar(eventLinks(e))}
                  onShare={(button) => openShareMenu(button, eventLinks(e), eventActions)}
                />
              )
            })}
          </Pages>
        </section>
      )}

      {shows('recents') && recents.data.length > 0 && (
        <section {...widget(styles.jumpBack)}>
          {remove('recents')}
          <SectionHead title="Jump Back In" onViewAll={editing ? undefined : openPlaces} />
          <Pages perPage={3} gap={10} reserve>
            {recents.data.map((p) => (
              <PlaceCard key={p.id} {...placeProps(p)} />
            ))}
          </Pages>
        </section>
      )}

      {shows('recommended') && recommended.data.length > 0 && (
        <section {...widget(styles.recommended)}>
          {remove('recommended')}
          <SectionHead title="Recommended Places" onViewAll={editing ? undefined : openPlaces} />
          <Pages perPage={3} gap={10} reserve>
            {recommended.data.map((p) => (
              <PlaceCard key={p.id} {...placeProps(p)} />
            ))}
          </Pages>
        </section>
      )}

      {editing && (
        <div className={styles.editPanel} role="dialog" aria-label="Customize Lobby">
          <h2 className={styles.editTitle}>Customize Lobby</h2>
          <p className={styles.editBody}>
            Choose which widgets to show. Click the close <b>[×]</b> button on a widget to remove it, or add it back from here.
          </p>
          <div className={styles.editList}>
            {WIDGET_IDS.filter((w) => off.includes(w)).map((w) => {
              const Icon = WIDGET_ICONS[w]
              return (
                <button key={w} type="button" className={styles.editRow} onClick={() => setDraft(off.filter((o) => o !== w))}>
                  <span className={styles.editGrip} aria-hidden="true" />
                  <Icon className={styles.editIcon} size={24} aria-hidden="true" />
                  {WIDGETS[w]}
                  <span className={styles.editAdd}>Add</span>
                </button>
              )
            })}
          </div>
          <div className={styles.editActions}>
            <button type="button" className={`${styles.editButton} ${styles.editReset}`} onClick={() => setDraft([])}>
              Reset to Default
            </button>
            <button type="button" className={`${styles.editButton} ${styles.editCancel}`} onClick={() => setDraft(null)}>
              Cancel
            </button>
            <button type="button" className={`${styles.editButton} ${styles.editSave}`} disabled={!changed} onClick={save}>
              <GeneralCheck size={18} aria-hidden="true" />
              Save
            </button>
          </div>
        </div>
      )}

      {tooltip && !editing && (
        <div className={styles.tooltip}>
          <MouseLeft />
          Customize
        </div>
      )}

      {/* a page card is open over the startup lobby: nothing under it takes a click but closing it */}
      {!onClose && (session.settings.open || session.map.open || session.places.open || session.events.open) && (
        <div className={styles.pageGuard} data-testid="lobby-page-guard" onClick={session.closeAllPanels} />
      )}
    </div>
  )
}

// The engine draws the stage and the avatar behind this transparent page.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import closeIcon from '../../assets/map/panel/close-thin.png'
import cursorRotate from '../../assets/lobby/cursor-rotate.png'
import arrowLeft from '../../assets/map/panel/arrow-left.png'
import editIcon from '../../assets/lobby/edit-icn.png'
import eventsIcon from '../../assets/lobby/events-off.png'
import logo from '../../assets/lobby/decentraland-logo.png'
import mapIcon from '../../assets/lobby/map-icn.png'
import mouseLeft from '../../assets/lobby/icon-mouse-left.png'
import notificationsOff from '../../assets/lobby/notifications-off.png'
import notificationsOn from '../../assets/lobby/notifications-on.png'
import placesIcon from '../../assets/lobby/places-off.png'
import { ContextMenu, MaskIcon, showToast } from '../../design'
import { isNativeHud, serviceUrl } from '@hud-host'
import { exitApplication } from '../gate/exitApplication'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { clickButton } from '@explorer-core/lib/analytics'
import { userColor } from '@explorer-core/lib/identity'
import { holeClip } from '@explorer-core/lib/holeClip'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { checkRealm, realmCheckMessage } from '@explorer-core/lib/realmCheck'
import { lobbyStagePreset } from '@explorer-core/engine/lobbyStage'
import { eventDestination } from '@explorer-core/features/events/eventsApi'
import { placeTeleport } from '@explorer-core/features/places/placesApi'
import { LobbyLayout, familyTitle, widgetFamily, widgetSpan } from '@explorer-core/features/lobby/layout'
import { loadLayout, saveLayout } from '@explorer-core/features/lobby/layoutStorage'
import { fetchCrowd, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
import { splitEvents } from '@explorer-core/features/lobby/lobbyEvents'
import { parseGotoTarget, randomParcel, worldName } from '@explorer-core/features/lobby/goto'
import * as lobbyAnalytics from '@explorer-core/features/lobby/lobbyAnalytics'
import type { Community, EventData, Friend } from '@explorer-core/engine/protocol'
import { modalHole } from '../backpack/BackpackModal'
import { EngineViewport } from '../engine/EngineViewport'
import { DEGREES_PER_PX } from '../engine/AvatarDragSurface'
import { openCommunityModal } from '../communities/CommunityModal'
import { openEventDetail } from '../events/EventDetail'
import { eventCalendarUrl, eventCopyLink, eventShareUrl } from '../events/eventFormat'
import { lobbyEvents } from '../events/eventsStore'
import { usePhotoViewer } from '../gallery/usePhotoViewer'
import { useOwnReelActions } from '../gallery/useOwnReelActions'
import { photoWorldRealm } from '../gallery/reelActions'
import { useReelList } from '../gallery/useReelList'
import { useStoredProfile } from '../login/useStoredProfile'
import { openPlaceDetail } from '../places/PlaceDetail'
import { openPassport } from '../profile/Passport'
import { signOutOfWorld } from '../gate/signOutOfWorld'
import { useSession } from '../session/SessionContext'
import { SIGNING_IN_PICK, type Destination } from '../session/useEngineSession'
import { profilePictureStyle, type ConnectedFriend } from './LobbyCards'
import { CustomizeModal, DropMarker, EditorGrids, WidgetHandle, WidgetPreview, useLayoutEditor } from './LobbyEditor'
import { openExploreSection, type ExploreSection } from './ExploreSectionModal'
import { LobbyProfileMenu } from './LobbyProfileMenu'
import { LobbyWidget, widgetShown, type LobbyWidgetContext } from './LobbyWidgets'
import { cellRect, type Rect } from './grid'
import { offlineTitle, refreshWhere, useFriendLocations, useLanding, usePlaces } from './useLobbyData'
import { STAGE_BACKDROPS } from './stageBackdrops'
import styles from './LobbyHome.module.css'
import type { EngineViewportRegion } from '@explorer-core/engine/protocol'

const cx = (...names: Array<string | false | null | undefined>): string => names.filter(Boolean).join(' ')

const RAIL_MAX_CARDS = 10
/** Pixels a press on the avatar travels before it turns the avatar instead of opening the Backpack. */
const DRAG_THRESHOLD = 8
/** Pixels from the pointer. */
const AVATAR_TOOLTIP_OFFSET = 100
const FRIEND_TOOLTIP_OFFSET = 16
const STALE = Symbol('stale')
const CROWD_ERROR = 'The most crowded place could not be found. Please try again.'
const FRIEND_UNKNOWN = 'Your friend’s location could not be found. Please try again.'

// The Backpack's preview runs past its modal's edge; only the part inside the modal is a hole.
function clipToModal(rect: Rect | null): ReturnType<typeof modalHole> {
  if (rect == null) return null
  const frame = document.querySelector<HTMLElement>('[role="dialog"][aria-label="Backpack"]')
  const whole = { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
  return frame == null ? { rect: whole, round: { topLeft: 0, topRight: 0, bottomRight: 0, bottomLeft: 0 } } : modalHole(whole, frame)
}

/** Until the engine has drawn the stage, so the lobby never shows an empty or half-loaded centre. */
export function StandInStage({ hidden, body, preset }: { hidden: boolean; body?: string; preset?: string }): React.JSX.Element {
  const backdrop = STAGE_BACKDROPS[lobbyStagePreset(preset).id]
  return (
    <div className={cx(styles.standIn, hidden && styles.standInHidden)} aria-hidden="true">
      <img className={styles.standInBackdrop} src={backdrop} alt="" />
      <div className={styles.standInFloor} />
      {body && <img className={styles.standInAvatar} src={body} alt="" draggable={false} />}
    </div>
  )
}

function useCanvas(root: React.RefObject<HTMLDivElement | null>): { canvas: { width: number; height: number }; toCanvas: (x: number, y: number) => { x: number; y: number } } {
  const [canvas, setCanvas] = useState({ width: 1920, height: 1080 })
  useLayoutEffect(() => {
    const el = root.current
    if (el == null) return
    const measure = (): void => setCanvas({ width: el.offsetWidth || 1920, height: el.offsetHeight || 1080 })
    measure()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => ro.disconnect()
  }, [root])
  const toCanvas = useCallback(
    (x: number, y: number) => {
      const el = root.current
      if (el == null) return { x, y }
      const box = el.getBoundingClientRect()
      const scale = box.width / (el.offsetWidth || 1) || 1
      return { x: (x - box.left) / scale, y: (y - box.top) / scale }
    },
    [root]
  )
  return { canvas, toCanvas }
}

function WidgetBoard({ layout, ctx, canvas, editing }: { layout: LobbyLayout; ctx: LobbyWidgetContext; canvas: { width: number; height: number }; editing: boolean }): React.JSX.Element {
  return (
    <>
      {layout.all.map((p) => {
        const r = cellRect(p.corner, p.cell, widgetSpan(p.widget), canvas)
        if (!widgetShown(p.widget, ctx)) return null
        return (
          <section
            key={p.widget}
            className={cx(styles.widget, editing && styles.widgetEditing)}
            style={{ left: r.x, top: r.y, width: r.width, height: r.height }}
            aria-label={familyTitle(widgetFamily(p.widget))}
            inert={editing || undefined}
          >
            <LobbyWidget widget={p.widget} ctx={editing ? { ...ctx, editing } : ctx} />
          </section>
        )
      })}
    </>
  )
}

function EditSession({
  saved,
  ctx,
  canvas,
  toCanvas,
  welcome,
  covered,
  onDone
}: {
  saved: LobbyLayout
  ctx: LobbyWidgetContext
  canvas: { width: number; height: number }
  toCanvas: (x: number, y: number) => { x: number; y: number }
  welcome: string
  covered: boolean
  onDone: (layout: LobbyLayout | null) => void
}): React.JSX.Element {
  const e = useLayoutEditor(saved, canvas, toCanvas)
  const { display, lifted, changed, draft } = e.editor
  const cancel = useRef(() => onDone(null))
  cancel.current = () => onDone(null)
  // a page over the editor takes Cancel first, so the draft stays
  useEffect(() => (covered ? undefined : registerCancelLayer(() => cancel.current())), [covered])
  return (
    <>
      <EditorGrids canvas={canvas} target={e.target} size={e.drag?.widget ?? null} />
      <WidgetBoard layout={display} ctx={ctx} canvas={canvas} editing />
      {display.all.map((p) => {
        const r = cellRect(p.corner, p.cell, widgetSpan(p.widget), canvas)
        return <WidgetHandle key={p.widget} widget={p.widget} rect={r} onStart={(ev) => e.start(p.widget, 'grid', ev, r)} onRemove={() => e.remove(p.widget)} />
      })}
      <CustomizeModal
        layout={draft}
        target={e.overModal}
        modalRef={e.modal}
        dragging={e.drag?.widget ?? null}
        changed={changed}
        onStart={(widget, ev, size) => e.start(widget, 'modal', ev, { x: 0, y: 0, ...size })}
        onPlace={e.placeAnywhere}
        onReset={e.reset}
        onCancel={() => onDone(null)}
        onSave={() => onDone(draft)}
      />
      {e.drag != null && e.target != null && <DropMarker target={e.target} widget={e.drag.widget} canvas={canvas} />}
      {lifted != null && <WidgetPreview widget={lifted.widget} rect={lifted.rect} welcome={welcome} />}
      <div className={cx(styles.cursorLayer, e.drag != null && (e.target?.valid || e.overModal ? styles.cursorDragging : styles.cursorNotAllowed))} />
    </>
  )
}

export function LobbyHome({
  onPick,
  onClose,
  setEngineViewport
}: {
  /** False when the pick is refused. */
  onPick: (dest: Destination) => boolean
  /** In-world only: the lobby can be closed back to the world (at startup Jump In is the way out). */
  onClose?: () => void
  setEngineViewport: (region: EngineViewportRegion, rect: Rect | null, dpr?: number) => void
}): React.JSX.Element {
  const session = useSession()
  const startup = onClose == null
  const profile = session.profile.data
  const stored = useStoredProfile(session.login.account ?? undefined)
  const rootRef = useRef<HTMLDivElement>(null)
  const { canvas, toCanvas } = useCanvas(rootRef)

  const [saved, setSaved] = useState(() => {
    const layout = new LobbyLayout()
    loadLayout(layout)
    return layout
  })
  const [editing, setEditing] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const [photoIndex, setPhotoIndex] = useState<number | null>(null)
  const reel = useReelList('lobby', session.gallery.fetchPage)
  const reelActions = useOwnReelActions(session.gallery, reel)
  const [share, setShare] = useState<{ event: EventData; x: number; y: number } | null>(null)
  const sharing = share != null
  useEffect(() => (sharing ? registerCancelLayer(() => setShare(null)) : undefined), [sharing])
  const [avatarTip, setAvatarTip] = useState(false)
  const [friendTip, setFriendTip] = useState<{ name: string; x: number; y: number } | null>(null)
  const [rotating, setRotating] = useState(false)
  // moved on the element itself: a pointer move must not render the whole lobby
  const tipAt = useRef({ x: 0, y: 0 })
  const tipRef = useRef<HTMLDivElement>(null)
  const rotateAt = useRef({ x: 0, y: 0 })
  const rotateRef = useRef<HTMLImageElement>(null)

  const { warm } = session.lobbyData
  const setStage = session.lobbyStage.set
  useEffect(() => {
    lobbyAnalytics.lobbyOpened(startup)
    setStage({ idleEmotes: true })
    warm()
    return () => {
      setStage({ idleEmotes: false })
      lobbyAnalytics.lobbyClosed()
    }
  }, [startup, setStage, warm])

  const landing = useLanding(session.homeScene())
  const places = usePlaces()
  const eventStore = session.events.data.store
  const eventsQuery = eventStore.queries.lobby
  const events = useMemo(() => splitEvents(lobbyEvents(eventStore)), [eventStore])
  // a failed read clears the live list (a stale LIVE badge is a false claim) and keeps the upcoming one
  const live = eventsQuery.status === 'failed' ? [] : events.live
  const referrer = profile?.address ?? null
  const online = useMemo(
    () => session.friends.list.filter((f) => f.status !== 'offline').sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())),
    [session.friends.list]
  )
  const locations = useFriendLocations(online)
  const onlineByAddress = useMemo(() => new Map(online.map((f) => [f.address.toLowerCase(), f])), [online])

  const busy = session.login.busy
  const busyRef = useRef(busy)
  busyRef.current = busy
  // moves on with every pick, lookup, modal or page opened over the lobby and on unmount: a
  // lookup answering later neither travels nor reports a failure
  const travels = useRef(0)
  const moveOn = (): void => void travels.current++
  useEffect(() => () => void travels.current++, [])
  const covered = session.lobbyData.covered
  const wasCovered = useRef(covered)
  useLayoutEffect(() => {
    if (covered && !wasCovered.current) travels.current++
    wasCovered.current = covered
  }, [covered])
  /** False when the pick was refused (sign-in still finishing, or nowhere to go). */
  const pick = useCallback(
    (dest: Destination): boolean => {
      travels.current++
      const ok = onPick(dest)
      if (!ok && busyRef.current) showToast(SIGNING_IN_PICK)
      return ok
    },
    [onPick]
  )
  const afterLookup = <T,>(lookup: Promise<T>): Promise<T | typeof STALE> => {
    const seq = ++travels.current
    return lookup.then(
      (value) => (seq === travels.current ? value : STALE),
      (error: unknown) => {
        if (seq !== travels.current) return STALE
        throw error
      }
    )
  }

  const friendsAt = useCallback(
    (addresses: string[] | null | undefined): ConnectedFriend[] =>
      (addresses ?? []).flatMap((a) => {
        const f = onlineByAddress.get(a.toLowerCase())
        return f ? [{ address: f.address, name: f.name, color: userColor(f.address, f.name, f.claimed, f.nameColor), picture: f.picture ?? null }] : []
      }),
    [onlineByAddress]
  )

  const openSection = (section: ExploreSection): void => {
    if (section === 'places' || section === 'events') lobbyAnalytics.sectionOpened(section)
    moveOn()
    openExploreSection(section)
  }

  /** False when the place names nowhere to travel to or the pick is refused. */
  const jumpPlace = (p: LobbyPlace, origin: lobbyAnalytics.CardOrigin): boolean => {
    const dest = placeTeleport(p)
    if (dest == null || !pick(dest)) return false
    lobbyAnalytics.placeJumpedIn(p, origin)
    clickButton('JUMP_IN', 'LOBBY', { kind: 'place', place_id: p.id })
    return true
  }
  const openPlace = (p: LobbyPlace, origin: lobbyAnalytics.CardOrigin): void => {
    lobbyAnalytics.placeCardClicked(p, origin)
    moveOn()
    openPlaceDetail({ place: p, onJumpIn: (place) => jumpPlace(place, origin) })
  }

  const jumpEvent = (e: EventData, origin: lobbyAnalytics.CardOrigin): (() => boolean) | null => {
    const dest = eventDestination(e)
    if (dest == null) return null
    return () => {
      if (!pick(dest.kind === 'world' ? { kind: 'world', realm: dest.realm } : { kind: 'parcel', x: dest.x, y: dest.y })) return false
      lobbyAnalytics.eventJumpedIn(e, origin)
      clickButton('JUMP_IN', 'LOBBY', { kind: 'event', event_id: e.id })
      return true
    }
  }

  const ctx: LobbyWidgetContext = {
    welcomeName: profile?.name || null,
    landing: { ...landing, title: landing.place?.title ?? (landing.dest ? offlineTitle(landing.dest) : '') },
    recent: places.recent,
    recommended: places.recommended,
    live,
    upcoming: events.upcoming,
    eventsLoaded: eventsQuery.status === 'loaded' || eventsQuery.ids.length > 0,
    online,
    friendsLoading: session.friends.loading,
    totalFriends: session.friends.list.length,
    locations,
    communities: session.lobbyData.communitiesLoaded ? session.communities.list.slice(0, RAIL_MAX_CARDS) : null,
    photos: reel.loaded ? reel.photos.slice(0, RAIL_MAX_CARDS) : null,
    busy,
    friendsAt,
    onFriendHover: (friend, e) => {
      if (friend == null || e == null) return setFriendTip(null)
      const p = toCanvas(e.clientX, e.clientY)
      setFriendTip({ name: friend.name, x: p.x + FRIEND_TOOLTIP_OFFSET, y: p.y })
    },
    openLanding: () => {
      if (landing.place != null) openPlace(landing.place, 'landing')
    },
    jumpLanding: () => {
      if (!pick(null)) return
      if (landing.place) lobbyAnalytics.placeJumpedIn(landing.place, 'landing')
      clickButton('JUMP_IN', 'LOBBY', { kind: 'landing', place_id: landing.place?.id ?? '' })
    },
    openPlace,
    jumpPlace: (p: LobbyPlace, origin) => {
      jumpPlace(p, origin)
    },
    openEvent: (e: EventData, origin) => {
      lobbyAnalytics.eventCardClicked(e, origin)
      moveOn()
      openEventDetail(e.id, (event) => {
        const jump = jumpEvent(event, origin)
        if (jump == null) {
          showToast('This event’s world cannot be reached.', { tone: 'error' })
          return false
        }
        return jump()
      })
    },
    jumpEvent,
    toggleInterest: (e) => session.events.data.setAttending(e, !e.attending),
    addToCalendar: (e) => {
      const url = eventCalendarUrl(e, referrer)
      if (url != null) window.open(url, '_blank', 'noopener')
    },
    shareEvent: (e, anchor) => {
      const box = anchor.getBoundingClientRect()
      const p = toCanvas(box.left + box.width / 2, box.top + box.height / 2)
      setShare({ event: e, x: p.x, y: p.y })
    },
    openFriend: (f: Friend) => {
      moveOn()
      openPassport(f.address)
    },
    joinFriend: (f: Friend) => {
      afterLookup(refreshWhere(f.address))
        .then((where) => {
          if (where === STALE) return
          if (where.kind === 'genesis') {
            if (pick({ kind: 'parcel', x: where.parcel.x, y: where.parcel.y })) lobbyAnalytics.friendJoined(f.address, where.parcel)
          } else if (where.kind === 'world') {
            if (pick({ kind: 'world', realm: where.world })) lobbyAnalytics.friendJoined(f.address, where.parcel ?? { x: 0, y: 0 })
          } else showToast(FRIEND_UNKNOWN)
        })
        .catch(() => showToast(FRIEND_UNKNOWN))
    },
    viewCommunity: (c: Community) => {
      moveOn()
      openCommunityModal(c.id, () => undefined)
    },
    openPhoto: (index) => {
      moveOn()
      setPhotoIndex(index)
    },
    goTo: async (text) => {
      const target = parseGotoTarget(text)
      if (target.world != null) {
        const world = worldName(target.world)
        // the realm's own address, so a name.eth world is probed on the worlds server too
        const reachable = await afterLookup(checkRealm(`${serviceUrl('worldsServer')}/world/${encodeURIComponent(world)}`))
        if (reachable === STALE) return null
        if (reachable === 'not-found') return false
        if (reachable === 'unreachable') {
          showToast(realmCheckMessage(world, 'unreachable'))
          return null
        }
        return pick({ kind: 'world', realm: world, position: target.parcel ? `${target.parcel.x},${target.parcel.y}` : undefined }) || null
      }
      if (target.random) {
        const p = randomParcel()
        return pick({ kind: 'parcel', x: p.x, y: p.y }) || null
      }
      if (target.crowd) {
        try {
          const p = await afterLookup(fetchCrowd())
          if (p === STALE) return null
          return pick({ kind: 'parcel', x: p.x, y: p.y }) || null
        } catch {
          showToast(CROWD_ERROR)
          return null
        }
      }
      if (target.parcel != null) {
        return pick({ kind: 'parcel', x: target.parcel.x, y: target.parcel.y }) || null
      }
      return false
    },
    viewAll: (target) => {
      if (target === 'recent' || target === 'recommended') openSection('places')
      else if (target === 'live' || target === 'upcoming') openSection('events')
      else openSection(target)
    }
  }

  // Under the Backpack modal the lobby shows, with a hole where the engine draws the Backpack's avatar, once drawn.
  const hole = session.backpack.open && session.avatarPreviewReady ? clipToModal(session.avatarPreviewRect) : null
  const canvasScale = (): number => {
    const el = rootRef.current
    return el != null ? el.getBoundingClientRect().width / (el.offsetWidth || 1) || 1 : 1
  }
  const holeScale = hole == null ? 1 : canvasScale()
  const backpackHole =
    hole == null
      ? undefined
      : holeClip(
          { x: hole.rect.x / holeScale, y: hole.rect.y / holeScale, width: hole.rect.width / holeScale, height: hole.rect.height / holeScale },
          { topLeft: hole.round.topLeft / holeScale, topRight: hole.round.topRight / holeScale, bottomRight: hole.round.bottomRight / holeScale, bottomLeft: hole.round.bottomLeft / holeScale }
        )

  const placeAt = (el: HTMLElement | null, at: { x: number; y: number }): void => {
    if (el == null) return
    el.style.left = `${at.x}px`
    el.style.top = `${at.y}px`
  }
  const showAvatarTip = (at: { x: number; y: number }): void => {
    tipAt.current = { x: at.x + AVATAR_TOOLTIP_OFFSET, y: at.y }
    placeAt(tipRef.current, tipAt.current)
    setAvatarTip(true)
  }

  const press = useRef<{ id: number; x: number; lastX: number; dragging: boolean } | null>(null)
  // a press that turned the avatar ends in no click
  const dragged = useRef(false)
  const onAvatarDown = (e: React.PointerEvent): void => {
    if (e.button !== 0 || !session.playerReady) return
    press.current = { id: e.pointerId, x: e.clientX, lastX: e.clientX, dragging: false }
    dragged.current = false
    e.currentTarget.setPointerCapture(e.pointerId)
  }
  const onAvatarMove = (e: React.PointerEvent): void => {
    const p = press.current
    const at = toCanvas(e.clientX, e.clientY)
    if (p == null || p.id !== e.pointerId) {
      showAvatarTip(at)
      return
    }
    const scale = canvasScale()
    if (!p.dragging && Math.abs(e.clientX - p.x) / scale < DRAG_THRESHOLD) return
    p.dragging = true
    dragged.current = true
    setAvatarTip(false)
    rotateAt.current = at
    placeAt(rotateRef.current, at)
    setRotating(true)
    session.turnLobbyAvatar(((e.clientX - p.lastX) / scale) * DEGREES_PER_PX)
    p.lastX = e.clientX
  }
  const onAvatarUp = (e: React.PointerEvent): void => {
    const p = press.current
    if (p == null || p.id !== e.pointerId) return
    press.current = null
    setRotating(false)
    if (p.dragging) showAvatarTip(toCanvas(e.clientX, e.clientY))
  }
  const onAvatarClick = (): void => {
    if (dragged.current) {
      dragged.current = false
      return
    }
    clickButton('CUSTOMIZE', 'LOBBY')
    setAvatarTip(false)
    moveOn()
    session.backpack.toggle()
  }

  const unread = session.notifications.unread
  const nameColor = profile ? userColor(profile.address, profile.name, profile.hasClaimedName, profile.nameColor) : 'var(--lobby-snow)'

  const openPhoto = photoIndex != null ? reel.photos[photoIndex] : undefined
  usePhotoViewer(
    photoIndex != null && openPhoto != null
      ? {
          photos: reel.photos,
          index: photoIndex,
          meta: session.gallery.metas[openPhoto.id],
          canEdit: true,
          onIndex: setPhotoIndex,
          onLoadMeta: session.gallery.loadPhoto,
          loadWearables: session.gallery.loadWearables,
          onClose: () => setPhotoIndex(null),
          onSetPublic: reelActions.setPublic,
          onJumpIn: (place) => {
            const world = photoWorldRealm(place)
            const dest: Destination = world != null ? { kind: 'world', realm: world } : { kind: 'parcel', x: place.x, y: place.y }
            if (pick(dest)) setPhotoIndex(null)
          },
          onDelete: (photo) =>
            void reelActions.remove(photo).then((removed) => {
              if (removed) setPhotoIndex(null)
            })
        }
      : null
  )

  return (
    <div ref={rootRef} className={cx(styles.root, editing && styles.editing)} style={backpackHole}>
      {/* the Backpack opens over the lobby: the engine sets the lobby's stage aside, as it is, until it closes */}
      <div className={styles.stage}>
        <EngineViewport region="lobby" report={setEngineViewport} />
      </div>
      {/* under the Backpack the engine keeps the stage's last frame */}
      <StandInStage hidden={session.lobbyStageReady} body={stored.body} preset={session.lobbyStage.preset} />
      <div className={styles.gradient} />
      <div className={styles.overlay} />

      {!editing && (
        <button
          type="button"
          className={styles.avatarHit}
          aria-label="Customize"
          disabled={!session.playerReady}
          onPointerDown={onAvatarDown}
          onPointerMove={onAvatarMove}
          onPointerUp={onAvatarUp}
          onPointerCancel={() => {
            press.current = null
            setRotating(false)
          }}
          onPointerLeave={() => press.current == null && setAvatarTip(false)}
          onClick={onAvatarClick}
        />
      )}

      <header className={styles.header}>
        <img className={styles.logo} src={logo} alt="Decentraland" />
        <span className={styles.headerTitle}>Lobby</span>
        {editing ? (
          <button type="button" className={cx(styles.barButton, styles.backButton)} onClick={() => setEditing(false)}>
            <MaskIcon src={arrowLeft} width={9} height={14} />
            <span className={styles.backText}>Back</span>
            <span className={styles.backKey}>[Esc]</span>
          </button>
        ) : (
          <button
            type="button"
            className={cx(styles.barButton, styles.roundButton, styles.editButton)}
            aria-label="Customize lobby layout"
            onClick={() => {
              moveOn()
              setEditing(true)
            }}
          >
            <img src={editIcon} alt="" width={24} height={24} />
          </button>
        )}
      </header>

      <nav className={styles.topBar} aria-label="Lobby">
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={() => openSection('map')}>
          <img src={mapIcon} alt="" width={32} height={32} />
          <span className={styles.pillLabel}>Map</span>
        </button>
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={() => openSection('places')}>
          <img src={placesIcon} alt="" width={32} height={32} />
          <span className={styles.pillLabel}>Places</span>
        </button>
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={() => openSection('events')}>
          <img src={eventsIcon} alt="" width={32} height={32} />
          <span className={styles.pillLabel}>Events</span>
        </button>
        <button
          type="button"
          className={cx(styles.barButton, styles.roundButton, session.notifications.open && styles.barButtonOpen)}
          aria-label={unread > 0 ? `Notifications, ${unread} unread` : 'Notifications'}
          aria-expanded={session.notifications.open}
          onClick={() => {
            clickButton('NOTIFICATIONS', 'LOBBY')
            moveOn()
            session.notifications.toggle()
          }}
        >
          <img src={session.notifications.open ? notificationsOn : notificationsOff} alt="" width={32} height={32} />
          {unread > 0 && <span className={styles.badge}>{unread}</span>}
        </button>
        <button type="button" className={cx(styles.barButton, styles.profile)} aria-haspopup="menu" aria-expanded={menuOpen} onClick={() => setMenuOpen((o) => !o)}>
          <span className={styles.profilePicture} style={profilePictureStyle(nameColor, profile?.picture)} />
          <span className={styles.profileName}>{profile?.name || 'Guest'}</span>
        </button>
        {onClose && (
          <button
            type="button"
            className={cx(styles.barButton, styles.closeButton)}
            aria-label="Close"
            onClick={() => {
              clickButton('CLOSE', 'LOBBY')
              onClose()
            }}
          >
            <MaskIcon src={closeIcon} size={12} />
          </button>
        )}
      </nav>

      {editing ? (
        <EditSession
          saved={saved}
          ctx={ctx}
          canvas={canvas}
          toCanvas={toCanvas}
          welcome={profile?.name ? `Welcome ${profile.name}!` : 'Welcome!'}
          covered={covered}
          onDone={(layout) => {
            if (layout != null) {
              saveLayout(layout)
              setSaved(layout)
            }
            setEditing(false)
          }}
        />
      ) : (
        <WidgetBoard layout={saved} ctx={ctx} canvas={canvas} editing={false} />
      )}

      {menuOpen && (
        <LobbyProfileMenu
          name={profile?.name ?? ''}
          address={profile?.address ?? null}
          picture={profile?.picture ?? null}
          verified={profile?.hasClaimedName === true}
          nameColor={nameColor}
          withClose={onClose != null}
          onClose={() => setMenuOpen(false)}
          onProfile={() => {
            if (profile == null) return
            moveOn()
            openPassport(profile.address)
          }}
          onSettings={() => openSection('settings')}
          onReportBug={() => window.open(bugReportUrl(), '_blank', 'noopener')}
          // in world the engine keeps its account while a realm is loaded
          onSignOut={onClose == null ? session.logout : session.login.signedInAs != null && !isNativeHud() ? signOutOfWorld : null}
          onExit={exitApplication}
        />
      )}

      {share && (
        <>
          <div className={styles.shareBackdrop} onClick={() => setShare(null)} />
          <div className={styles.share} style={{ left: share.x, top: share.y }}>
            <ContextMenu
              items={[
                {
                  label: 'Share',
                  onClick: () => {
                    window.open(eventShareUrl(share.event, referrer), '_blank', 'noopener')
                    setShare(null)
                  }
                },
                {
                  label: 'Copy link',
                  onClick: () => {
                    navigator.clipboard?.writeText(eventCopyLink(share.event, referrer)).then(
                      () => showToast('Link copied!'),
                      () => undefined
                    )
                    setShare(null)
                  }
                }
              ]}
            />
          </div>
        </>
      )}

      {avatarTip && !editing && (
        <div ref={tipRef} className={styles.avatarTooltip} style={{ left: tipAt.current.x, top: tipAt.current.y }}>
          <img src={mouseLeft} alt="" width={22} height={22} />
          CUSTOMIZE
        </div>
      )}
      {friendTip && (
        <div className={styles.friendTooltip} style={{ left: friendTip.x, top: friendTip.y }}>
          {friendTip.name}
        </div>
      )}
      {rotating && <img ref={rotateRef} className={styles.rotateCursor} src={cursorRotate} alt="" style={{ left: rotateAt.current.x, top: rotateAt.current.y }} />}
    </div>
  )
}

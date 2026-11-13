// The lobby: shown after sign-in, before entering the world, and opened from the menu in-world. The
// engine draws the stage and the avatar behind this transparent page; the page draws the header, the
// top bar and the widgets on four corner grids around the avatar, laid out as the player arranged
// them (the layout editor) and stored on this device.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import closeIcon from '../../assets/lobby/close-icn-thin.png'
import cursorRotate from '../../assets/lobby/cursor-rotate.png'
import arrowLeft from '../../assets/lobby/arrow-left.png'
import editIcon from '../../assets/lobby/edit-icn.png'
import eventsIcon from '../../assets/lobby/events-off.png'
import logo from '../../assets/lobby/decentraland-logo.png'
import mapIcon from '../../assets/lobby/map-icn.png'
import mouseLeft from '../../assets/lobby/icon-mouse-left.png'
import notificationsOff from '../../assets/lobby/notifications-off.png'
import notificationsOn from '../../assets/lobby/notifications-on.png'
import placesIcon from '../../assets/lobby/places-off.png'
import defaultProfile from '../../assets/lobby/default-profile-thumbnail.png'
import { ContextMenu, MaskIcon, showToast } from '../../design'
import { siteUrl, isNativeHud } from '@hud-host'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { clickButton } from '@explorer-core/lib/analytics'
import { userColor } from '@explorer-core/lib/identity'
import { holeClip } from '@explorer-core/lib/holeClip'
import { lobbyStagePreset } from '@explorer-core/engine/lobbyStage'
import { eventDestination } from '@explorer-core/features/events/eventsApi'
import { placeTeleport } from '@explorer-core/features/places/placesApi'
import { LobbyLayout, widgetSpan } from '@explorer-core/features/lobby/layout'
import { loadLayout, saveLayout } from '@explorer-core/features/lobby/layoutStorage'
import { fetchCrowd, worldReachable, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
import { eventCalendarLink, eventCopyLink, eventShareLink, splitEvents, type LobbyEvent } from '@explorer-core/features/lobby/lobbyEvents'
import { parseGotoTarget, randomParcel, worldName } from '@explorer-core/features/lobby/goto'
import * as lobbyAnalytics from '@explorer-core/features/lobby/lobbyAnalytics'
import type { Community, Friend } from '@explorer-core/engine/protocol'
import { clip } from '../backpack/BackpackModal'
import { EngineViewport } from '../engine/EngineViewport'
import { DEGREES_PER_PX } from '../engine/AvatarDragSurface'
import { openCommunityModal } from '../communities/CommunityModal'
import { PhotoDetail } from '../gallery/PhotoDetail'
import { useStoredProfile } from '../login/useStoredProfile'
import { openPassport } from '../profile/Passport'
import { useSession } from '../session/SessionContext'
import type { Destination } from '../session/useEngineSession'
import type { ConnectedFriend } from './LobbyCards'
import { CustomizeModal, DropMarker, EditorGrids, WidgetHandle, WidgetPreview, useLayoutEditor } from './LobbyEditor'
import { LobbyProfileMenu } from './LobbyProfileMenu'
import { LobbyWidget, widgetShown, type LobbyWidgetContext } from './LobbyWidgets'
import { cellRect, type Rect } from './grid'
import { offlineTitle, refreshWhere, useFriendLocations, useLanding, usePlaces } from './useLobbyData'
import { STAGE_BACKDROPS } from './stageBackdrops'
import styles from './LobbyHome.module.css'

const cx = (...names: Array<string | false | null | undefined>): string => names.filter(Boolean).join(' ')

/** Pixels a press on the avatar travels before it turns the avatar instead of opening the Backpack. */
const DRAG_THRESHOLD = 8
/** The tooltips' distance from the pointer. */
const AVATAR_TOOLTIP_OFFSET = 100
const FRIEND_TOOLTIP_OFFSET = 16
const CROWD_ERROR = 'The most crowded place could not be found. Please try again.'

// The Backpack's preview runs past its modal's edge; only the part inside the modal is a hole.
function clipToModal(rect: Rect | null): { x: number; y: number; width: number; height: number } | null {
  const frame = document.querySelector('[role="dialog"][aria-label="Backpack"]')?.getBoundingClientRect()
  return rect == null || frame == null ? rect : clip({ x: rect.x, y: rect.y, width: rect.width, height: rect.height }, frame)
}

/** Until the engine has drawn the stage, the page shows the chosen look's backdrop and the account's
 *  snapshot, so the lobby never shows an empty or half-loaded centre. */
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

/** The page's canvas size and how it maps from the window (it is scaled to the 1920×1080 reference). */
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

/** The placed widgets, each in the cells its placement spans. */
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
            aria-label={p.widget}
            inert={editing || undefined}
          >
            <LobbyWidget widget={p.widget} ctx={editing ? { ...ctx, editing } : ctx} />
          </section>
        )
      })}
    </>
  )
}

/** The page while its layout is edited: the draft's widgets under their frames, and the modal. */
function EditSession({
  saved,
  ctx,
  canvas,
  toCanvas,
  welcome,
  onDone
}: {
  saved: LobbyLayout
  ctx: LobbyWidgetContext
  canvas: { width: number; height: number }
  toCanvas: (x: number, y: number) => { x: number; y: number }
  welcome: string
  onDone: (layout: LobbyLayout | null) => void
}): React.JSX.Element {
  const e = useLayoutEditor(saved, canvas, toCanvas)
  const { display, lifted, changed, draft } = e.editor
  const cancel = useRef(() => onDone(null))
  cancel.current = () => onDone(null)
  useEffect(() => {
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === 'Escape') {
        ev.stopPropagation()
        cancel.current()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [])
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
  onPick: (dest: Destination) => void
  /** In-world only: the lobby can be closed back to the world (at startup Jump In is the way out). */
  onClose?: () => void
  setEngineViewport: (region: 'map' | 'avatarPreview' | 'lobby', rect: Rect | null, dpr?: number) => void
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
  const [share, setShare] = useState<{ event: LobbyEvent; x: number; y: number } | null>(null)
  const [avatarTip, setAvatarTip] = useState<{ x: number; y: number } | null>(null)
  const [friendTip, setFriendTip] = useState<{ name: string; x: number; y: number } | null>(null)
  const [rotating, setRotating] = useState<{ x: number; y: number } | null>(null)

  // the visit: its analytics, the idle avatar, and the reads the widgets need
  const { warm } = session.lobbyEvents
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
  const events = useMemo(() => splitEvents(session.lobbyEvents.list ?? []), [session.lobbyEvents.list])
  // a failed read clears the live list (a stale LIVE badge is a false claim) and keeps the upcoming one
  const live = session.lobbyEvents.failed ? [] : events.live
  const online = useMemo(
    () => session.friends.list.filter((f) => f.status !== 'offline').sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())),
    [session.friends.list]
  )
  const locations = useFriendLocations(online)
  const onlineByAddress = useMemo(() => new Map(online.map((f) => [f.address.toLowerCase(), f])), [online])

  const busy = session.login.busy
  const pick = useCallback((dest: Destination) => onPick(dest), [onPick])

  const friendsAt = useCallback(
    (addresses: string[] | null | undefined): ConnectedFriend[] =>
      (addresses ?? []).flatMap((a) => {
        const f = onlineByAddress.get(a.toLowerCase())
        return f ? [{ address: f.address, name: f.name, picture: f.picture ?? null }] : []
      }),
    [onlineByAddress]
  )

  const openPlacesPage = (): void => {
    lobbyAnalytics.sectionOpened('places')
    if (!session.places.open) session.places.toggle()
  }
  const openEventsPage = (section: 'live' | 'upcoming'): void => {
    lobbyAnalytics.sectionOpened('events')
    session.events.openAt(section)
  }

  const ctx: LobbyWidgetContext = {
    welcomeName: profile?.name || null,
    landing: { ...landing, title: landing.place?.title ?? (landing.dest ? offlineTitle(landing.dest) : '') },
    recent: places.recent,
    recommended: places.recommended,
    live,
    upcoming: events.upcoming,
    eventsLoaded: session.lobbyEvents.list != null,
    online,
    friendsLoading: session.friends.loading,
    totalFriends: session.friends.list.length,
    locations,
    communities: session.lobbyEvents.communitiesLoaded ? session.communities.list : null,
    photos: session.gallery.loaded ? session.gallery.list.slice(0, 10) : null,
    busy,
    friendsAt,
    onFriendHover: (friend, e) => {
      if (friend == null || e == null) return setFriendTip(null)
      const p = toCanvas(e.clientX, e.clientY)
      setFriendTip({ name: friend.name, x: p.x + FRIEND_TOOLTIP_OFFSET, y: p.y })
    },
    openLanding: () => {
      const p = landing.place
      if (p == null) return
      lobbyAnalytics.placeCardClicked(p, 'landing')
      openPlacesPage()
    },
    jumpLanding: () => {
      if (landing.place) lobbyAnalytics.placeJumpedIn(landing.place, 'landing')
      clickButton('JUMP_IN', 'LOBBY', { kind: 'landing', place_id: landing.place?.id ?? '' })
      pick(null)
    },
    openPlace: (p: LobbyPlace, origin) => {
      lobbyAnalytics.placeCardClicked(p, origin)
      openPlacesPage()
    },
    jumpPlace: (p: LobbyPlace, origin) => {
      const dest = placeTeleport(p)
      if (dest == null) return
      lobbyAnalytics.placeJumpedIn(p, origin)
      clickButton('JUMP_IN', 'LOBBY', { kind: 'place', place_id: p.id })
      pick(dest)
    },
    openEvent: (e: LobbyEvent, origin) => {
      lobbyAnalytics.eventCardClicked(e, origin)
      openEventsPage(e.live ? 'live' : 'upcoming')
    },
    jumpEvent: (e: LobbyEvent, origin) => {
      const dest = eventDestination(e)
      if (dest == null) return null
      return () => {
        lobbyAnalytics.eventJumpedIn(e, origin)
        clickButton('JUMP_IN', 'LOBBY', { kind: 'event', event_id: e.id })
        pick(dest.kind === 'world' ? { kind: 'world', realm: dest.realm } : { kind: 'parcel', x: dest.x, y: dest.y })
      }
    },
    toggleInterest: (e) => session.lobbyEvents.setInterest(e.id, e.attending !== true),
    addToCalendar: (e) => window.open(`${eventCalendarLink(e, siteUrl, profile?.address)}&utm_source=explorer&utm_campaign=discover`, '_blank', 'noopener'),
    shareEvent: (e, anchor) => {
      const box = anchor.getBoundingClientRect()
      const p = toCanvas(box.left + box.width / 2, box.top + box.height / 2)
      setShare({ event: e, x: p.x, y: p.y })
    },
    openFriend: (f: Friend) => openPassport(f.address),
    joinFriend: (f: Friend) => {
      refreshWhere(f.address)
        .then((where) => {
          if (where.kind === 'genesis') {
            lobbyAnalytics.friendJoined(f.address, where.parcel)
            pick({ kind: 'parcel', x: where.parcel.x, y: where.parcel.y })
          } else if (where.kind === 'world') {
            lobbyAnalytics.friendJoined(f.address, where.parcel ?? { x: 0, y: 0 })
            pick({ kind: 'world', realm: where.world })
          }
        })
        .catch(() => undefined)
    },
    viewCommunity: (c: Community) => {
      openCommunityModal(c.id, () => undefined)
    },
    openPhoto: (index) => setPhotoIndex(index),
    goTo: async (text) => {
      const target = parseGotoTarget(text)
      if (target.world != null) {
        const world = worldName(target.world)
        if (!(await worldReachable(world))) return false
        pick({ kind: 'world', realm: world, position: target.parcel ? `${target.parcel.x},${target.parcel.y}` : undefined })
        return true
      }
      if (target.random) {
        const p = randomParcel()
        pick({ kind: 'parcel', x: p.x, y: p.y })
        return true
      }
      if (target.crowd) {
        try {
          const p = await fetchCrowd()
          pick({ kind: 'parcel', x: p.x, y: p.y })
        } catch {
          showToast(CROWD_ERROR)
        }
        return true
      }
      if (target.parcel != null) {
        pick({ kind: 'parcel', x: target.parcel.x, y: target.parcel.y })
        return true
      }
      return false
    },
    viewAll: (target) => {
      if (target === 'recent' || target === 'recommended') openPlacesPage()
      else if (target === 'live' || target === 'upcoming') openEventsPage(target)
      else if (target === 'communities') session.communities.toggle()
      else session.gallery.toggle()
    }
  }

  // An interested mark the service refused is reported, as the events page does.
  const interestError = session.lobbyEvents.interestError
  useEffect(() => {
    if (interestError != null) showToast('Something went wrong, please try again.')
  }, [interestError])

  // Under the Backpack modal the lobby shows, with a hole where the engine draws the Backpack's avatar.
  const hole = session.backpack.open ? clipToModal(session.avatarPreviewRect) : null
  const scale = rootRef.current != null ? rootRef.current.getBoundingClientRect().width / (rootRef.current.offsetWidth || 1) || 1 : 1
  const backpackHole = hole == null ? undefined : holeClip({ x: hole.x / scale, y: hole.y / scale, width: hole.width / scale, height: hole.height / scale })

  // The avatar: a click opens the Backpack, a drag turns it
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
      setAvatarTip({ x: at.x + AVATAR_TOOLTIP_OFFSET, y: at.y })
      return
    }
    if (!p.dragging && Math.abs(e.clientX - p.x) / scale < DRAG_THRESHOLD) return
    p.dragging = true
    dragged.current = true
    setAvatarTip(null)
    setRotating(at)
    session.backpack.rotate(((e.clientX - p.lastX) / scale) * DEGREES_PER_PX)
    p.lastX = e.clientX
  }
  const onAvatarUp = (e: React.PointerEvent): void => {
    const p = press.current
    if (p == null || p.id !== e.pointerId) return
    press.current = null
    setRotating(null)
    if (p.dragging) {
      const at = toCanvas(e.clientX, e.clientY)
      setAvatarTip({ x: at.x + AVATAR_TOOLTIP_OFFSET, y: at.y })
    }
  }
  const onAvatarClick = (): void => {
    if (dragged.current) {
      dragged.current = false
      return
    }
    clickButton('CUSTOMIZE', 'LOBBY')
    setAvatarTip(null)
    session.backpack.toggle()
  }

  const unread = session.notifications.unread
  const nameColor = profile ? userColor(profile.address, profile.name, profile.hasClaimedName) : 'var(--lobby-snow)'

  return (
    <div ref={rootRef} className={cx(styles.root, editing && styles.editing)} style={backpackHole}>
      {/* the Backpack modal takes the avatar preview while it's open; the stage comes back after */}
      <div className={styles.stage}>{!session.backpack.open && <EngineViewport region="lobby" report={setEngineViewport} />}</div>
      <StandInStage hidden={session.lobbyStageReady && !session.backpack.open} body={stored.body} preset={session.lobbyStage.preset} />
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
            setRotating(null)
          }}
          onPointerLeave={() => press.current == null && setAvatarTip(null)}
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
          <button type="button" className={cx(styles.barButton, styles.roundButton, styles.editButton)} aria-label="Customize lobby layout" onClick={() => setEditing(true)}>
            <img src={editIcon} alt="" width={24} height={24} />
          </button>
        )}
      </header>

      <nav className={styles.topBar} aria-label="Lobby">
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={() => session.map.toggle()}>
          <img src={mapIcon} alt="" width={32} height={32} />
          <span className={styles.pillLabel}>Map</span>
        </button>
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={openPlacesPage}>
          <img src={placesIcon} alt="" width={32} height={32} />
          <span className={styles.pillLabel}>Places</span>
        </button>
        <button type="button" className={cx(styles.barButton, styles.pill)} onClick={() => openEventsPage('live')}>
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
            session.notifications.toggle()
          }}
        >
          <img src={session.notifications.open ? notificationsOn : notificationsOff} alt="" width={32} height={32} />
          {unread > 0 && <span className={styles.badge}>{unread}</span>}
        </button>
        <button type="button" className={cx(styles.barButton, styles.profile)} aria-haspopup="menu" aria-expanded={menuOpen} onClick={() => setMenuOpen((o) => !o)}>
          <span className={styles.profilePicture} style={{ backgroundImage: `url("${profile?.picture || defaultProfile}")` }} />
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
          onProfile={() => profile && openPassport(profile.address)}
          onSettings={() => session.settings.toggle()}
          onReportBug={() => window.open(bugReportUrl(), '_blank', 'noopener')}
          onSignOut={session.logout}
          onExit={isNativeHud() ? session.quit : null}
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
                    window.open(`${eventShareLink(share.event, siteUrl, profile?.address)}&utm_source=explorer&utm_campaign=discover`, '_blank', 'noopener')
                    setShare(null)
                  }
                },
                {
                  label: 'Copy link',
                  onClick: () => {
                    navigator.clipboard?.writeText(eventCopyLink(share.event, siteUrl, profile?.address)).then(
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

      {photoIndex != null && session.gallery.list[photoIndex] != null && (
        <div className={styles.photo}>
          <PhotoDetail
            photos={session.gallery.list}
            index={photoIndex}
            meta={session.gallery.metas[session.gallery.list[photoIndex].id]}
            isSelf
            onIndex={setPhotoIndex}
            onLoadMeta={session.gallery.loadPhoto}
            onClose={() => setPhotoIndex(null)}
            onTeleport={(x, y) => {
              setPhotoIndex(null)
              pick({ kind: 'parcel', x, y })
            }}
            onDelete={(id) => {
              session.gallery.remove(id)
              setPhotoIndex(null)
            }}
          />
        </div>
      )}

      {avatarTip && !editing && (
        <div className={styles.avatarTooltip} style={{ left: avatarTip.x, top: avatarTip.y }}>
          <img src={mouseLeft} alt="" width={22} height={22} />
          CUSTOMIZE
        </div>
      )}
      {friendTip && (
        <div className={styles.friendTooltip} style={{ left: friendTip.x, top: friendTip.y }}>
          {friendTip.name}
        </div>
      )}
      {rotating && <img className={styles.rotateCursor} src={cursorRotate} alt="" style={{ left: rotating.x, top: rotating.y }} />}
    </div>
  )
}

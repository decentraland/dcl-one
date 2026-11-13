// A place in the map's panel: its picture, who is there, the actions on it, and its overview,
// photos and events.

import { useEffect, useRef, useState } from 'react'
import {
  PHOTOS_PAGE_SIZE,
  fetchEventsAt,
  fetchPlacePhotos,
  isEmptyPlace,
  isRoad,
  isWorldPlace,
  likeRate,
  likeRateOf,
  setEventAttending,
  setPlaceFavorite,
  setPlaceLike,
  type MapEvent,
  type MapPlace,
  type PlacePhoto
} from '@explorer-core/features/map/mapApi'
import { parcelKey, parseParcel, type Vec2 } from '@explorer-core/features/map/mapCoords'
import type { PlaceView } from '@explorer-core/features/map/navmap'
import { eventSchedule, eventShareLinks, placeShareLinks } from '@explorer-core/features/map/shareLinks'
import { MaskIcon } from '../../../design/MaskIcon'
import tipIcon from '../../../assets/donate/tip.png'
import { useDonate } from '../../donate/useDonate'
import { CAT_ICONS, PANEL_ART } from '../mapArt'
import { ShareMenu } from './ShareMenu'
import { TextWithLinks } from './TextWithLinks'
import styles from './MapPanel.module.css'

type Tab = 'overview' | 'photos' | 'events'

// Categories a place can be listed under in its overview.
const APPEARS_ON = ['game', 'music', 'art', 'social', 'crypto', 'sports', 'casino', 'fashion', 'shop', 'education']
const APPEARS_ICONS: Record<string, string | undefined> = CAT_ICONS

function ActionButton({ label, on, icon, iconOn, disabled, size, flipped, onClick }: {
  label: string
  on: boolean
  icon: string
  iconOn?: string
  disabled?: boolean
  size: number
  flipped?: boolean
  onClick: () => void
}): React.JSX.Element {
  return (
    <button type="button" className={`${styles.action} ${on ? styles.actionOn : ''}`.trim()} aria-label={label} aria-pressed={on} disabled={disabled} onClick={onClick}>
      <MaskIcon className={flipped && on ? styles.flipped : undefined} src={on && iconOn != null ? iconOn : icon} size={size} />
    </button>
  )
}

function EventRow({ event, placeUsers, referrer, onOpen }: { event: MapEvent; placeUsers: number; referrer: string | undefined; onOpen: () => void }): React.JSX.Element {
  const [attending, setAttending] = useState(event.attending === true)
  const [sharing, setSharing] = useState(false)
  const reminds = useRef(0)
  const remind = (): void => {
    const run = ++reminds.current
    const next = !attending
    setAttending(next)
    setEventAttending(event.id, next).catch(() => {
      if (run === reminds.current) setAttending(!next)
    })
  }
  return (
    <li className={styles.eventRow}>
      <button type="button" className={styles.eventOpen} aria-label={event.name} onClick={onOpen} />
      <span className={styles.eventImage}>
        {event.image != null && event.image !== '' && <img src={event.image} alt="" loading="lazy" />}
        {event.live === true && (
          <span className={styles.liveBadge}>
            <MaskIcon src={PANEL_ART.live} size={14} />
            LIVE
            <MaskIcon src={PANEL_ART.players} size={12} />
            {placeUsers}
          </span>
        )}
      </span>
      <span className={styles.eventBody}>
        <span className={styles.eventSchedule}>{eventSchedule(event)}</span>
        <span className={styles.eventName}>{event.name}</span>
        <span className={styles.eventAttendees}>
          <MaskIcon src={PANEL_ART.players} size={14} />
          {event.total_attendees ?? 0}
        </span>
        <span className={styles.eventButtons}>
          <button type="button" className={styles.remind} aria-pressed={attending} onClick={remind}>
            <MaskIcon src={attending ? PANEL_ART.reminderOn : PANEL_ART.reminderOff} size={24} />
            remind me
          </button>
          <span className={styles.shareAnchor}>
            <button type="button" className={styles.eventShare} aria-label="Share" aria-haspopup="menu" aria-expanded={sharing} onClick={() => setSharing(true)}>
              <MaskIcon src={PANEL_ART.share} size={22} />
            </button>
            {sharing && <ShareMenu links={eventShareLinks(event, referrer)} onClose={() => setSharing(false)} />}
          </span>
        </span>
      </span>
      <MaskIcon className={styles.eventArrow} src={PANEL_ART.arrowRight} size={15} />
    </li>
  )
}

function EventsTab({ place, referrer, onOpenEvent }: { place: MapPlace; referrer: string | undefined; onOpenEvent: (event: MapEvent) => void }): React.JSX.Element {
  const [events, setEvents] = useState<MapEvent[] | null>(null)
  useEffect(() => {
    let live = true
    const parcels = place.positions.map(parseParcel).filter((p): p is Vec2 => p != null)
    fetchEventsAt(parcels, false)
      .then((list) => live && setEvents(list))
      .catch(() => live && setEvents([]))
    return () => {
      live = false
    }
  }, [place])
  if (events == null) {
    return (
      <ul className={styles.eventList} aria-busy="true" aria-label="Loading events">
        {Array.from({ length: 8 }, (_, i) => (
          <li key={i} className={styles.resultSkeleton} />
        ))}
      </ul>
    )
  }
  if (events.length === 0) {
    return (
      <div className={styles.tabEmpty} role="status">
        <MaskIcon src={PANEL_ART.emptyEvents} size={56} />
        <p>No Upcoming Events</p>
      </div>
    )
  }
  return (
    <ul className={styles.eventList}>
      {events.map((e) => (
        <EventRow key={e.id} event={e} placeUsers={place.user_count ?? 0} referrer={referrer} onOpen={() => onOpenEvent(e)} />
      ))}
    </ul>
  )
}

function PhotosTab({ placeId, onCount, onOpen }: { placeId: string; onCount: (count: number) => void; onOpen: (photos: PlacePhoto[], index: number) => void }): React.JSX.Element {
  const [photos, setPhotos] = useState<PlacePhoto[] | null>(null)
  const [total, setTotal] = useState(0)
  const loading = useRef(false)
  const load = (offset: number): void => {
    if (loading.current) return
    loading.current = true
    fetchPlacePhotos(placeId, offset)
      .then((page) => {
        setTotal(page.total)
        onCount(page.total)
        setPhotos((prev) => (offset === 0 ? page.images : [...(prev ?? []), ...page.images]))
      })
      .catch(() => {
        onCount(0)
        setPhotos((prev) => prev ?? [])
      })
      .finally(() => {
        loading.current = false
      })
  }
  useEffect(() => {
    load(0)
    // one read per place opened on this tab
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [placeId])

  if (photos == null) return <div className={styles.tabEmpty} aria-busy="true" />
  if (photos.length === 0) {
    return (
      <div className={styles.tabEmpty} role="status">
        <p>No photos here yet!</p>
      </div>
    )
  }
  return (
    <ul
      className={styles.photoList}
      onScroll={(e) => {
        const el = e.currentTarget
        // read on once fewer than a dozen photos remain below
        if (photos.length < total && photos.length % PHOTOS_PAGE_SIZE === 0 && el.scrollHeight - el.scrollTop - el.clientHeight < 12 * 324) load(photos.length)
      }}
    >
      {photos.map((p, i) => (
        <li key={p.id}>
          <button type="button" className={styles.photo} aria-label="Open photo" onClick={() => onOpen(photos, i)}>
            <img src={p.thumbnailUrl ?? p.url} alt="" loading="lazy" />
          </button>
        </li>
      ))}
    </ul>
  )
}

export interface PlaceInfoProps {
  view: PlaceView
  /** The sharer's wallet address, put on the links the panel hands out. */
  referrer: string | undefined
  isHome: boolean
  /** The route the player follows ends at this place. */
  navigating: boolean
  onPlaceChange: (place: MapPlace) => void
  onHome: (home: boolean) => void
  onStartNavigation: () => void
  onStopNavigation: () => void
  onJumpIn: () => void
  onOpenEvent: (event: MapEvent) => void
  onOpenPhoto: (photos: PlacePhoto[], index: number) => void
}

export function PlaceInfo({ view, referrer, isHome, navigating, onPlaceChange, onHome, onStartNavigation, onStopNavigation, onJumpIn, onOpenEvent, onOpenPhoto }: PlaceInfoProps): React.JSX.Element {
  const { place, origin, liveEvent } = view
  const [tab, setTab] = useState<Tab>('overview')
  const [photoCount, setPhotoCount] = useState<number | null>(null)
  const [sharing, setSharing] = useState(false)
  const [imageFailed, setImageFailed] = useState(false)
  // the latest place, for a write that lands after another has changed it
  const current = useRef(place)
  current.current = place
  // an answer to a toggle the player has since toggled again is dropped
  const votes = useRef(0)
  const favourites = useRef(0)
  // what the services last held, which a toggle that fails goes back to
  const heldVote = useRef({ user_like: place.user_like, user_dislike: place.user_dislike })
  const heldFavourite = useRef(place.user_favorite)
  const base = parseParcel(place.base_position)
  // Shown where the host can sign a tip and the place has a creator to tip.
  const donate = useDonate(isWorldPlace(place) || isEmptyPlace(place) || base == null ? null : { parcel: base })

  const world = isWorldPlace(place)
  const empty = isEmptyPlace(place)
  const location = world ? (place.world_name ?? '') : isRoad(place.title) && origin != null ? parcelKey(origin) : place.base_position
  const categories = (place.categories ?? []).filter((c) => APPEARS_ON.includes(c.toLowerCase()))
  const description = place.description != null && place.description !== '' ? place.description : 'No description'
  const image = !imageFailed && place.image != null && place.image !== '' ? place.image : PANEL_ART.defaultPlace

  const vote = (like: boolean | null): void => {
    const run = ++votes.current
    const before = current.current
    onPlaceChange({ ...before, user_like: like === true, user_dislike: like === false })
    setPlaceLike(before.id, like)
      .then((r) => {
        heldVote.current = { user_like: r.user_like, user_dislike: r.user_dislike }
        if (run === votes.current) onPlaceChange({ ...current.current, ...r, like_rate: likeRateOf(r.likes, r.dislikes) })
      })
      .catch(() => {
        if (run === votes.current) onPlaceChange({ ...current.current, ...heldVote.current })
      })
  }
  const favourite = (): void => {
    const before = current.current
    const next = before.user_favorite !== true
    onPlaceChange({ ...before, user_favorite: next })
    const run = ++favourites.current
    setPlaceFavorite(before.id, next)
      .then((r) => {
        heldFavourite.current = r.user_favorite
        if (run === favourites.current) onPlaceChange({ ...current.current, ...r })
      })
      .catch(() => {
        if (run === favourites.current) onPlaceChange({ ...current.current, user_favorite: heldFavourite.current })
      })
  }
  const selectTab = (next: Tab): void => {
    if (next === tab) return
    if (next !== 'photos') setPhotoCount(null)
    setTab(next)
  }

  return (
    <div className={styles.info}>
      <div className={styles.thumbnail}>
        <img src={image} alt="" onError={() => setImageFailed(true)} />
      </div>
      {liveEvent != null && (
        <div className={styles.liveStrip}>
          <span className={styles.liveStripBadge}>
            <MaskIcon src={PANEL_ART.live} size={16} />
            LIVE
          </span>
          <span className={styles.liveStripName}>{liveEvent.name}</span>
        </div>
      )}
      <div className={styles.header}>
        <h2 className={styles.placeName}>{place.title}</h2>
        <p className={styles.creator}>
          created by <b>{place.contact_name}</b>
        </p>
        <div className={styles.stats}>
          <span className={styles.statsDot} />
          <MaskIcon src={PANEL_ART.players} size={22} />
          <span>{place.user_count ?? 0}</span>
          <MaskIcon className={styles.statsLike} src={PANEL_ART.likeOutline} size={20} />
          <span>{((likeRate(place) ?? 0) * 100).toFixed(0)}%</span>
        </div>
        <div className={styles.primary}>
          {!world &&
            (navigating ? (
              <button
                type="button"
                className={`${styles.primaryButton} ${styles.exitNavigation}`}
                onClick={onStopNavigation}
              >
                exit navigation
              </button>
            ) : (
              <button
                type="button"
                className={`${styles.primaryButton} ${styles.startNavigation}`}
                onClick={onStartNavigation}
              >
                <MaskIcon src={PANEL_ART.navigation} size={19} />
                start navigation
              </button>
            ))}
          <button type="button" className={`${styles.primaryButton} ${styles.jumpIn}`} onClick={onJumpIn}>
            JUMP IN
            <MaskIcon src={PANEL_ART.jumpIn} size={24} />
          </button>
        </div>
        <div className={styles.actions}>
          <ActionButton label="Like" on={place.user_like === true} icon={PANEL_ART.likeOutline} iconOn={PANEL_ART.likeSolid} size={30} onClick={() => vote(place.user_like === true ? null : true)} />
          <ActionButton label="Dislike" on={place.user_dislike === true} icon={PANEL_ART.dislikeOutline} iconOn={PANEL_ART.likeSolid} flipped size={30} onClick={() => vote(place.user_dislike === true ? null : false)} />
          <ActionButton
            label="Favorite"
            on={!empty && place.user_favorite === true}
            icon={empty ? PANEL_ART.favouriteBlocked : PANEL_ART.favouriteOutline}
            iconOn={PANEL_ART.favouriteSolid}
            disabled={empty}
            size={26}
            onClick={favourite}
          />
          <ActionButton label="Set as Home" on={isHome} icon={PANEL_ART.homeOutline} iconOn={PANEL_ART.homeSolid} size={26} onClick={() => onHome(!isHome)} />
          {donate.visible && (
            <button type="button" className={styles.action} aria-label="Donate" onClick={(e) => donate.open(e.detail === 0 ? 'keyboard' : 'pointer')}>
              <MaskIcon src={tipIcon} size={26} />
            </button>
          )}
          <span className={styles.shareAnchor}>
            <button type="button" className={styles.action} aria-label="Share" aria-haspopup="menu" aria-expanded={sharing} onClick={() => setSharing(true)}>
              <MaskIcon src={PANEL_ART.share} size={24} />
            </button>
            {sharing && <ShareMenu links={placeShareLinks(place, referrer)} onClose={() => setSharing(false)} />}
          </span>
        </div>
        <div className={styles.tabs} role="tablist">
          {(['overview', 'photos', 'events'] as const).map((t) => (
            <button key={t} type="button" role="tab" aria-selected={tab === t} className={`${styles.tab} ${tab === t ? styles.tabOn : ''}`.trim()} onClick={() => selectTab(t)}>
              {t === 'photos' && photoCount != null ? `photos (${photoCount})` : t}
            </button>
          ))}
        </div>
      </div>
      <div className={styles.tabContent} role="tabpanel">
        {tab === 'overview' && (
          <div className={styles.overview}>
            <div className={styles.additional}>
              <div>
                <h3 className={styles.label}>LOCATION</h3>
                <p className={styles.value}>
                  <MaskIcon src={PANEL_ART.pinOutline} size={22} />
                  {location}
                </p>
              </div>
              <div>
                <h3 className={styles.label}>PARCELS</h3>
                <p className={styles.value}>
                  <MaskIcon src={PANEL_ART.parcels} size={17} />
                  {place.positions.length}
                </p>
              </div>
            </div>
            {categories.length > 0 && <h3 className={styles.label}>APPEARS ON</h3>}
            <ul className={styles.tags}>
              {categories.map((c) => (
                <li key={c} className={styles.tag}>
                  {APPEARS_ICONS[c.toLowerCase()] != null && <img src={APPEARS_ICONS[c.toLowerCase()]} alt="" />}
                  {c}
                </li>
              ))}
            </ul>
            <h3 className={`${styles.label} ${styles.descriptionLabel}`}>DESCRIPTION</h3>
            <p className={styles.description}>
              <TextWithLinks text={description} />
            </p>
          </div>
        )}
        {tab === 'photos' && <PhotosTab placeId={place.id} onCount={setPhotoCount} onOpen={onOpenPhoto} />}
        {tab === 'events' && <EventsTab place={place} referrer={referrer} onOpenEvent={onOpenEvent} />}
      </div>
    </div>
  )
}

// The card's place lacks the player's own votes and favourite: a signed read on opening adds them
// (`usePlaceActions`, which also holds the votes, favourite, home, share and JUMP IN).

import { useMemo, useState, useSyncExternalStore, type CSSProperties } from 'react'
import { openPopup } from '../../design'
import closeIcon from '../../assets/events/close-icn-thin.png'
import defaultPlace from '../../assets/map/panel/default-place.png'
import navigationIcon from '../../assets/map/panel/navigation.png'
import parcelsIcon from '../../assets/map/panel/parcels.png'
import playersIcon from '../../assets/map/panel/players.png'
import featuredIcon from '../../assets/map/icons/layer-poi.png'
import likeRateIcon from '../../assets/places/like-rate.png'
import locationIcon from '../../assets/places/location.png'
import visitsIcon from '../../assets/places/visits.png'
import type { SignedPlace } from '@explorer-core/engine/protocol'
import { isRouteTo, navigation, placeParcel } from '@explorer-core/features/map/navigation'
import type { WorldAccessVerdict as WorldAccess } from '@explorer-core/engine/generated'
import { placeOnline } from '@explorer-core/features/lobby/lobbyApi'
import { userColor } from '@explorer-core/lib/identity'
import { lobbyEvents } from '../events/eventsStore'
import { ConnectedFriends, type ConnectedFriend } from '../lobby/LobbyCards'
import { CAT_ICONS } from '../map/mapArt'
import { openPassport } from '../profile/Passport'
import { useSession } from '../session/SessionContext'
import { useProfile } from '../session/profileStore'
import { compactNumber, creatorFallback, isWorldPlace, likeRateText, updatedText } from './placeDetailFormat'
import { PlaceActionButtons, PlaceJumpIn, usePlaceActions, type DetailPlace } from './PlaceActions'
import styles from './PlaceDetail.module.css'
import { Button, MaskIcon } from '@dcl/one-ui'

export type { DetailPlace }

export interface PlaceDetailOptions {
  place: DetailPlace
  /** Travels to the place; false when it could not, and the detail stays open. */
  onJumpIn: (place: DetailPlace) => boolean
}

const CATEGORIES: { id: string; name: string; icon?: string }[] = [
  { id: 'featured', name: 'Featured', icon: featuredIcon },
  { id: 'social', name: 'Social', icon: CAT_ICONS.social },
  { id: 'music', name: 'Music', icon: CAT_ICONS.music },
  { id: 'art', name: 'Art', icon: CAT_ICONS.art },
  { id: 'game', name: 'Game', icon: CAT_ICONS.game },
  { id: 'fashion', name: 'Fashion', icon: CAT_ICONS.fashion },
  { id: 'education', name: 'Education', icon: CAT_ICONS.education },
  { id: 'shop', name: 'Shop', icon: CAT_ICONS.shop },
  { id: 'sports', name: 'Sports', icon: CAT_ICONS.sports },
  { id: 'business', name: 'Business', icon: CAT_ICONS.business }
]

function Creator({ place, signed }: { place: DetailPlace; signed: SignedPlace | null }): React.JSX.Element {
  const address = signed?.creator_address ?? place.creator_address ?? ''
  const profile = useProfile(address)
  const name = profile?.name != null && profile.name !== '' ? profile.name : creatorFallback(place)
  const color = profile != null ? userColor(profile.address, profile.name, profile.hasClaimedName, profile.nameColor) : 'var(--text)'
  return (
    <p className={styles.creator}>
      <span>created by</span>
      {profile?.picture != null && (
        <button type="button" className={styles.creatorPicture} style={{ '--creator-color': color } as CSSProperties} aria-label={`Open ${name}'s passport`} onClick={() => openPassport(profile.address)}>
          <img src={profile.picture} alt="" draggable={false} />
        </button>
      )}
      <b style={{ color }}>{name}</b>
    </p>
  )
}

const ACCESS_TEXT: Partial<Record<WorldAccess, string>> = { denied: 'INVITE ONLY', password: 'PASSWORD REQUIRED', invited: 'YOU ARE INVITED' }

function PlaceDetail({ place, onJumpIn, onClose }: PlaceDetailOptions & { onClose: () => void }): React.JSX.Element {
  const session = useSession()
  const actions = usePlaceActions(place)
  const { signed, counts, access } = actions
  const world = isWorldPlace(place)
  const [thumbFailed, setThumbFailed] = useState(false)

  const destination = useSyncExternalStore(navigation.subscribe, navigation.getSnapshot)
  const navigating = isRouteTo(destination, place)
  const routable = !world && placeParcel(place) != null

  const jumpIn = (): void => {
    if (!onJumpIn(place)) return
    if (isRouteTo(navigation.getSnapshot(), place)) navigation.stop()
    onClose()
  }

  const connected = signed?.connected_addresses ?? place.connected_addresses
  const online = placeOnline({ connected_addresses: connected, user_count: signed?.user_count ?? place.user_count })
  const friendList = session.friends.list
  const friends = useMemo((): ConnectedFriend[] => {
    const here = new Set((connected ?? []).map((a) => a.toLowerCase()))
    return friendList
      .filter((f) => f.status !== 'offline' && here.has(f.address.toLowerCase()))
      .map((f) => ({ address: f.address, name: f.name, color: userColor(f.address, f.name, f.claimed, f.nameColor), picture: f.picture ?? null }))
  }, [connected, friendList])
  const eventStore = session.events.data.store
  const liveName = useMemo(() => lobbyEvents(eventStore).find((e) => e.live && e.place_id === place.id)?.name ?? place.live_event_name ?? null, [eventStore, place.id, place.live_event_name])
  const liveEvent = liveName != null && liveName !== '' ? { name: liveName } : null
  const categories = CATEGORIES.filter((c) => place.categories?.includes(c.id) === true)
  const accessText = access !== 'checking' ? ACCESS_TEXT[access] : undefined
  const description = place.description != null && place.description !== '' ? place.description : 'No description.'

  return (
    <div className={styles.panel} role="dialog" aria-modal="true" aria-label={place.title} onClick={(e) => e.stopPropagation()}>
      <span className={styles.glow} />
      <div className={styles.thumbnail}>
        <img src={thumbFailed || place.image === '' ? defaultPlace : place.image} alt="" draggable={false} onError={() => setThumbFailed(true)} />
        <span className={styles.badges}>
          {place.live === true && (
            <span className={styles.live} tabIndex={liveEvent != null ? 0 : undefined} aria-label={liveEvent != null ? `Live: ${liveEvent.name}` : 'Live'}>
              LIVE
              {liveEvent != null && (
                <span className={styles.tooltip} role="tooltip">
                  {liveEvent.name}
                </span>
              )}
            </span>
          )}
          {online > 0 && (
            <span className={styles.online} aria-label={`${online} online`}>
              <MaskIcon src={playersIcon} size={16} />
              {online}
            </span>
          )}
          <ConnectedFriends friends={friends} />
        </span>
      </div>

      <div className={styles.head}>
        {accessText != null && <p className={`${styles.access} ${access === 'invited' ? styles.accessOpen : ''}`.trim()}>{accessText}</p>}
        <h2 className={styles.title}>{place.title}</h2>
        <Creator place={place} signed={signed} />
      </div>

      <div className={styles.actions}>
        <p className={styles.stats}>
          <span aria-label="Visits">
            <MaskIcon src={visitsIcon} size={24} />
            {compactNumber(signed?.user_visits ?? place.user_visits)}
          </span>
          <span aria-label="Like rate">
            <MaskIcon src={likeRateIcon} size={24} />
            {likeRateText(counts.likeRate !== undefined ? counts.likeRate : (signed?.like_rate ?? place.like_rate))}
          </span>
        </p>
        <div className={styles.row}>
          <PlaceActionButtons actions={actions} toggle={styles.toggle} share={styles.share} />
        </div>
        <div className={styles.row}>
          {routable &&
            (navigating ? (
              <Button appearance="hud" size="bare" radius="md" variant="navigateExit" className={`${styles.wide} ${styles.navigation}`} onClick={navigation.stop}>
                exit navigation
              </Button>
            ) : (
              <Button appearance="hud" size="bare" radius="md" variant="navigate" className={`${styles.wide} ${styles.navigation}`} onClick={() => navigation.start(place)}>
                <MaskIcon src={navigationIcon} size={19} />
                start navigation
              </Button>
            ))}
          <PlaceJumpIn actions={actions} className={styles.wide} onJumpIn={jumpIn} />
        </div>
      </div>

      <div className={styles.body}>
        <section className={styles.section}>
          <h3 className={styles.label}>DESCRIPTION</h3>
          <p className={styles.description}>{description}</p>
        </section>
        {categories.length > 0 && (
          <section className={styles.section}>
            <h3 className={styles.label}>CATEGORIES</h3>
            <ul className={styles.chips}>
              {categories.map((c) => (
                <li key={c.id} className={styles.chip}>
                  {c.icon != null && <img src={c.icon} alt="" width={16} height={16} draggable={false} />}
                  {c.name.toUpperCase()}
                </li>
              ))}
            </ul>
          </section>
        )}
        <div className={styles.location}>
          <section className={styles.section}>
            <h3 className={styles.label}>LOCATION</h3>
            <p className={styles.value}>
              <MaskIcon src={locationIcon} size={22} />
              {world ? place.world_name : place.base_position}
            </p>
          </section>
          <section className={styles.section}>
            <h3 className={styles.label}>PARCELS</h3>
            <p className={styles.value}>
              <MaskIcon src={parcelsIcon} size={14} />
              {place.positions.length}
            </p>
          </section>
        </div>
        <hr className={styles.line} />
        <div className={styles.facts}>
          <section className={styles.section}>
            <h3 className={styles.label}>FAVORITES</h3>
            <p className={styles.value}>{compactNumber(counts.favorites ?? signed?.favorites ?? place.favorites)}</p>
          </section>
          <section className={styles.section}>
            <h3 className={styles.label}>UPDATED</h3>
            <p className={styles.value}>{updatedText(signed?.updated_at ?? place.updated_at)}</p>
          </section>
        </div>
      </div>

      <Button appearance="hud" size="bare" radius="md" variant="shade" className={styles.close} aria-label="Close place detail" onClick={onClose}>
        <MaskIcon src={closeIcon} size={14} />
      </Button>
    </div>
  )
}

export function openPlaceDetail(options: PlaceDetailOptions): void {
  openPopup((close) => <PlaceDetail {...options} onClose={close} />, { deep: true })
}

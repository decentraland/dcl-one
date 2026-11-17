// The card's place lacks the player's own votes and favourite: a signed read on opening adds them.
// Votes and the favourite change at once and go back if the service refuses them.

import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { Button, ContextMenu, MaskIcon, openPopup, showToast } from '../../design'
import closeIcon from '../../assets/events/close-icn-thin.png'
import linkIcon from '../../assets/events/link-icn.png'
import xIcon from '../../assets/events/x-icn.png'
import defaultPlace from '../../assets/map/panel/default-place.png'
import dislikeOutline from '../../assets/map/panel/dislike-outline.png'
import favouriteOutline from '../../assets/map/panel/favourite-outline.png'
import favouriteSolid from '../../assets/map/panel/favourite-solid.png'
import jumpInIcon from '../../assets/map/panel/jump-in.png'
import likeOutline from '../../assets/map/panel/like-outline.png'
import likeSolid from '../../assets/map/panel/like-solid.png'
import parcelsIcon from '../../assets/map/panel/parcels.png'
import playersIcon from '../../assets/map/panel/players.png'
import shareIcon from '../../assets/map/panel/share.png'
import dislikeSolid from '../../assets/places/dislike-solid.png'
import featuredIcon from '../../assets/map/icons/layer-poi.png'
import likeRateIcon from '../../assets/places/like-rate.png'
import locationIcon from '../../assets/places/location.png'
import visitsIcon from '../../assets/places/visits.png'
import type { SignedPlace } from '@explorer-core/engine/protocol'
import { likeRateOf, setPlaceFavorite, setPlaceLike } from '@explorer-core/features/map/mapApi'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { fetchWorldAccess, type WorldAccess } from '@explorer-core/features/places/worldAccess'
import { placeOnline } from '@explorer-core/features/lobby/lobbyApi'
import { userColor } from '@explorer-core/lib/identity'
import { lobbyEvents } from '../events/eventsStore'
import { ConnectedFriends, type ConnectedFriend } from '../lobby/LobbyCards'
import { CAT_ICONS } from '../map/mapArt'
import { openPassport } from '../profile/Passport'
import { useSession } from '../session/SessionContext'
import { useProfile } from '../session/profileStore'
import { compactNumber, creatorFallback, isWorldPlace, likeRateText, placeJumpLink, placeShareUrl, updatedText } from './placeDetailFormat'
import styles from './PlaceDetail.module.css'

export type DetailPlace = DiscoverPlace & { connected_addresses?: string[] | null }

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

const LIKE_ERROR = 'There was an error liking the place. Please try again.'
const DISLIKE_ERROR = 'There was an error disliking the place. Please try again.'
const FAVORITE_ERROR = 'There was an error setting the place as favorite. Please try again.'

interface Votes {
  like: boolean
  dislike: boolean
  favorite: boolean
}

type Field = 'vote' | 'favorite'
const votesOf = (p: Partial<SignedPlace>): Votes => ({ like: p.user_like === true, dislike: p.user_dislike === true, favorite: p.user_favorite === true })

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

function Toggle({ label, on, iconOn, iconOff, disabled, onClick }: { label: string; on: boolean; iconOn: string; iconOff: string; disabled?: boolean; onClick?: () => void }): React.JSX.Element {
  return (
    <Button size="bare" radius="md" variant="faint" className={styles.toggle} aria-label={label} aria-pressed={on} disabled={disabled} onClick={onClick}>
      <MaskIcon src={on ? iconOn : iconOff} size={24} />
    </Button>
  )
}

function openShareMenu(anchor: HTMLElement, place: DetailPlace, referrer: string | null): void {
  const rect = anchor.getBoundingClientRect()
  openPopup(
    (dismiss) => (
      <div className={styles.shareMenu} style={{ '--menu-left': `${rect.left}px`, '--menu-top': `${rect.bottom}px` } as CSSProperties} onClick={(e) => e.stopPropagation()}>
        <ContextMenu
          items={[
            {
              label: 'Share on X',
              icon: <MaskIcon src={xIcon} size={18} />,
              onClick: () => {
                dismiss()
                window.open(placeShareUrl(place, referrer), '_blank', 'noopener')
              }
            },
            {
              label: 'Copy link',
              icon: <MaskIcon src={linkIcon} size={18} />,
              onClick: () => {
                dismiss()
                navigator.clipboard?.writeText(placeJumpLink(place, referrer)).then(
                  () => showToast('Link copied!'),
                  () => undefined
                )
              }
            }
          ]}
        />
      </div>
    ),
    { dim: false }
  )
}

const ACCESS_TEXT: Partial<Record<WorldAccess, string>> = { denied: 'INVITE ONLY', password: 'PASSWORD REQUIRED', invited: 'YOU ARE INVITED' }

function PlaceDetail({ place, onJumpIn, onClose }: PlaceDetailOptions & { onClose: () => void }): React.JSX.Element {
  const session = useSession()
  const { readSigned } = session.places
  const wallet = session.profile.data?.address ?? null
  const communityIds = session.communities.list.map((c) => c.id)
  const communitiesLoaded = session.lobbyData.communitiesLoaded
  const world = isWorldPlace(place)

  const [signed, setSigned] = useState<SignedPlace | null>(null)
  const [votes, setVotes] = useState<Votes>(() => votesOf(place as Partial<SignedPlace>))
  // the last votes the service confirmed, which a refused change goes back to
  const confirmed = useRef(votes)
  // The signed read can predate a vote or favourite. Per field: before a write it sets the field;
  // while one is in flight it only sets what a refusal goes back to; after one answered it is stale.
  const writes = useRef<Record<Field, 'none' | 'sent'>>({ vote: 'none', favorite: 'none' })
  const answered = useRef<Record<Field, boolean>>({ vote: false, favorite: false })
  const [counts, setCounts] = useState<{ likeRate?: number | null; favorites?: number }>({})
  const [busy, setBusy] = useState<{ vote: boolean; favorite: boolean }>({ vote: false, favorite: false })
  const [access, setAccess] = useState<WorldAccess | 'checking'>(world ? 'checking' : 'allowed')
  const [thumbFailed, setThumbFailed] = useState(false)

  useEffect(() => {
    let live = true
    void readSigned(place.id).then((read) => {
      if (!live || read == null) return
      setSigned(read)
      const fresh = votesOf(read)
      const take = (field: Field, keys: (keyof Votes)[]): void => {
        if (answered.current[field]) return
        const state = writes.current[field]
        for (const key of keys) confirmed.current = { ...confirmed.current, [key]: fresh[key] }
        if (state === 'none') setVotes((v) => ({ ...v, ...Object.fromEntries(keys.map((k) => [k, fresh[k]])) }))
      }
      take('vote', ['like', 'dislike'])
      take('favorite', ['favorite'])
    })
    return () => {
      live = false
    }
  }, [readSigned, place.id])

  const worldName = place.world_name ?? ''
  const communityKey = communityIds.join(',')
  // a re-check for the communities keeps the verdict until it answers
  const checkedFor = useRef<string | null>(null)
  useEffect(() => {
    if (!world) return
    const target = `${worldName}|${wallet ?? ''}`
    if (checkedFor.current !== target) {
      checkedFor.current = target
      setAccess('checking')
    }
    const ctrl = new AbortController()
    const communities = !communitiesLoaded ? null : communityKey === '' ? [] : communityKey.split(',')
    fetchWorldAccess(worldName, wallet, communities, ctrl.signal).then(setAccess, () => undefined)
    return () => ctrl.abort()
  }, [world, worldName, wallet, communityKey, communitiesLoaded])

  const vote = (next: boolean | null, error: string): void => {
    writes.current.vote = 'sent'
    setVotes((v) => ({ ...v, like: next === true, dislike: next === false }))
    setBusy((b) => ({ ...b, vote: true }))
    setPlaceLike(place.id, next).then(
      (result) => {
        writes.current.vote = 'none'
        answered.current.vote = true
        confirmed.current = { ...confirmed.current, like: result.user_like, dislike: result.user_dislike }
        setCounts((c) => ({ ...c, likeRate: likeRateOf(result.likes, result.dislikes) }))
        setVotes((v) => ({ ...v, like: result.user_like, dislike: result.user_dislike }))
        setBusy((b) => ({ ...b, vote: false }))
      },
      () => {
        writes.current.vote = 'none'
        setVotes((v) => ({ ...v, like: confirmed.current.like, dislike: confirmed.current.dislike }))
        setBusy((b) => ({ ...b, vote: false }))
        showToast(error, { tone: 'error' })
      }
    )
  }
  const favorite = (): void => {
    const next = !votes.favorite
    writes.current.favorite = 'sent'
    setVotes((v) => ({ ...v, favorite: next }))
    setBusy((b) => ({ ...b, favorite: true }))
    setPlaceFavorite(place.id, next).then(
      (result) => {
        writes.current.favorite = 'none'
        answered.current.favorite = true
        confirmed.current = { ...confirmed.current, favorite: result.user_favorite }
        setCounts((c) => ({ ...c, favorites: result.favorites }))
        setVotes((v) => ({ ...v, favorite: result.user_favorite }))
        setBusy((b) => ({ ...b, favorite: false }))
      },
      () => {
        writes.current.favorite = 'none'
        setVotes((v) => ({ ...v, favorite: confirmed.current.favorite }))
        setBusy((b) => ({ ...b, favorite: false }))
        showToast(FAVORITE_ERROR, { tone: 'error' })
      }
    )
  }

  const signingIn = session.login.busy
  const jumpIn = (): void => {
    if (onJumpIn(place)) onClose()
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
  const referrer = wallet
  const accessText = access !== 'checking' ? ACCESS_TEXT[access] : undefined
  const showJumpIn = access === 'allowed' || access === 'invited' || access === 'failed'
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
          <Toggle label="Like" on={votes.like} iconOn={likeSolid} iconOff={likeOutline} disabled={busy.vote} onClick={() => vote(votes.like ? null : true, LIKE_ERROR)} />
          <Toggle label="Dislike" on={votes.dislike} iconOn={dislikeSolid} iconOff={dislikeOutline} disabled={busy.vote} onClick={() => vote(votes.dislike ? null : false, DISLIKE_ERROR)} />
          <Toggle label="Favorite" on={votes.favorite} iconOn={favouriteSolid} iconOff={favouriteOutline} disabled={busy.favorite} onClick={favorite} />
          <Button size="bare" radius="md" variant="faint" className={styles.share} aria-label="Share" aria-haspopup="menu" onClick={(e) => openShareMenu(e.currentTarget, place, referrer)}>
            <MaskIcon src={shareIcon} size={24} />
          </Button>
        </div>
        <div className={styles.row}>
          {showJumpIn && (
            <Button size="bare" radius="md" variant="primary" className={styles.wide} aria-label={`Jump in to ${place.title}`} disabled={signingIn} onClick={jumpIn}>
              JUMP IN
              <MaskIcon src={jumpInIcon} size={24} />
            </Button>
          )}
          {access === 'password' && (
            <Button size="bare" radius="md" variant="primary" className={styles.wide} disabled={signingIn} onClick={jumpIn}>
              ENTER PASSWORD
            </Button>
          )}
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

      <Button size="bare" radius="md" variant="shade" className={styles.close} aria-label="Close place detail" onClick={onClose}>
        <MaskIcon src={closeIcon} size={14} />
      </Button>
    </div>
  )
}

export function openPlaceDetail(options: PlaceDetailOptions): void {
  openPopup((close) => <PlaceDetail {...options} onClose={close} />, { deep: true })
}

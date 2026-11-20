// What a player does with a place, shared by the Place Detail panel and the community card's place
// cards (unity-explorer's PlaceCardView): like, dislike, favourite, set as home, share (Share on X,
// Copy link), and JUMP IN, or ENTER PASSWORD for a World that asks one.
// Votes and the favourite change at once and go back if the service refuses them. The place a list
// hands over may lack the player's own votes and favourite: `readSigned` adds them with a signed read.
// What the service confirms for a place reaches every open view of it (a card and the Place Detail
// it opened), so neither shows, nor sends again, a vote the other already changed.

import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { DEFAULT_REALM } from '@hud-host'
import { Button, ContextMenu, MaskIcon, openPopup, showToast } from '../../design'
import linkIcon from '../../assets/events/link-icn.png'
import xIcon from '../../assets/events/x-icn.png'
import dislikeOutline from '../../assets/map/panel/dislike-outline.png'
import favouriteOutline from '../../assets/map/panel/favourite-outline.png'
import favouriteSolid from '../../assets/map/panel/favourite-solid.png'
import homeOutline from '../../assets/map/panel/home-outline.png'
import homeSolid from '../../assets/map/panel/home-solid.png'
import jumpInIcon from '../../assets/map/panel/jump-in.png'
import likeOutline from '../../assets/map/panel/like-outline.png'
import likeSolid from '../../assets/map/panel/like-solid.png'
import shareIcon from '../../assets/map/panel/share.png'
import dislikeSolid from '../../assets/places/dislike-solid.png'
import type { SignedPlace } from '@explorer-core/engine/protocol'
import type { WorldAccessVerdict as WorldAccess } from '@explorer-core/engine/generated'
import { homeOf, isPlacePanelHome, placeHomeTarget } from '@explorer-core/features/map/home'
import { likeRateOf, setPlaceFavorite, setPlaceLike } from '@explorer-core/features/map/mapApi'
import { placeTeleport, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { useSession } from '../session/SessionContext'
import type { TravelOutcome } from '../session/useEngineSession'
import { isWorldPlace, placeJumpLink, placeShareUrl } from './placeDetailFormat'
import styles from './PlaceDetail.module.css'

export type DetailPlace = DiscoverPlace & { connected_addresses?: string[] | null }

const LIKE_ERROR = 'There was an error liking the place. Please try again.'
const DISLIKE_ERROR = 'There was an error disliking the place. Please try again.'
const FAVORITE_ERROR = 'There was an error setting the place as favorite. Please try again.'

interface Votes {
  like: boolean
  dislike: boolean
  favorite: boolean
}

type Field = 'vote' | 'favorite'

/** What the service confirmed for a place, told to the other views of it. */
type Confirmed = { from: object; id: string } & ({ field: 'vote'; like: boolean; dislike: boolean; likeRate: number | null } | { field: 'favorite'; favorite: boolean; favorites: number })
const listeners = new Set<(news: Confirmed) => void>()
const tell = (news: Confirmed): void => {
  for (const hear of listeners) hear(news)
}
const votesOf = (p: Partial<SignedPlace>): Votes => ({ like: p.user_like === true, dislike: p.user_dislike === true, favorite: p.user_favorite === true })

export interface PlaceActions {
  place: DetailPlace
  /** The signed read, once it answered (`readSigned`). */
  signed: SignedPlace | null
  votes: Votes
  busy: { vote: boolean; favorite: boolean }
  /** What the service answered for the counts after a vote or favourite. */
  counts: { likeRate?: number | null; favorites?: number }
  /** A World's access for the signed-in wallet; a parcel is always allowed. */
  access: WorldAccess | 'checking'
  isHome: boolean
  /** The signed-in wallet, the referrer of a shared link. */
  referrer: string | null
  signingIn: boolean
  like: () => void
  dislike: () => void
  favorite: () => void
  setHome: () => void
}

// A list of cards asks once per World and wallet for a while, not each time it shows again; one
// per session's reader, so another session asks afresh. A check that failed is not kept.
const ACCESS_KEPT_MS = 60_000
type AccessReader = (world: string) => Promise<WorldAccess>
const keptAccess = new WeakMap<AccessReader, Map<string, { at: number; verdict: Promise<WorldAccess> }>>()
function sharedAccess(read: AccessReader, world: string, wallet: string | null): Promise<WorldAccess> {
  let kept = keptAccess.get(read)
  if (kept == null) keptAccess.set(read, (kept = new Map()))
  const key = `${wallet ?? ''}|${world.toLowerCase()}`
  const hit = kept.get(key)
  if (hit != null && Date.now() - hit.at < ACCESS_KEPT_MS) return hit.verdict
  const verdict = read(world)
  const entry = { at: Date.now(), verdict }
  kept.set(key, entry)
  void verdict.then((v) => {
    if (v === 'failed' && kept.get(key) === entry) kept.delete(key)
  })
  return verdict
}

/** `readSigned`: add the player's own votes with a signed read. `sharedAccess`: a World's access
 *  answered for another card of it a moment ago serves this one (a list of cards); Place Detail
 *  asks afresh. */
export function usePlaceActions(place: DetailPlace, { readSigned: read = true, sharedAccess: share = false }: { readSigned?: boolean; sharedAccess?: boolean } = {}): PlaceActions {
  const session = useSession()
  const { readSigned, worldAccess } = session.places
  const wallet = session.profile.data?.address ?? null
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
  const self = useRef({}).current

  useEffect(() => {
    if (!read) return
    let live = true
    void readSigned(place.id).then((answer) => {
      if (!live || answer == null) return
      setSigned(answer)
      const fresh = votesOf(answer)
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
  }, [read, readSigned, place.id])

  // another view's confirmed change wins over the signed read; one of this view in flight answers itself
  useEffect(() => {
    const hear = (news: Confirmed): void => {
      if (news.from === self || news.id !== place.id || writes.current[news.field] === 'sent') return
      answered.current[news.field] = true
      if (news.field === 'vote') {
        confirmed.current = { ...confirmed.current, like: news.like, dislike: news.dislike }
        setVotes((v) => ({ ...v, like: news.like, dislike: news.dislike }))
        setCounts((c) => ({ ...c, likeRate: news.likeRate }))
      } else {
        confirmed.current = { ...confirmed.current, favorite: news.favorite }
        setVotes((v) => ({ ...v, favorite: news.favorite }))
        setCounts((c) => ({ ...c, favorites: news.favorites }))
      }
    }
    listeners.add(hear)
    return () => {
      listeners.delete(hear)
    }
  }, [self, place.id])

  const worldName = place.world_name ?? ''
  // the engine checks for the signed-in wallet, so a change of wallet asks again
  useEffect(() => {
    if (!world) return
    let live = true
    setAccess('checking')
    void (share ? sharedAccess(worldAccess, worldName, wallet) : worldAccess(worldName)).then((a) => {
      if (live) setAccess(a)
    })
    return () => {
      live = false
    }
  }, [world, worldName, wallet, worldAccess, share])

  const vote = (next: boolean | null, error: string): void => {
    writes.current.vote = 'sent'
    setVotes((v) => ({ ...v, like: next === true, dislike: next === false }))
    setBusy((b) => ({ ...b, vote: true }))
    setPlaceLike(place.id, next).then(
      (result) => {
        writes.current.vote = 'none'
        answered.current.vote = true
        confirmed.current = { ...confirmed.current, like: result.user_like, dislike: result.user_dislike }
        const likeRate = likeRateOf(result.likes, result.dislikes)
        setCounts((c) => ({ ...c, likeRate }))
        tell({ from: self, id: place.id, field: 'vote', like: result.user_like, dislike: result.user_dislike, likeRate })
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
        tell({ from: self, id: place.id, field: 'favorite', favorite: result.user_favorite, favorites: result.favorites })
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

  const home = homeOf(session.homeScene())
  const isHome = isPlacePanelHome(home, place, null)
  const setHome = (): void => {
    if (isHome) return
    const target = placeHomeTarget(place, null)
    // `placeHomeTarget` reads a World with no name as a Genesis City parcel
    if (target == null || (world && target.realm == null)) return
    session.map.setHome(target.realm ?? DEFAULT_REALM, target.parcel)
  }

  return {
    place,
    signed,
    votes,
    busy,
    counts,
    access,
    isHome,
    referrer: wallet,
    signingIn: session.login.busy,
    like: () => vote(votes.like ? null : true, LIKE_ERROR),
    dislike: () => vote(votes.dislike ? null : false, DISLIKE_ERROR),
    favorite,
    setHome
  }
}

/** Travels to a place: a World changes realm, a parcel teleports; 'refused' when it is neither. */
export function travelToPlace(place: DiscoverPlace, map: { teleportToPlace: (x: number, y: number) => TravelOutcome; changeRealm: (realm: string) => TravelOutcome }): TravelOutcome {
  const t = placeTeleport(place)
  if (t == null) return 'refused'
  return t.kind === 'world' ? map.changeRealm(t.realm) : map.teleportToPlace(t.x, t.y)
}

function Toggle({ label, on, iconOn, iconOff, size, className, disabled, onClick }: { label: string; on: boolean; iconOn: string; iconOff: string; size: number; className: string; disabled?: boolean; onClick?: () => void }): React.JSX.Element {
  return (
    <Button size="bare" radius="md" variant="faint" className={className} aria-label={label} aria-pressed={on} disabled={disabled} onClick={onClick}>
      <MaskIcon src={on ? iconOn : iconOff} size={size} />
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

/** Like, Dislike, Favorite, Set as Home and Share. `toggle` and `share` size the buttons. */
export function PlaceActionButtons({ actions, iconSize = 24, toggle, share }: { actions: PlaceActions; iconSize?: number; toggle: string; share: string }): React.JSX.Element {
  const { votes, busy, isHome, place, referrer } = actions
  return (
    <>
      <Toggle label="Like" on={votes.like} iconOn={likeSolid} iconOff={likeOutline} size={iconSize} className={toggle} disabled={busy.vote} onClick={actions.like} />
      <Toggle label="Dislike" on={votes.dislike} iconOn={dislikeSolid} iconOff={dislikeOutline} size={iconSize} className={toggle} disabled={busy.vote} onClick={actions.dislike} />
      <Toggle label="Favorite" on={votes.favorite} iconOn={favouriteSolid} iconOff={favouriteOutline} size={iconSize} className={toggle} disabled={busy.favorite} onClick={actions.favorite} />
      <Toggle label="Set as Home" on={isHome} iconOn={homeSolid} iconOff={homeOutline} size={iconSize} className={toggle} onClick={actions.setHome} />
      <Button size="bare" radius="md" variant="faint" className={share} aria-label="Share" aria-haspopup="menu" onClick={(e) => openShareMenu(e.currentTarget, place, referrer)}>
        <MaskIcon src={shareIcon} size={iconSize} />
      </Button>
    </>
  )
}

/** JUMP IN where the place lets the player in (or the check failed), ENTER PASSWORD for a World
 *  that asks one, nothing while a World's access is checked or when it is invite-only. */
export function PlaceJumpIn({ actions, className, iconSize = 24, onJumpIn }: { actions: PlaceActions; className: string; iconSize?: number; onJumpIn: () => void }): React.JSX.Element | null {
  const { access, place, signingIn } = actions
  if (access === 'allowed' || access === 'invited' || access === 'failed') {
    return (
      <Button size="bare" radius="md" variant="primary" className={className} aria-label={`Jump in to ${place.title}`} disabled={signingIn} onClick={onJumpIn}>
        JUMP IN
        <MaskIcon src={jumpInIcon} size={iconSize} />
      </Button>
    )
  }
  if (access === 'password') {
    return (
      <Button size="bare" radius="md" variant="primary" className={className} disabled={signingIn} onClick={onJumpIn}>
        ENTER PASSWORD
      </Button>
    )
  }
  return null
}

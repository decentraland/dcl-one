// DestinationCard — the detail card a place or event opens before teleporting: cover, LIVE and
// people badges, title, creator, when/where, description, and the Jump In that actually goes there.

import { Button, Close, Modal, openPopup, PersonIcon } from '../../design'
import { GeneralGrid, GeneralJumpIn, GeneralMapPin } from '../../../../foundations/icons/general'
import { eventLocation, eventWhen, type DclEvent } from '@explorer-core/features/events/eventsApi'
import { placeCoords, placeCreator, placePlayers, placeRating, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { useCreatorName } from './useCreatorName'
import { Mark } from '../../design/Glyphs'
import styles from './DestinationCard.module.css'

export interface DestinationInfo {
  title: string
  image?: string | null
  description?: string | null
  /** A wallet address (shown as its profile name) or a display name. */
  creator?: string | null
  /** e.g. an event's start time. */
  when?: string | null
  location?: { text: string; world: boolean } | null
  live?: boolean
  count?: number
  /** Share of likes among the votes, 0-100. */
  rating?: number
  favorites?: number
  parcels?: number
}

function DestinationCard({ info, onJumpIn, close }: { info: DestinationInfo; onJumpIn: () => void; close: () => void }): React.JSX.Element {
  const creator = useCreatorName(info.creator ?? null)
  return (
    <Modal width={1180} ariaLabel={info.title} className={styles.card}>
      <button type="button" className={styles.close} aria-label="Close" onClick={close}>
        <Close size={18} />
      </button>
      <div className={styles.top}>
        <div className={styles.media}>
          {info.image && <img src={info.image} alt="" draggable={false} />}
          <div className={styles.badges}>
            {info.live && <span className={`${styles.badge} ${styles.live}`}><Mark name="dot" /> Live</span>}
            {info.count != null && info.count > 0 && (
              <span className={styles.badge}>
                <i className={styles.dot} />
                <PersonIcon size={15} />
                {info.count}
              </span>
            )}
          </div>
        </div>
        <div className={styles.summary}>
          <h2 className={styles.title}>{info.title}</h2>
          <div className={styles.meta}>
            {creator && <span>created by <strong>{creator}</strong></span>}
            {info.when && <span>{info.when}</span>}
          </div>
          <div className={styles.grow} />
          {info.rating != null && (
            <div className={styles.stats}>
              <svg viewBox="0 0 16 16" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" aria-hidden="true">
                <path d="M5 7.2 7.6 1.8c1 0 1.8.8 1.8 1.8v2.2h3.3a1.2 1.2 0 0 1 1.2 1.4l-.8 5.2a1.2 1.2 0 0 1-1.2 1H5M5 7.2v6.2M5 7.2H3.2A1.2 1.2 0 0 0 2 8.4v3.8a1.2 1.2 0 0 0 1.2 1.2H5" />
              </svg>
              {info.rating}%
            </div>
          )}
          <Button
            size="modal"
            className={styles.jump}
            onClick={() => {
              close()
              onJumpIn()
            }}
          >
            Jump in <GeneralJumpIn size={22} />
          </Button>
        </div>
      </div>
      <div className={styles.body}>
        {info.description?.trim() && (
          <>
            <h3 className={styles.label}>Description</h3>
            <p className={styles.desc}>{info.description.trim()}</p>
          </>
        )}
        <div className={styles.facts}>
          {info.location && (
            <div>
              <h3 className={styles.label}>Location</h3>
              <span className={`${styles.loc} ${info.location.world ? styles.locWorld : ''}`.trim()}>
                <GeneralMapPin size={18} />
                {info.location.text}
              </span>
            </div>
          )}
          {info.parcels != null && info.parcels > 0 && (
            <div>
              <h3 className={styles.label}>Parcels</h3>
              <span className={styles.loc}>
                <GeneralGrid size={18} />
                {info.parcels}
              </span>
            </div>
          )}
        </div>
        {info.favorites != null && (
          <div className={styles.totals}>
            <div>
              <h3 className={styles.label}>Favorites</h3>
              <span className={styles.total}>{info.favorites}</span>
            </div>
          </div>
        )}
      </div>
    </Modal>
  )
}

export function placeInfo(p: DiscoverPlace): DestinationInfo {
  const players = placePlayers(p)
  const coords = placeCoords(p)
  return {
    title: p.title || (p.world ? (p.world_name ?? '') : 'Untitled'),
    image: p.image,
    description: p.description,
    creator: placeCreator(p) || null,
    location: coords ? { text: coords, world: p.world === true } : null,
    live: players > 0,
    count: players,
    rating: placeRating(p),
    favorites: p.favorites,
    parcels: p.world ? undefined : p.positions?.length
  }
}

/** The detail card's fields for an event. */
export function eventInfo(e: DclEvent, people?: number): DestinationInfo {
  return {
    title: e.name,
    image: e.image,
    description: e.description,
    creator: e.user_name,
    when: eventWhen(e),
    location: { text: eventLocation(e), world: e.world === true },
    live: e.live,
    count: people
  }
}

/** Show a place's or event's details; Jump In closes the card and runs `onJumpIn`. */
export function openDestinationCard(info: DestinationInfo, onJumpIn: () => void): void {
  openPopup((close) => <DestinationCard info={info} onJumpIn={onJumpIn} close={close} />)
}

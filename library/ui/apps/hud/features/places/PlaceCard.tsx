// PlaceCard — a place from the places API, as the Places grid draws it: cover with
// the people count and the Featured ribbon, then title, creator, rating and where it is.

import { PersonIcon, Pin } from '../../design'
import { placeCoords, placeCreator, placeIsFeatured, placePlayers, placeRating, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { useCreatorName } from './useCreatorName'
import styles from './PlaceCard.module.css'

export function PlaceCard({ place, onClick }: { place: DiscoverPlace; onClick: () => void }): React.JSX.Element {
  const players = placePlayers(place)
  const coords = placeCoords(place)
  const creator = useCreatorName(placeCreator(place)) ?? ''
  const isWorld = place.world === true
  const rating = placeRating(place)
  return (
    <button type="button" className={styles.card} onClick={onClick}>
      <span className={styles.media}>
        {place.image && <img src={place.image} alt="" loading="lazy" draggable={false} />}
        {players > 0 && (
          <span className={styles.players}>
            <i className={styles.dot} />
            <PersonIcon size={15} />
            {players}
          </span>
        )}
        {placeIsFeatured(place) && (
          <span className={styles.featured}>
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
              <circle cx="8" cy="6" r="4" />
              <path d="M5.5 9.5 4.5 15 8 13l3.5 2-1-5.5" strokeLinejoin="round" />
            </svg>
            Featured
          </span>
        )}
      </span>
      <span className={styles.title}>{place.title || (isWorld ? (place.world_name ?? '') : 'Untitled')}</span>
      <span className={styles.creator}>{creator}</span>
      <span className={styles.stats}>
        <span className={styles.rating}>
          <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" aria-hidden="true">
            <path d="M5 7.2 7.6 1.8c1 0 1.8.8 1.8 1.8v2.2h3.3a1.2 1.2 0 0 1 1.2 1.4l-.8 5.2a1.2 1.2 0 0 1-1.2 1H5M5 7.2v6.2M5 7.2H3.2A1.2 1.2 0 0 0 2 8.4v3.8a1.2 1.2 0 0 0 1.2 1.2H5" />
          </svg>
          {rating}%
        </span>
        {coords && (
          <span className={styles.where}>
            <Pin size={12} />
            {coords}
          </span>
        )}
      </span>
    </button>
  )
}

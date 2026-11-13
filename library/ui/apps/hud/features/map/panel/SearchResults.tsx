// The results of a search or a category: sort chips, the list, and paging.

import { isWorldPlace, type MapPlace, type MapSort } from '@explorer-core/features/map/mapApi'
import { pagination, type NavmapSnapshot } from '@explorer-core/features/map/navmap'
import { MaskIcon } from '../../../design/MaskIcon'
import { PANEL_ART } from '../mapArt'
import styles from './MapPanel.module.css'

const SORTS: { key: MapSort; label: string; className: string }[] = [
  { key: 'most_active', label: 'Most active', className: styles.sortMostActive },
  { key: 'like_score', label: 'Best rated', className: styles.sortBestRated },
  { key: 'created_at', label: 'Newest', className: styles.sortNewest }
]

const SKELETONS = Array.from({ length: 8 }, (_, i) => i)

function Result({ place, live, onHover, onClick }: { place: MapPlace; live: boolean; onHover: (hovered: boolean) => void; onClick: () => void }): React.JSX.Element {
  const creator = place.contact_name
  const users = place.user_count ?? 0
  return (
    <li>
      <button type="button" className={styles.result} onMouseEnter={() => onHover(true)} onMouseLeave={() => onHover(false)} onClick={onClick}>
        <span className={styles.resultImage}>
          <img src={place.image != null && place.image !== '' ? place.image : PANEL_ART.defaultPlace} alt="" loading="lazy" />
          {live && (
            <span className={styles.liveBadge}>
              <MaskIcon src={PANEL_ART.live} size={14} />
              LIVE
            </span>
          )}
        </span>
        <span className={styles.resultBody}>
          <span className={styles.resultName}>{place.title}</span>
          {creator != null && creator !== '' && creator !== 'Unknown' && (
            <span className={styles.resultCreator}>
              created by <b>{creator}</b>
            </span>
          )}
          {users > 0 && (
            <span className={styles.resultPlayers}>
              <span className={styles.resultDot} />
              {users}
            </span>
          )}
        </span>
        <MaskIcon className={styles.resultArrow} src={PANEL_ART.arrowRight} size={15} />
      </button>
    </li>
  )
}

export function SearchResults({
  results,
  sort,
  onSort,
  onHover,
  onPick,
  onPage
}: {
  results: NavmapSnapshot['results']
  sort: MapSort
  onSort: (sort: MapSort) => void
  onHover: (place: MapPlace, hovered: boolean) => void
  onPick: (place: MapPlace) => void
  onPage: (delta: 1 | -1) => void
}): React.JSX.Element {
  const pages = pagination(results.total, results.page)
  // the list shows one row per base parcel; worlds all sit on 0,0, so they are told apart by id
  const seen = new Set<string>()
  const rows = results.places.filter((p) => {
    const key = isWorldPlace(p) ? p.id : p.base_position
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
  return (
    <div className={styles.search}>
      <div className={styles.sorts} role="group" aria-label="Sort results">
        {SORTS.map((s) => (
          <button
            key={s.key}
            type="button"
            className={`${styles.sort} ${s.className} ${sort === s.key ? styles.sortOn : ''}`.trim()}
            aria-pressed={sort === s.key}
            disabled={sort === s.key}
            onClick={() => onSort(s.key)}
          >
            {s.label}
          </button>
        ))}
      </div>
      <div className={styles.results}>
        {results.loading ? (
          <ul className={styles.resultList} aria-busy="true" aria-label="Loading results">
            {SKELETONS.map((i) => (
              <li key={i} className={styles.resultSkeleton} />
            ))}
          </ul>
        ) : rows.length === 0 ? (
          <div className={styles.noResults} role="status">
            <MaskIcon src={PANEL_ART.emptySearch} size={84} />
            <p className={styles.noResultsTitle}>No results founds</p>
            <p className={styles.noResultsText}>Make sure your search is spelled correctly.</p>
          </div>
        ) : (
          <ul className={styles.resultList} aria-label="Results">
            {rows.map((p) => (
              <Result
                key={isWorldPlace(p) ? p.id : p.base_position}
                place={p}
                live={!isWorldPlace(p) && results.live.has(p.base_position)}
                onHover={(hovered) => onHover(p, hovered)}
                onClick={() => onPick(p)}
              />
            ))}
            {pages.shown && (
              <li className={styles.pages}>
                <button type="button" className={styles.page} disabled={!pages.prev} onClick={() => onPage(-1)}>
                  <MaskIcon className={styles.pageArrow} src={PANEL_ART.arrowLeft} size={12} />
                  Prev
                </button>
                <button type="button" className={styles.page} disabled={!pages.next} onClick={() => onPage(1)}>
                  Next
                  <MaskIcon className={styles.pageArrow} src={PANEL_ART.arrowRight} size={12} />
                </button>
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  )
}

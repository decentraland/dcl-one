// The Places browser: the 68px header band the menu pages share (title,
// section pills, Filter & Sort, search), a row of category chips, and a five-column grid of cards.
// Shared by the in-world PlacesPage and the post-jump-in PlacesPicker; the host supplies an
// `onPick(place)` action.

import { useEffect, useMemo, useRef, useState } from 'react'
import { BrowseLoading, CheckBold, ChevronDown, EmptyState, Heart, Pin, Search } from '../../design'
import { PlaceCard } from './PlaceCard'
import { usePlaces, type PlacesSection, type PlacesSort } from './usePlaces'
import { isHiddenPlace, type DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { CAT_ICONS } from '../map/mapArt'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import styles from './PlacesPage.module.css'
import { Button } from '@dcl/one-ui'

type Pill = 'explore' | 'recent' | 'favourites' | 'my'
// Recent is the public feed, newest first: the HUD keeps no visit history.
const PILLS: { id: Pill; label: string; icon: React.JSX.Element; section: PlacesSection }[] = [
  { id: 'explore', label: 'Explore', icon: <CompassGlyph />, section: 'all' },
  { id: 'recent', label: 'Recent', icon: <RecentGlyph />, section: 'all' },
  { id: 'favourites', label: 'Favorites', icon: <HeartGlyph />, section: 'favourites' },
  { id: 'my', label: 'My Places', icon: <Pin size={20} />, section: 'my' }
]

const CATEGORIES = ['all', 'social', 'music', 'art', 'game', 'fashion', 'education', 'shop', 'sports', 'business']

const SORT_OPTIONS: { label: string; value: PlacesSort }[] = [
  { label: 'Most active', value: 'most_active' },
  { label: 'Most liked', value: 'like_score_best' },
  { label: 'Newest', value: 'created_at' },
  { label: 'Name', value: 'name' }
]

const SEARCH_DEBOUNCE_MS = 300

function CompassGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      <path d="M15.5 8.5l-2 5-5 2 2-5z" />
    </svg>
  )
}
function RecentGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3.5 12a8.5 8.5 0 1 0 2.6-6.1L3.5 8.4" />
      <path d="M3.5 4v4.4h4.4M12 7.5V12l3 2" />
    </svg>
  )
}
function HeartGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 20.5S3.5 15.4 3.5 9.2A4.7 4.7 0 0 1 12 6.6a4.7 4.7 0 0 1 8.5 2.6c0 6.2-8.5 11.3-8.5 11.3z" />
    </svg>
  )
}
function FilterGlyph(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
      <path d="M9 6h12M3 12h12M9 18h12" />
      <circle cx="6" cy="6" r="2.4" />
      <circle cx="18" cy="12" r="2.4" />
      <circle cx="6" cy="18" r="2.4" />
    </svg>
  )
}

export function PlacesBrowser({
  account,
  headExtra,
  columns,
  onPick
}: {
  account?: string | null
  /** Slot rendered at the far right of the header (e.g. the picker's "Skip" button). */
  headExtra?: React.ReactNode
  /** The results grid's column count; unset, the page's five. */
  columns?: number
  onPick: (place: DiscoverPlace) => void
}): React.JSX.Element {
  const { places: list, loading, loadingMore, hasMore, loadMore, retry, error, search, setSearch, category, setCategory, sort, setSort, section, setSection } = usePlaces(account)
  // Local input value, debounced into the hook's `search` so we don't refetch on every keystroke.
  const [draft, setDraft] = useState(search)
  const [sortOpen, setSortOpen] = useState(false)
  const [pill, setPill] = useState<Pill>('explore')
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setSearch(draft), SEARCH_DEBOUNCE_MS)
    return () => {
      if (timer.current) clearTimeout(timer.current)
    }
  }, [draft, setSearch])

  // Escape closes the open sort list before the page
  useEffect(() => {
    if (sortOpen) return registerCancelLayer(() => setSortOpen(false))
  }, [sortOpen])

  const pick = (next: (typeof PILLS)[number]): void => {
    setPill(next.id)
    setSection(next.section)
    setSortOpen(false)
    if (next.id === 'recent') setSort('created_at')
    else if (next.id === 'explore' && sort === 'created_at') setSort('most_active')
  }

  // A search or a personal section shows everything it returns: hiding is for the unfiltered browse only.
  const filtered = search.trim() !== '' || section !== 'all'
  const mainList = useMemo(() => (filtered ? list : list.filter((p) => !isHiddenPlace(p))), [list, filtered])

  return (
    <div className={styles.browser}>
      <header className={styles.header}>
        <h1 className={styles.title}>Places</h1>
        <div className={styles.pills} role="tablist" aria-label="Places sections">
          {PILLS.map((p) => (
            <button key={p.id} type="button" role="tab" aria-selected={p.id === pill} className={`${styles.pill} ${p.id === pill ? styles.pillOn : ''}`.trim()} onClick={() => pick(p)}>
              {p.icon}
              {p.label}
            </button>
          ))}
        </div>
        <div className={styles.tools}>
          <div className={styles.sortWrap}>
            <button type="button" className={styles.sort} aria-haspopup="listbox" aria-expanded={sortOpen} disabled={section !== 'all'} onClick={() => setSortOpen((o) => !o)}>
              <FilterGlyph />
              <span>Filter &amp; Sort</span>
              <ChevronDown size={14} />
            </button>
            {sortOpen && section === 'all' && (
              <>
                <div className={styles.sortScrim} onClick={() => setSortOpen(false)} />
                <div className={styles.sortMenu} role="listbox" aria-label="Sort by">
                  <div className={styles.sortHead}>Sort by</div>
                  {SORT_OPTIONS.map((o) => (
                    <button
                      key={o.value}
                      type="button"
                      role="option"
                      aria-selected={o.value === sort}
                      className={styles.sortItem}
                      onClick={() => {
                        setSort(o.value)
                        setPill(o.value === 'created_at' ? 'recent' : 'explore')
                        setSortOpen(false)
                      }}
                    >
                      {o.label}
                      {o.value === sort && <CheckBold size={14} />}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>
          <label className={styles.search}>
            <Search size={18} />
            <input type="search" value={draft} placeholder="Search" aria-label="Search places" onChange={(e) => setDraft(e.target.value)} />
          </label>
          {headExtra}
        </div>
      </header>

      <div className={styles.categories} role="group" aria-label="Categories">
        {CATEGORIES.map((c) => (
          <button key={c} type="button" aria-pressed={c === category} className={`${styles.category} ${c === category ? styles.categoryOn : ''}`.trim()} onClick={() => setCategory(c)}>
            <img src={CAT_ICONS[c] ?? CAT_ICONS.all} alt="" width={26} height={26} draggable={false} />
            {c}
          </button>
        ))}
      </div>

      {/* the search the hook runs is the trimmed box: the results answer the box once they match */}
      <div className={styles.results} role="region" aria-label="Places results" aria-busy={loading || draft.trim() !== search}>
        {loading ? (
          <BrowseLoading />
        ) : error && list.length === 0 ? (
          <EmptyState variant="inline" tone="error" title="Couldn't load places" subtitle={error} actions={[{ label: 'Retry', onClick: retry }]} />
        ) : mainList.length === 0 ? (
          <EmptyState
            variant="inline"
            icon={section === 'favourites' ? <Heart size={34} /> : <Pin size={34} />}
            title={section === 'favourites' ? 'No favourites yet' : section === 'my' ? 'No places of your own' : 'No results'}
            subtitle={
              section === 'favourites'
                ? 'Sign in and favourite places to find them here.'
                : section === 'my'
                  ? 'Places you own will show up here.'
                  : 'Nothing matched your search.'
            }
          />
        ) : (
          <div className={styles.grid} style={columns != null ? { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` } : undefined}>
            {mainList.map((place) => (
              <PlaceCard key={place.id} place={place} onClick={() => onPick(place)} />
            ))}
          </div>
        )}
        {error && list.length > 0 && (
          <EmptyState variant="inline" tone="error" title="Couldn't load more places" subtitle={error} actions={[{ label: 'Retry', onClick: retry }]} />
        )}
        {!loading && !error && hasMore && (
          <Button appearance="hud" variant="secondary" className={styles.more} disabled={loadingMore} onClick={loadMore}>
            {loadingMore ? 'Loading more…' : 'Load more places'}
          </Button>
        )}
      </div>
    </div>
  )
}

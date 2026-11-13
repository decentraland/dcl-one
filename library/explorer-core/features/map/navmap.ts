// The full-screen map's panel: what the search bar, the results list, the place and event
// panels and the result markers show, and the history the back button walks.
//
// A search, a place and an event are each one entry of that history. Opening one of the same
// kind as the entry on top replaces it, so back always returns to the previous kind of screen.

import {
  SEARCH_PAGE_SIZE,
  emptyPlace,
  fetchEventsAt,
  fetchPlaceAt,
  isWorldPlace,
  searchPlaces,
  type MapEvent,
  type MapPlace,
  type MapSearch,
  type MapSearchResult,
  type MapSort
} from './mapApi'
import { parseParcel, type Vec2 } from './mapCoords'
import { resultMarkers, type PlaceMarker } from './mapMarkers'
import { CAMERA_MOVE_SECONDS } from './mapZoom'

export type CategoryKey = 'all' | 'favorites' | 'social' | 'music' | 'art' | 'game' | 'fashion' | 'education' | 'shop' | 'sports' | 'business'

export interface Category {
  key: CategoryKey
  /** The chip's label and the text the search bar shows while it is selected. */
  name: string
}

export const CATEGORIES: readonly Category[] = [
  { key: 'all', name: 'All' },
  { key: 'favorites', name: 'Favorites' },
  { key: 'social', name: 'Social' },
  { key: 'music', name: 'Music' },
  { key: 'art', name: 'Art' },
  { key: 'game', name: 'Game' },
  { key: 'fashion', name: 'Fashion' },
  { key: 'education', name: 'Education' },
  { key: 'shop', name: 'Shop' },
  { key: 'sports', name: 'Sports' },
  { key: 'business', name: 'Business' }
]

const categoryName = (key: CategoryKey): string => CATEGORIES.find((c) => c.key === key)?.name ?? ''

export type PanelSection = 'search' | 'place' | 'event'

interface SearchParams {
  text: string
  category: CategoryKey | null
  favorites: boolean
  sort: MapSort
  page: number
}

export interface PlaceView {
  place: MapPlace
  /** The parcel the player picked, where the place was reached through one. */
  origin: Vec2 | null
  /** Jump In lands on `origin` rather than the place's base parcel. */
  landOnOrigin: boolean
  /** The place's first live event, once read. */
  liveEvent: MapEvent | null
  /** Steps each time the panel opens on a place, so the view starts over. */
  visit: number
}

export interface EventView {
  event: MapEvent
  place: MapPlace
  visit: number
}

export interface NavmapSnapshot {
  /** Which of the panel's three views is in it. */
  section: PanelSection
  expanded: boolean
  /** The handle on the panel's edge. */
  handle: 'collapse' | 'expand' | null
  input: {
    text: string
    interactable: boolean
    /** The back button stands in for the search icon. */
    back: boolean
    clear: boolean
    /** The category whose icon the field shows. */
    category: CategoryKey | null
  }
  /** The chip that reads as selected. */
  chip: CategoryKey | null
  sort: MapSort
  results: {
    /** False once back has dismissed the list. */
    shown: boolean
    loading: boolean
    places: MapPlace[]
    /** Base parcels of the listed places with a live event. */
    live: ReadonlySet<string>
    page: number
    total: number
  }
  place: PlaceView | null
  event: EventView | null
  /** Markers of a text search. */
  searchMarkers: PlaceMarker[]
  /** Markers of a category search, and the category whose pin they wear. */
  categoryMarkers: PlaceMarker[]
  markerCategory: CategoryKey | null
  /** The Category layer is on (it stays on after a search clears its chip). */
  categoryLayer: boolean
  /** Base parcel of the result the list is pointing at, and whether it was clicked. */
  resultHover: string | null
  resultSelected: string | null
}

type Entry =
  | { type: 'search'; params: SearchParams; result: MapSearchResult | null; live: Set<string> | null }
  | { type: 'place'; place: MapPlace; events: MapEvent[] | null }
  | { type: 'event'; event: MapEvent; place: MapPlace | null }

export interface NavmapEffects {
  /** Bring a parcel to the centre of the map over `seconds`. */
  moveCamera: (parcel: Vec2, seconds: number) => void
}

export interface NavmapApi {
  searchPlaces: typeof searchPlaces
  fetchPlaceAt: typeof fetchPlaceAt
  fetchEventsAt: typeof fetchEventsAt
}

const DEFAULT_API: NavmapApi = { searchPlaces, fetchPlaceAt, fetchEventsAt }

const INITIAL: NavmapSnapshot = {
  section: 'search',
  expanded: false,
  handle: null,
  input: { text: '', interactable: true, back: false, clear: false, category: null },
  chip: null,
  sort: 'most_active',
  results: { shown: true, loading: false, places: [], live: new Set(), page: 0, total: 0 },
  place: null,
  event: null,
  searchMarkers: [],
  categoryMarkers: [],
  markerCategory: null,
  categoryLayer: false,
  resultHover: null,
  resultSelected: null
}

export class Navmap {
  private state: NavmapSnapshot = INITIAL
  private readonly listeners = new Set<() => void>()
  private history: Entry[] = []
  private searchText = ''
  private category: CategoryKey | null = null
  private favorites = false
  // a newer request of the same kind supersedes one still in flight
  private searchRun = 0
  private placeRun = 0
  private eventRun = 0
  private backRun = 0
  private visits = 0
  private effects: NavmapEffects = { moveCamera: () => {} }

  constructor(private readonly api: NavmapApi = DEFAULT_API) {}

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  readonly getSnapshot = (): NavmapSnapshot => this.state

  setEffects(effects: NavmapEffects): void {
    this.effects = effects
  }

  private set(patch: Partial<NavmapSnapshot>): void {
    this.state = { ...this.state, ...patch }
    this.listeners.forEach((l) => l())
  }

  private setInput(patch: Partial<NavmapSnapshot['input']>): void {
    this.set({ input: { ...this.state.input, ...patch } })
  }

  activate(): void {
    this.set({ results: { ...this.state.results, shown: true } })
  }

  deactivate(): void {
    this.history = []
    this.searchRun++
    this.placeRun++
    this.eventRun++
    this.backRun++
    this.clearInput()
  }

  expand(): void {
    this.set({ expanded: true, handle: 'collapse' })
  }

  /** The player closing the panel: it slides away and leaves a handle to bring it back. */
  collapse(): void {
    this.set({ expanded: false, handle: 'expand', input: { ...this.state.input, back: false, category: null } })
  }

  private togglePanel(active: boolean): void {
    if (active && !this.state.expanded) this.expand()
    else if (!active) this.set({ expanded: false, handle: null, input: { ...this.state.input, back: false, category: null } })
  }

  typeText(text: string): void {
    this.category = null
    this.searchText = text
    this.setInput({ text, clear: text !== '' })
  }

  clearInput(): void {
    this.category = null
    this.searchText = ''
    this.togglePanel(false)
    this.set({
      input: { text: '', interactable: true, back: false, clear: false, category: null },
      chip: null,
      searchMarkers: [],
      categoryMarkers: [],
      resultHover: null,
      resultSelected: null
    })
  }

  private updateFilterAndSorting(favorites: boolean, sort: MapSort): void {
    this.favorites = favorites
    this.set({ sort, searchMarkers: [], categoryMarkers: [], resultHover: null, resultSelected: null })
  }

  submitSearch(): void {
    this.updateFilterAndSorting(false, this.state.sort)
    this.togglePanel(this.searchText !== '')
    void this.search({ text: this.searchText, category: null, favorites: false, sort: this.state.sort, page: 0 })
  }

  setSort(sort: MapSort): void {
    this.updateFilterAndSorting(this.favorites, sort)
    void this.search({ text: this.searchText, category: this.category, favorites: this.favorites, sort, page: 0 })
  }

  /** A category chip turned on, or (`null`) the selected one turned off. */
  selectCategory(key: CategoryKey | null): void {
    if (this.state.chip != null && key != null && key !== this.state.chip) this.selectCategory(null)
    this.updateFilterAndSorting(key === 'favorites', this.state.sort)
    if (key == null) {
      this.set({ categoryLayer: false })
      this.clearInput()
      return
    }
    this.set({ chip: key, categoryLayer: true })
    this.togglePanel(true)
    this.searchText = ''
    this.category = key
    void this.search({ text: '', category: key, favorites: key === 'favorites', sort: this.state.sort, page: 0 })
  }

  turnPage(delta: 1 | -1): void {
    const top = this.history[this.history.length - 1]
    if (top?.type !== 'search') return
    void this.search({ ...top.params, page: top.params.page + delta })
  }

  private async search(params: SearchParams): Promise<void> {
    const entry: Entry = { type: 'search', params, result: null, live: null }
    if (await this.showSearch(entry)) this.push(entry)
  }

  private async showSearch(entry: Extract<Entry, { type: 'search' }>): Promise<boolean> {
    const run = ++this.searchRun
    const { params } = entry
    this.favorites = params.favorites
    this.set({
      section: 'search',
      sort: params.sort,
      searchMarkers: [],
      categoryMarkers: [],
      resultHover: null,
      resultSelected: null,
      results: { shown: true, loading: true, places: [], live: new Set(), page: params.page, total: 0 },
      input: {
        ...this.state.input,
        text: params.text !== '' ? params.text : params.category != null ? categoryName(params.category) : '',
        clear: params.category != null || this.searchText !== '',
        category: params.category,
        interactable: params.category == null
      }
    })

    try {
      entry.result ??= await this.api.searchPlaces(toSearch(params))
    } catch {
      entry.result = { places: [], total: 0 }
    }
    if (run !== this.searchRun) return false
    const { places, total } = entry.result
    this.set({ results: { ...this.state.results, loading: false, places, total } })

    if (entry.live == null) {
      const bases: Vec2[] = []
      for (const place of places) {
        const base = parseParcel(place.base_position)
        if (base != null) bases.push(base)
      }
      const live = new Set<string>()
      try {
        for (const event of await this.api.fetchEventsAt(bases, true)) if (event.live === true) live.add(`${event.x},${event.y}`)
      } catch {
        // the list still stands without its live badges
      }
      entry.live = live
    }
    if (run !== this.searchRun) return false
    this.set({ results: { ...this.state.results, live: entry.live } })

    const markers = resultMarkers(places)
    if (params.category != null) this.set({ categoryMarkers: markers, markerCategory: params.category, searchMarkers: [] })
    else this.set({ searchMarkers: markers, categoryMarkers: [] })
    return true
  }

  /** The pointer entering or leaving a result. Worlds have no place on the map. */
  hoverResult(place: MapPlace, hovered: boolean): void {
    if (isWorldPlace(place)) return
    this.set({ resultHover: hovered ? place.base_position : null })
  }

  clickResult(place: MapPlace): void {
    if (!isWorldPlace(place)) this.set({ resultSelected: place.base_position, resultHover: null })
    void this.selectPlace(place, { fromSearch: true })
  }

  async selectPlace(place: MapPlace, options: { fromSearch?: boolean; origin?: Vec2 | null; landOnOrigin?: boolean } = {}): Promise<void> {
    const { fromSearch = false, origin = null, landOnOrigin = false } = options
    if (!fromSearch) this.set({ searchMarkers: [], categoryMarkers: [], resultHover: null, resultSelected: null })
    if (!isWorldPlace(place)) {
      const first = parseParcel(place.positions[0])
      if (first != null) this.effects.moveCamera(first, CAMERA_MOVE_SECONDS)
    }
    const entry: Entry = { type: 'place', place, events: null }
    if (await this.showPlace(entry, { fromSearch, origin, landOnOrigin })) this.push(entry)
  }

  /** A parcel picked on the map: its place, or an empty one. */
  async selectParcel(parcel: Vec2, fromSearch: boolean): Promise<void> {
    const run = ++this.placeRun
    let place: MapPlace | null = null
    try {
      place = await this.api.fetchPlaceAt(parcel)
    } catch {
      place = null
    }
    if (run !== this.placeRun) return
    await this.selectPlace(place ?? emptyPlace(parcel), { fromSearch, origin: parcel })
  }

  /** Open the home parcel's place; Jump In from it lands on that parcel. */
  async selectParcelAsHome(parcel: Vec2): Promise<void> {
    const run = ++this.placeRun
    let place: MapPlace | null = null
    try {
      place = await this.api.fetchPlaceAt(parcel)
    } catch {
      place = null
    }
    if (run !== this.placeRun) return
    await this.selectPlace(place ?? emptyPlace(parcel), { fromSearch: true, origin: parcel, landOnOrigin: true })
  }

  private async showPlace(
    entry: Extract<Entry, { type: 'place' }>,
    params: { fromSearch: boolean; origin: Vec2 | null; landOnOrigin: boolean } | null
  ): Promise<boolean> {
    const run = ++this.placeRun
    const { place } = entry
    this.set({
      section: 'place',
      expanded: true,
      handle: 'collapse',
      place: { place, origin: params?.origin ?? null, landOnOrigin: params?.landOnOrigin === true, liveEvent: null, visit: ++this.visits },
      input: { text: place.title, interactable: false, clear: true, back: params?.fromSearch === true, category: null }
    })
    if (entry.events == null) {
      const parcels: Vec2[] = []
      for (const p of place.positions) {
        const parcel = parseParcel(p)
        if (parcel != null) parcels.push(parcel)
      }
      try {
        entry.events = await this.api.fetchEventsAt(parcels, true)
      } catch {
        entry.events = []
      }
    }
    if (run !== this.placeRun) return false
    const view = this.state.place
    if (view != null && view.place === place && entry.events.length > 0) this.set({ place: { ...view, liveEvent: entry.events[0] } })
    return true
  }

  /** Replace the open place's data after a like, a favourite or a vote changed it. */
  updatePlace(place: MapPlace): void {
    const view = this.state.place
    if (view == null || view.place.id !== place.id) return
    const top = this.history[this.history.length - 1]
    if (top?.type === 'place' && top.place.id === place.id) top.place = place
    this.set({ place: { ...view, place } })
  }

  async selectEvent(event: MapEvent, place: MapPlace | null = null): Promise<void> {
    const entry: Entry = { type: 'event', event, place }
    if (await this.showEvent(entry)) this.push(entry)
  }

  private async showEvent(entry: Extract<Entry, { type: 'event' }>): Promise<boolean> {
    const run = ++this.eventRun
    const { event } = entry
    this.set({ section: 'event', expanded: true, handle: 'collapse' })
    if (entry.place == null) {
      const at = { x: event.coordinates?.[0] ?? event.x, y: event.coordinates?.[1] ?? event.y }
      try {
        entry.place = (await this.api.fetchPlaceAt(at)) ?? emptyPlace(at)
      } catch {
        entry.place = emptyPlace(at)
      }
    }
    if (run !== this.eventRun) return false
    this.set({
      event: { event, place: entry.place, visit: ++this.visits },
      input: { ...this.state.input, text: event.name, interactable: false, back: true, category: null }
    })
    return true
  }

  /** Replace the open event's data after the player's interest changed it. */
  updateEvent(event: MapEvent): void {
    const view = this.state.event
    if (view == null || view.event.id !== event.id) return
    const top = this.history[this.history.length - 1]
    if (top?.type === 'event' && top.event.id === event.id) top.event = event
    this.set({ event: { ...view, event } })
  }

  private push(entry: Entry): void {
    const top = this.history[this.history.length - 1]
    if (top != null && top.type === entry.type) this.history.pop()
    this.history.push(entry)
  }

  async back(): Promise<void> {
    const run = ++this.backRun
    const last = this.history.pop()
    if (last == null) return
    if (last.type === 'search') {
      this.searchRun++
      this.set({ results: { shown: false, loading: false, places: [], live: new Set(), page: 0, total: 0 } })
    } else if (last.type === 'place') {
      this.placeRun++
      this.setInput({ interactable: true, back: false })
    } else {
      this.eventRun++
      this.clearInput()
    }
    const top = this.history[this.history.length - 1]
    if (top == null || run !== this.backRun) return
    if (top.type === 'search') await this.showSearch(top)
    else if (top.type === 'place') await this.showPlace(top, null)
    else await this.showEvent(top)
  }
}

function toSearch(params: SearchParams): MapSearch {
  return {
    text: params.text,
    category: params.category == null || params.category === 'all' || params.category === 'favorites' ? '' : params.category,
    favorites: params.favorites,
    sort: params.sort,
    page: params.page
  }
}

/** Whether the list offers paging, and which way it can turn. */
export function pagination(total: number, page: number): { shown: boolean; next: boolean; prev: boolean } {
  return { shown: total >= SEARCH_PAGE_SIZE, next: Math.floor(total / SEARCH_PAGE_SIZE) > page, prev: page > 0 }
}

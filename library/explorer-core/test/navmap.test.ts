import { describe, it, expect, vi } from 'vitest'
import type { MapEvent, MapPlace, MapSearch } from '../features/map/mapApi'
import { homeOf, isHomePlace, createHomeStore, NO_HOME } from '../features/map/home'
import { createNavigationStore, destinationParcel } from '../features/map/navigation'
import { Navmap, pagination, type NavmapApi } from '../features/map/navmap'
import { parcelCenter } from '../features/map/mapCoords'
import { calendarLink, eventSchedule, eventShareLinks, jumpInLink, placeShareLinks, upcomingDates } from '../features/map/shareLinks'
import { searchQuery } from '../features/map/mapApi'

const place = (id: string, title: string, positions: string[], extra: Partial<MapPlace> = {}): MapPlace => ({ id, title, positions, base_position: positions[0], ...extra })
const event = (id: string, x: number, y: number, extra: Partial<MapEvent> = {}): MapEvent => ({ id, name: `Event ${id}`, x, y, start_at: '2026-11-10T10:00:00.000Z', ...extra })

const PLAZA = place('plaza', 'Genesis Plaza', ['0,0', '1,0'])
const CLUB = place('club', 'Sugar Club', ['-20,60'])
const WORLD = place('world', 'Mansion', ['0,0'], { world_name: 'mansion.dcl.eth' })

function setup(over: Partial<NavmapApi> = {}): { nav: Navmap; api: { [K in keyof NavmapApi]: ReturnType<typeof vi.fn> }; moves: unknown[] } {
  const api = {
    searchPlaces: vi.fn(async (_search: MapSearch) => ({ places: [PLAZA, CLUB, WORLD], total: 3 })),
    fetchPlaceAt: vi.fn(async () => PLAZA as MapPlace | null),
    fetchEventsAt: vi.fn(async () => [] as MapEvent[]),
    ...over
  }
  const nav = new Navmap(api as unknown as NavmapApi)
  const moves: unknown[] = []
  nav.setEffects({ moveCamera: (parcel, seconds) => moves.push([parcel, seconds]) })
  return { nav, api: api as never, moves }
}

const settle = async (): Promise<void> => {
  for (let i = 0; i < 6; i++) await Promise.resolve()
}

describe('map search', () => {
  it('starts collapsed with an empty, editable search bar', () => {
    const { nav } = setup()
    const s = nav.getSnapshot()
    expect(s.expanded).toBe(false)
    expect(s.handle).toBeNull()
    expect(s.input).toEqual({ text: '', interactable: true, back: false, clear: false, category: null })
  })

  it('searches the typed text on submit, opens the panel and marks the places on the map', async () => {
    const { nav, api } = setup()
    nav.typeText('plaza')
    expect(nav.getSnapshot().input.clear).toBe(true)
    nav.submitSearch()
    expect(nav.getSnapshot().expanded).toBe(true)
    expect(nav.getSnapshot().results.loading).toBe(true)
    await settle()
    expect(api.searchPlaces).toHaveBeenCalledWith({ text: 'plaza', category: '', favorites: false, sort: 'most_active', page: 0 })
    const s = nav.getSnapshot()
    expect(s.results.loading).toBe(false)
    expect(s.results.places).toHaveLength(3)
    // worlds are listed but have no marker
    expect(s.searchMarkers.map((m) => m.place.id)).toEqual(['plaza', 'club'])
    expect(s.categoryMarkers).toEqual([])
  })

  it('runs an empty search with the panel closed', async () => {
    const { nav, api } = setup()
    nav.submitSearch()
    await settle()
    expect(api.searchPlaces).toHaveBeenCalledTimes(1)
    expect(nav.getSnapshot().expanded).toBe(false)
    expect(nav.getSnapshot().searchMarkers).toHaveLength(2)
  })

  it('flags the listed places that have a live event', async () => {
    const { nav } = setup({ fetchEventsAt: vi.fn(async () => [event('e', -20, 60, { live: true }), event('f', 0, 0, { live: false })]) })
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    expect([...nav.getSnapshot().results.live]).toEqual(['-20,60'])
  })

  it('keeps the list when the live events cannot be read', async () => {
    const { nav } = setup({ fetchEventsAt: vi.fn(async () => Promise.reject(new Error('down'))) })
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    expect(nav.getSnapshot().results.places).toHaveLength(3)
    expect(nav.getSnapshot().results.live.size).toBe(0)
  })

  it('shows an empty list when the search fails', async () => {
    const { nav } = setup({ searchPlaces: vi.fn(async () => Promise.reject(new Error('down'))) })
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    expect(nav.getSnapshot().results).toMatchObject({ loading: false, places: [], total: 0 })
  })

  it('searches again with the same text when the sort changes', async () => {
    const { nav, api } = setup()
    nav.typeText('plaza')
    nav.submitSearch()
    await settle()
    nav.setSort('created_at')
    await settle()
    expect(api.searchPlaces).toHaveBeenLastCalledWith(expect.objectContaining({ text: 'plaza', sort: 'created_at', page: 0 }))
    expect(nav.getSnapshot().sort).toBe('created_at')
  })

  it('turns pages on the search in view', async () => {
    const { nav, api } = setup({ searchPlaces: vi.fn(async () => ({ places: [PLAZA], total: 120 })) })
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    nav.turnPage(1)
    await settle()
    expect(api.searchPlaces).toHaveBeenLastCalledWith(expect.objectContaining({ page: 1 }))
    expect(nav.getSnapshot().results.page).toBe(1)
  })

  it('offers paging from fifty results, and one page per fifty', () => {
    expect(pagination(49, 0)).toEqual({ shown: false, next: false, prev: false })
    expect(pagination(120, 0)).toEqual({ shown: true, next: true, prev: false })
    expect(pagination(120, 2)).toEqual({ shown: true, next: false, prev: true })
    // a full last page has nothing after it
    expect(pagination(50, 0)).toEqual({ shown: true, next: false, prev: false })
    expect(pagination(100, 0)).toEqual({ shown: true, next: true, prev: false })
    expect(pagination(100, 1)).toEqual({ shown: true, next: false, prev: true })
    expect(pagination(101, 1)).toEqual({ shown: true, next: true, prev: true })
  })

  it('leaves a search that is still on its way behind when a place is opened', async () => {
    let answer: (result: { places: MapPlace[]; total: number }) => void = () => undefined
    const { nav } = setup({ searchPlaces: vi.fn(() => new Promise<{ places: MapPlace[]; total: number }>((resolve) => (answer = resolve))) })
    nav.typeText('club')
    nav.submitSearch()
    await settle()
    void nav.selectParcel({ x: 0, y: 0 }, false)
    await settle()
    answer({ places: [CLUB], total: 1 })
    await settle()
    const s = nav.getSnapshot()
    expect(s.section).toBe('place')
    expect(s.searchMarkers).toEqual([])
    expect(s.results.places).toEqual([])
    // the place is the only thing to go back from
    await nav.back()
    await nav.back()
    expect(nav.getSnapshot().input.interactable).toBe(true)
    expect(nav.getSnapshot().results.shown).toBe(false)
  })

  it('goes back to the list from a place opened while its live badges were still being read', async () => {
    let answer: (events: MapEvent[]) => void = () => undefined
    let reads = 0
    const { nav } = setup({ fetchEventsAt: vi.fn(() => (reads++ === 0 ? new Promise<MapEvent[]>((resolve) => (answer = resolve)) : Promise.resolve([]))) })
    nav.typeText('club')
    nav.submitSearch()
    await settle()
    expect(nav.getSnapshot().results.places).toHaveLength(3)
    nav.clickResult(CLUB)
    await settle()
    answer([])
    await settle()
    expect(nav.getSnapshot().section).toBe('place')
    await nav.back()
    await settle()
    const s = nav.getSnapshot()
    expect(s.section).toBe('search')
    expect(s.results.shown).toBe(true)
    expect(s.results.places).toHaveLength(3)
    expect(s.searchMarkers.map((m) => m.place.id)).toEqual(['plaza', 'club'])
  })

  it('clears the text, the chip and the markers, and closes the panel', async () => {
    const { nav } = setup()
    nav.typeText('plaza')
    nav.submitSearch()
    await settle()
    nav.clearInput()
    const s = nav.getSnapshot()
    expect(s.input).toEqual({ text: '', interactable: true, back: false, clear: false, category: null })
    expect(s.searchMarkers).toEqual([])
    expect(s.expanded).toBe(false)
    expect(s.handle).toBeNull()
  })

  it('builds the destinations query the services expect', () => {
    expect(searchQuery({ text: 'sugar club', category: 'music', favorites: false, sort: 'like_score', page: 2 })).toBe(
      'search=sugar+club&offset=100&limit=50&order_by=like_score&order=desc&categories=music&with_realms_detail=true&only_sdk7=true'
    )
    expect(searchQuery({ text: '', category: '', favorites: true, sort: 'most_active', page: 0 })).toContain('&only_favorites=true')
    // what the player typed stays one value
    const typed = searchQuery({ text: 'a&only_favorites=true #b%c+d', category: '', favorites: false, sort: 'most_active', page: 0 })
    expect(typed.startsWith('search=a%26only_favorites%3Dtrue+%23b%25c%2Bd&offset=0&')).toBe(true)
    expect(new URLSearchParams(typed).get('search')).toBe('a&only_favorites=true #b%c+d')
    expect(new URLSearchParams(typed).has('only_favorites')).toBe(false)
  })
})

describe('map categories', () => {
  it('searches a category, shows its name read-only and marks its places with its pin', async () => {
    const { nav, api } = setup()
    nav.selectCategory('music')
    expect(nav.getSnapshot().chip).toBe('music')
    await settle()
    expect(api.searchPlaces).toHaveBeenCalledWith(expect.objectContaining({ text: '', category: 'music', favorites: false }))
    const s = nav.getSnapshot()
    expect(s.input).toMatchObject({ text: 'Music', interactable: false, category: 'music', clear: true })
    expect(s.categoryMarkers).toHaveLength(2)
    expect(s.markerCategory).toBe('music')
    expect(s.categoryLayer).toBe(true)
    expect(s.searchMarkers).toEqual([])
  })

  it('searches the favourites for the Favorites chip and everything for All', async () => {
    const { nav, api } = setup()
    nav.selectCategory('favorites')
    await settle()
    expect(api.searchPlaces).toHaveBeenLastCalledWith(expect.objectContaining({ category: '', favorites: true }))
    nav.selectCategory('all')
    await settle()
    expect(api.searchPlaces).toHaveBeenLastCalledWith(expect.objectContaining({ category: '', favorites: false }))
  })

  it('clears the search when the chip is turned off', async () => {
    const { nav } = setup()
    nav.selectCategory('art')
    await settle()
    nav.selectCategory(null)
    const s = nav.getSnapshot()
    expect(s.chip).toBeNull()
    expect(s.categoryMarkers).toEqual([])
    expect(s.categoryLayer).toBe(false)
    expect(s.input.text).toBe('')
  })

  it('forgets the category once the player types', async () => {
    const { nav, api } = setup()
    nav.selectCategory('art')
    await settle()
    nav.clearInput()
    nav.typeText('club')
    nav.submitSearch()
    await settle()
    expect(api.searchPlaces).toHaveBeenLastCalledWith(expect.objectContaining({ text: 'club', category: '' }))
  })
})

describe('map place panel', () => {
  it('opens a place, moves the camera to its first parcel and reads its live event', async () => {
    const live = event('live', 0, 0, { live: true })
    const { nav, moves } = setup({ fetchEventsAt: vi.fn(async () => [live]) })
    void nav.selectPlace(PLAZA, { fromSearch: true })
    let s = nav.getSnapshot()
    expect(s.section).toBe('place')
    expect(s.expanded).toBe(true)
    expect(s.handle).toBe('collapse')
    expect(s.input).toMatchObject({ text: 'Genesis Plaza', interactable: false, back: true, clear: true })
    expect(moves).toEqual([[{ x: 0, y: 0 }, 1]])
    await settle()
    s = nav.getSnapshot()
    expect(s.place?.liveEvent).toBe(live)
  })

  it('opens a parcel as its place, or as an empty place where there is none', async () => {
    const { nav } = setup({ fetchPlaceAt: vi.fn(async () => null) })
    await nav.selectParcel({ x: 40, y: -7 }, true)
    const view = nav.getSnapshot().place
    expect(view?.place).toMatchObject({ id: 'fake_id', title: 'Empty place', base_position: '40,-7' })
    expect(view?.origin).toEqual({ x: 40, y: -7 })
  })

  it('clears the result markers when a place is opened from outside a search', async () => {
    const { nav } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    await nav.selectPlace(CLUB)
    expect(nav.getSnapshot().searchMarkers).toEqual([])
    expect(nav.getSnapshot().input.back).toBe(false)
  })

  it('does not move the camera for a world', async () => {
    const { nav, moves } = setup()
    await nav.selectPlace(WORLD, { fromSearch: true })
    expect(moves).toEqual([])
  })

  it('opens the home parcel so that Jump In lands on it', async () => {
    const { nav } = setup()
    await nav.selectParcelAsHome({ x: 1, y: 0 })
    expect(nav.getSnapshot().place).toMatchObject({ origin: { x: 1, y: 0 }, landOnOrigin: true })
  })

  it('takes a changed place into the panel', async () => {
    const { nav } = setup()
    await nav.selectPlace(PLAZA)
    nav.updatePlace({ ...PLAZA, user_like: true })
    expect(nav.getSnapshot().place?.place.user_like).toBe(true)
    // a change to another place is not this panel's
    nav.updatePlace({ ...CLUB, user_like: true })
    expect(nav.getSnapshot().place?.place.id).toBe('plaza')
  })

  it('highlights a listed place on the map while it is pointed at, and selects it when clicked', async () => {
    const { nav } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    nav.hoverResult(CLUB, true)
    expect(nav.getSnapshot().resultHover).toBe('-20,60')
    nav.hoverResult(WORLD, true)
    expect(nav.getSnapshot().resultHover).toBe('-20,60')
    nav.clickResult(CLUB)
    expect(nav.getSnapshot()).toMatchObject({ resultSelected: '-20,60', resultHover: null, section: 'place' })
    // the markers stay while a place opened from the list is shown
    expect(nav.getSnapshot().searchMarkers).toHaveLength(2)
  })
})

describe('map event panel', () => {
  it('opens an event with its place and a back button', async () => {
    const { nav, api } = setup()
    await nav.selectEvent(event('e', 5, 6))
    expect(api.fetchPlaceAt).toHaveBeenCalledWith({ x: 5, y: 6 })
    const s = nav.getSnapshot()
    expect(s.section).toBe('event')
    expect(s.event?.place).toBe(PLAZA)
    expect(s.input).toMatchObject({ text: 'Event e', interactable: false, back: true })
  })

  it('uses the place it was opened from without reading it again', async () => {
    const { nav, api } = setup()
    await nav.selectEvent(event('e', 5, 6), CLUB)
    expect(api.fetchPlaceAt).not.toHaveBeenCalled()
    expect(nav.getSnapshot().event?.place).toBe(CLUB)
  })

  it('takes a changed event into the panel', async () => {
    const { nav } = setup()
    const e = event('e', 5, 6)
    await nav.selectEvent(e, CLUB)
    nav.updateEvent({ ...e, attending: true })
    expect(nav.getSnapshot().event?.event.attending).toBe(true)
  })
})

describe('map history', () => {
  it('goes back from a place to the search it came from, without searching again', async () => {
    const { nav, api } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    await nav.selectPlace(CLUB, { fromSearch: true })
    await nav.back()
    const s = nav.getSnapshot()
    expect(s.section).toBe('search')
    expect(s.results.places).toHaveLength(3)
    expect(s.input).toMatchObject({ text: 'a', interactable: true, back: false })
    expect(api.searchPlaces).toHaveBeenCalledTimes(1)
  })

  it('goes back from an event to its place, which no longer offers back', async () => {
    const { nav } = setup()
    await nav.selectPlace(CLUB, { fromSearch: true })
    await nav.selectEvent(event('e', -20, 60), CLUB)
    await nav.back()
    const s = nav.getSnapshot()
    expect(s.section).toBe('place')
    expect(s.place?.place).toBe(CLUB)
    expect(s.input).toMatchObject({ text: 'Sugar Club', back: false })
  })

  it('keeps one entry per kind of screen in a row', async () => {
    const { nav } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    await nav.selectPlace(CLUB, { fromSearch: true })
    await nav.selectPlace(PLAZA, { fromSearch: true })
    await nav.back()
    // one step back from the second place is the search, not the first place
    expect(nav.getSnapshot().section).toBe('search')
  })

  it('dismisses the list when back leaves nothing behind a search', async () => {
    const { nav } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    await nav.back()
    expect(nav.getSnapshot().results).toMatchObject({ shown: false, places: [] })
  })

  it('forgets the whole history when the map closes', async () => {
    const { nav } = setup()
    nav.typeText('a')
    nav.submitSearch()
    await settle()
    await nav.selectPlace(CLUB, { fromSearch: true })
    nav.deactivate()
    nav.activate()
    await nav.back()
    const s = nav.getSnapshot()
    expect(s.input.text).toBe('')
    expect(s.expanded).toBe(false)
    expect(s.results.shown).toBe(true)
  })

  it('drops a search answer that a newer search has replaced', async () => {
    let resolveFirst!: (value: { places: MapPlace[]; total: number }) => void
    const searchPlaces = vi
      .fn()
      .mockImplementationOnce(() => new Promise((resolve) => (resolveFirst = resolve)))
      .mockImplementationOnce(async () => ({ places: [CLUB], total: 1 }))
    const { nav } = setup({ searchPlaces })
    nav.typeText('first')
    nav.submitSearch()
    nav.typeText('second')
    nav.submitSearch()
    await settle()
    resolveFirst({ places: [PLAZA], total: 1 })
    await settle()
    expect(nav.getSnapshot().results.places).toEqual([CLUB])
  })

  it('tells a parcel that opened from one a newer pick replaced', async () => {
    let answerFirst!: (place: MapPlace | null) => void
    const fetchPlaceAt = vi
      .fn()
      .mockImplementationOnce(() => new Promise<MapPlace | null>((resolve) => (answerFirst = resolve)))
      .mockImplementation(async () => CLUB)
    const { nav } = setup({ fetchPlaceAt })
    const first = nav.selectParcel({ x: 0, y: 0 }, false)
    const second = nav.selectParcel({ x: -20, y: 60 }, true)
    answerFirst(PLAZA)
    expect(await first).toBe(false)
    expect(await second).toBe(true)
    expect(nav.getSnapshot().place?.place).toBe(CLUB)
  })

  it('tells a parcel replaced while its events were being read', async () => {
    let answerEvents!: (events: MapEvent[]) => void
    const fetchEventsAt = vi
      .fn()
      .mockImplementationOnce(() => new Promise<MapEvent[]>((resolve) => (answerEvents = resolve)))
      .mockImplementation(async () => [])
    const fetchPlaceAt = vi.fn().mockImplementationOnce(async () => PLAZA).mockImplementation(async () => CLUB)
    const { nav } = setup({ fetchPlaceAt, fetchEventsAt })
    const first = nav.selectParcel({ x: 0, y: 0 }, false)
    await settle()
    expect(nav.getSnapshot().place?.place).toBe(PLAZA)
    const second = nav.selectParcel({ x: -20, y: 60 }, true)
    await settle()
    answerEvents([])
    expect(await first).toBe(false)
    expect(await second).toBe(true)
    expect(nav.getSnapshot().place?.place).toBe(CLUB)
  })

  it('goes back to a place whose events arrive after an event was opened', async () => {
    let answerEvents!: (events: MapEvent[]) => void
    const fetchEventsAt = vi
      .fn()
      .mockImplementationOnce(() => new Promise<MapEvent[]>((resolve) => (answerEvents = resolve)))
      .mockImplementation(async () => [])
    const { nav } = setup({ fetchEventsAt })
    const opened = nav.selectPlace(CLUB)
    await settle()
    await nav.selectEvent(event('e', 0, 0), PLAZA)
    answerEvents([event('late', -20, 60, { live: true })])
    expect(await opened).toBe(false)
    expect(nav.getSnapshot().section).toBe('event')
    await nav.back()
    const s = nav.getSnapshot()
    expect(s.section).toBe('place')
    expect(s.place?.place).toBe(CLUB)
    expect(s.input).toMatchObject({ text: 'Sugar Club', back: false })
  })

  it('goes back to the list from a place whose events are still being read', async () => {
    let reads = 0
    const { nav } = setup({ fetchEventsAt: vi.fn(() => (reads++ === 1 ? new Promise<MapEvent[]>(() => undefined) : Promise.resolve([]))) })
    nav.typeText('club')
    nav.submitSearch()
    await settle()
    nav.clickResult(CLUB)
    await settle()
    expect(nav.getSnapshot().input.back).toBe(true)
    await nav.back()
    await settle()
    const s = nav.getSnapshot()
    expect(s.section).toBe('search')
    expect(s.results.shown).toBe(true)
    expect(s.results.places).toHaveLength(3)
  })

  it('leaves a search that is still on its way behind when the bar is cleared', async () => {
    let answer!: (result: { places: MapPlace[]; total: number }) => void
    const { nav } = setup({ searchPlaces: vi.fn(() => new Promise<{ places: MapPlace[]; total: number }>((resolve) => (answer = resolve))) })
    nav.typeText('club')
    nav.submitSearch()
    await settle()
    nav.clearInput()
    answer({ places: [CLUB], total: 1 })
    await settle()
    const s = nav.getSnapshot()
    expect(s.input.text).toBe('')
    expect(s.expanded).toBe(false)
    expect(s.searchMarkers).toEqual([])
    expect(s.results).toMatchObject({ loading: false, places: [] })
    // nothing entered the history
    await nav.back()
    expect(nav.getSnapshot().input.text).toBe('')
  })

  it('leaves an event whose place arrives after a place was opened out of the panel', async () => {
    let answerPlace!: (place: MapPlace | null) => void
    const { nav } = setup({ fetchPlaceAt: vi.fn(() => new Promise<MapPlace | null>((resolve) => (answerPlace = resolve))) })
    void nav.selectEvent(event('e', 0, 0))
    await settle()
    await nav.selectPlace(CLUB)
    answerPlace(PLAZA)
    await settle()
    const s = nav.getSnapshot()
    expect(s.section).toBe('place')
    expect(s.input.text).toBe('Sugar Club')
    expect(s.event).toBeNull()
    await nav.back()
    await settle()
    expect(nav.getSnapshot().section).toBe('place')
    expect(nav.getSnapshot().input).toMatchObject({ interactable: true, back: false })
  })
})

describe('map panel handle', () => {
  it('leaves a handle to bring the panel back when the player collapses it', async () => {
    const { nav } = setup()
    await nav.selectPlace(PLAZA, { fromSearch: true })
    nav.collapse()
    expect(nav.getSnapshot()).toMatchObject({ expanded: false, handle: 'expand' })
    expect(nav.getSnapshot().input.back).toBe(false)
    nav.expand()
    expect(nav.getSnapshot()).toMatchObject({ expanded: true, handle: 'collapse' })
  })
})

describe('home', () => {
  it('reads a pinned default-realm home as a parcel and any other realm as a world', () => {
    expect(homeOf({ realm: 'main', parcel: { x: 3, y: 4 }, pinned: true }, 'main')).toEqual({ parcel: { x: 3, y: 4 }, world: null })
    expect(homeOf({ realm: 'mansion.dcl.eth', parcel: { x: 0, y: 0 }, pinned: true }, 'main')).toEqual({ parcel: null, world: 'mansion.dcl.eth' })
    expect(homeOf({ realm: 'main', parcel: { x: 0, y: 0 }, pinned: false }, 'main')).toEqual(NO_HOME)
  })

  it('counts any parcel of a place as home, and a world by its name', () => {
    expect(isHomePlace({ parcel: { x: 1, y: 0 }, world: null }, PLAZA)).toBe(true)
    expect(isHomePlace({ parcel: { x: 9, y: 9 }, world: null }, PLAZA)).toBe(false)
    expect(isHomePlace({ parcel: null, world: 'mansion.dcl.eth' }, WORLD)).toBe(true)
    expect(isHomePlace({ parcel: null, world: 'mansion.dcl.eth' }, PLAZA)).toBe(false)
    expect(isHomePlace(NO_HOME, PLAZA)).toBe(false)
  })

  it('shows a change at once and sends it to the engine', () => {
    const store = createHomeStore()
    const sent: unknown[] = []
    store.setTransport((change) => sent.push(change))
    store.set({ parcel: { x: 2, y: 3 } })
    expect(store.getSnapshot()).toEqual({ parcel: { x: 2, y: 3 }, world: null })
    store.set({ world: 'mansion.dcl.eth' })
    expect(store.getSnapshot()).toEqual({ parcel: null, world: 'mansion.dcl.eth' })
    store.set(null)
    expect(store.getSnapshot()).toEqual(NO_HOME)
    expect(sent).toEqual([{ parcel: { x: 2, y: 3 } }, { world: 'mansion.dcl.eth' }, null])
  })

  it('holds no home once the engine is gone', () => {
    const store = createHomeStore()
    const heard = vi.fn()
    store.subscribe(heard)
    store.setTransport(() => undefined)
    store.set({ parcel: { x: 2, y: 3 } })
    store.setTransport(null)
    expect(store.getSnapshot()).toEqual(NO_HOME)
    expect(heard).toHaveBeenCalledTimes(2)
  })
})

describe('navigation', () => {
  it('routes to the only parcel of a one-parcel place and to the base of a larger one', () => {
    const store = createNavigationStore()
    store.start(CLUB)
    expect(store.getSnapshot()).toMatchObject({ parcel: { x: -20, y: 60 }, title: 'Sugar Club (-20, 60)', position: parcelCenter({ x: -20, y: 60 }) })
    store.start(place('big', 'Big', ['5,5', '6,5'], { base_position: '6,5' }))
    expect(store.getSnapshot()?.parcel).toEqual({ x: 6, y: 5 })
    expect(destinationParcel(CLUB)).toEqual({ x: -20, y: 60 })
    expect(destinationParcel(place('big', 'Big', ['5,5', '6,5'], { base_position: '6,5' }))).toEqual({ x: 6, y: 5 })
    expect(destinationParcel(null)).toEqual({ x: 0, y: 0 })
  })

  it('ends when the player reaches the destination, and not before', () => {
    const store = createNavigationStore()
    store.start(CLUB)
    const destination = parcelCenter({ x: -20, y: 60 })
    expect(store.playerMoved({ x: 0, y: 0 })).toBe(false)
    expect(store.getSnapshot()).not.toBeNull()
    expect(store.playerMoved({ x: destination.x + 5, y: destination.y + 4 })).toBe(true)
    expect(store.getSnapshot()).toBeNull()
  })

  it('ends when stopped', () => {
    const store = createNavigationStore()
    store.start(CLUB)
    store.stop()
    expect(store.getSnapshot()).toBeNull()
  })
})

describe('share links', () => {
  it('adds the sharer to a jump-in link when there is one', () => {
    expect(jumpInLink(10, -5, undefined)).toMatch(/\/jump\/\?position=10,-5$/)
    expect(jumpInLink(10, -5, '0xabc')).toMatch(/\/jump\/\?position=10,-5&referrer=0xabc$/)
  })

  it("shares a place by its base parcel, with its name in the post", () => {
    const links = placeShareLinks(CLUB, '0xabc')
    expect(links.copy).toContain('position=-20,60&referrer=0xabc')
    expect(links.x).toContain('https://twitter.com/intent/tweet?text=Check%20out%20Sugar%20Club')
    expect(links.x).toContain('&hashtags=DCLPlace&url=')
    expect(links.x).toContain(encodeURIComponent(links.copy))
  })

  it('shares a live event as its place and one yet to start as its own page', () => {
    expect(eventShareLinks(event('e', 5, 6, { live: true }), undefined).copy).toMatch(/\/jump\/\?position=5,6$/)
    expect(eventShareLinks(event('e', 5, 6), '0xabc').copy).toMatch(/\/whats-on\/\?id=e$/)
  })

  it('says how long ago a live event started, and when another starts', () => {
    const start = Date.parse('2026-11-10T10:00:00.000Z')
    const live = event('e', 0, 0, { live: true })
    expect(eventSchedule(live, start + 5 * 60_000)).toBe('Event started 5 min ago')
    expect(eventSchedule(live, start + 3 * 3_600_000)).toBe('Event started 3 hour ago')
    expect(eventSchedule(live, start + 2 * 86_400_000)).toBe('Event started 2 day ago')
    expect(eventSchedule(event('e', 0, 0))).toBe('Tue, 10 Nov 2026 10:00:00 GMT')
  })

  it('lists the dates an event repeats on from its next one', () => {
    const e = event('e', 0, 0, {
      next_start_at: '2026-11-11T10:00:00.000Z',
      recurrent_dates: ['2026-11-10T10:00:00.000Z', '2026-11-11T10:00:00.000Z', '2026-11-12T10:00:00.000Z', 'nonsense']
    })
    expect(upcomingDates(e).map((d) => d.toISOString())).toEqual(['2026-11-11T10:00:00.000Z', '2026-11-12T10:00:00.000Z'])
  })

  it('writes a calendar entry as long as the event runs', () => {
    const e = event('e', 5, 6, { name: 'Party & more', next_start_at: '2026-11-11T10:00:00.000Z', next_finish_at: '2026-11-11T12:30:00.000Z' })
    const link = calendarLink(e, new Date('2026-11-12T10:00:00.000Z'), undefined)
    expect(link).toContain('https://www.google.com/calendar/event?action=TEMPLATE')
    expect(link).toContain('&text=Party%20%26%20more')
    expect(link).toContain('&dates=20261112T100000Z/20261112T123000Z')
    expect(link).toContain(encodeURIComponent('jump in: '))
  })
})

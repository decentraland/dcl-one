import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { DEFAULT_REALM } from '@hud-host'
import type { SceneToPage } from '@explorer-core/engine/protocol'
import { fixMapPublicReads, receiveMapReply, setMapTransport, type MapRequest } from '@explorer-core/features/map/mapApi'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import type { EventData } from '@explorer-core/engine/protocol'
import { mapLayers } from '@explorer-core/features/map/mapLayersStore'
import { navigation } from '@explorer-core/features/map/navigation'
import { eventOf, placeOf } from '@explorer-core/test/mapFixtures'
import { MapPage } from '../../features/map/MapPage'
import type { MapState, TravelOutcome } from '../../features/session/useEngineSession'
import { fakeProfileState, fakeSession } from '../harness'

const place = (id: string, title: string, positions: string[], extra: Partial<DiscoverPlace> = {}): DiscoverPlace =>
  placeOf({
    id,
    title,
    description: `About ${title}`,
    contact_name: 'Creator',
    positions,
    categories: ['social'],
    likes: 3,
    dislikes: 1,
    like_rate: 0.75,
    user_count: 7,
    ...extra
  })
const event = (id: string, name: string, x: number, y: number, extra: Partial<EventData> = {}): EventData =>
  eventOf({ id, name, x, y, description: `About ${name}`, user_name: 'Host', total_attendees: 4, start_at: '2026-11-10T10:00:00.000Z', ...extra })

const PLAZA = place('plaza', 'Genesis Plaza', ['0,0'])
const CLUB = place('club', 'Sugar Club', ['-20,60'])

interface Services {
  /** Places by the parcel they cover. */
  places?: DiscoverPlace[]
  search?: DiscoverPlace[]
  total?: number
  events?: EventData[]
  photos?: number
  /** Requests the services refuse. */
  fail?: MapRequest['kind'][]
}

/** Answer the map's requests to the bridge from fixed data, and record them. */
function serve(services: Services = {}): MapRequest[] {
  const seen: MapRequest[] = []
  const failed = (request: MapRequest): boolean => services.fail?.includes(request.kind) === true
  const answer = (request: MapRequest): SceneToPage => {
    switch (request.kind) {
      case 'getPlacesAt':
        return { kind: 'placesAt', request: request.request, places: (services.places ?? []).filter((p) => p.positions.some((pos) => request.positions.includes(pos))), failed: failed(request) }
      case 'searchPlaces':
        return { kind: 'placeSearch', request: request.request, places: services.search ?? [], total: services.total ?? services.search?.length ?? 0, failed: failed(request) }
      case 'getEvents': {
        const live = request.list === 'live'
        return { kind: 'events', request: request.request, events: (services.events ?? []).filter((e) => !live || e.live), failed: failed(request) }
      }
      case 'setPlaceLike':
        return { kind: 'placeLike', request: request.request, likes: 4, dislikes: 1, user_like: request.like === true, user_dislike: request.like === false, failed: failed(request) }
      case 'setPlaceFavorite':
        return { kind: 'placeFavorite', request: request.request, favorites: 1, user_favorite: request.favorite, failed: failed(request) }
      case 'setEventAttending':
        return { kind: 'eventAttending', eventId: request.eventId, attending: request.attending, failed: failed(request) }
      case 'getPlacePhotos':
        return {
          kind: 'placePhotos',
          request: request.request,
          images: Array.from({ length: services.photos ?? 0 }, (_, i) => ({ id: `photo-${i}`, url: `https://photos.test/${i}.jpg` })),
          total: services.photos ?? 0,
          failed: failed(request)
        }
    }
  }
  setMapTransport((request) => {
    seen.push(request)
    queueMicrotask(() => receiveMapReply(answer(request)))
  })
  return seen
}

function renderMap(over: Partial<MapState> = {}, genesis = true, compact?: { onTravel: () => void }): MapState {
  const session = fakeSession()
  const map: MapState = { ...session.map, open: true, teleportToPlace: vi.fn(() => 'sent' as const), changeRealm: vi.fn(() => 'sent' as const), toggle: vi.fn(), setHome: vi.fn(), ...over }
  const minimap = genesis ? session.minimap : { ...session.minimap, satelliteView: null }
  render(<MapPage map={map} minimap={minimap} profile={fakeProfileState()} gallery={session.gallery} compact={compact} onNavigate={vi.fn()} />)
  return map
}

const view = (): HTMLElement => screen.getByTestId('map-view')
const marker = (name: string): HTMLElement => within(view()).getByRole('button', { name })
const panel = (): HTMLElement => screen.getByRole('complementary', { name: 'Places and events' })

async function search(text: string): Promise<void> {
  const field = screen.getByRole('textbox', { name: 'Search' })
  await userEvent.type(field, `${text}{Enter}`)
}

beforeEach(() => {
  fixMapPublicReads({ pois: [], peers: [] })
})

afterEach(() => {
  vi.restoreAllMocks()
  setMapTransport(null)
  fixMapPublicReads(null)
  for (const layer of ['liveEvents', 'pois', 'people'] as const) mapLayers.set(layer, true)
  mapLayers.setMapType('satellite')
  navigation.stop()
})

describe('map page controls', () => {
  it('offers the category chips, the search field and the map controls', () => {
    serve()
    renderMap()
    const chips = screen.getByRole('group', { name: 'Categories' })
    expect(within(chips).getAllByRole('button').map((chip) => chip.textContent)).toEqual([
      'All',
      'Favorites',
      'Social',
      'Music',
      'Art',
      'Game',
      'Fashion',
      'Education',
      'Shop',
      'Sports',
      'Business'
    ])
    expect(screen.getByRole('textbox', { name: 'Search' })).toBeInTheDocument()
    const controls = screen.getByRole('toolbar', { name: 'Map controls' })
    for (const name of ['Layers', 'Center on home', 'Center on me', 'Zoom in', 'Zoom out']) {
      expect(within(controls).getByRole('button', { name })).toBeInTheDocument()
    }
  })

  it('stops zooming at the closest and the farthest view', async () => {
    serve()
    vi.useFakeTimers({ shouldAdvanceTime: true })
    renderMap()
    const zoomIn = screen.getByRole('button', { name: 'Zoom in' })
    for (let i = 0; i < 3; i++) {
      fireEvent.click(zoomIn)
      await act(async () => {
        await vi.advanceTimersByTimeAsync(400)
      })
    }
    expect(zoomIn).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Zoom out' })).toBeEnabled()
    vi.useRealTimers()
  })

  it('warns that the map is Genesis City while the player is in a world, until dismissed', async () => {
    serve()
    renderMap({}, false)
    expect(screen.getByText(/This is the Genesis City map/)).toBeInTheDocument()
    expect(within(view()).queryByRole('img', { name: 'You are here' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByText(/This is the Genesis City map/)).not.toBeInTheDocument()
  })

  it('shows the player on the map in Genesis City, with no warning', () => {
    serve()
    renderMap()
    expect(within(view()).getByRole('img', { name: 'You are here' })).toBeInTheDocument()
    expect(screen.queryByText(/This is the Genesis City map/)).not.toBeInTheDocument()
  })
})

describe('map layers', () => {
  it('marks live events and points of interest, and hides a layer switched off', async () => {
    fixMapPublicReads({ pois: [{ x: 0, y: 0 }], peers: [] })
    serve({ places: [PLAZA], events: [event('party', 'Friday Party', -20, 60, { live: true })] })
    renderMap()
    expect(await within(view()).findByRole('button', { name: 'Genesis Plaza' })).toBeInTheDocument()
    expect(await within(view()).findByRole('button', { name: 'Friday Party' })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Layers' }))
    const layers = screen.getByRole('region', { name: 'Layers' })
    await userEvent.click(within(layers).getByRole('switch', { name: 'Points of interest' }))
    expect(within(view()).queryByRole('button', { name: 'Genesis Plaza' })).not.toBeInTheDocument()
    expect(marker('Friday Party')).toBeInTheDocument()
    await userEvent.click(within(layers).getByRole('switch', { name: 'Live Events' }))
    expect(within(view()).queryByRole('button', { name: 'Friday Party' })).not.toBeInTheDocument()
  })

  it('leaves a live event in a world off the map', async () => {
    serve({ events: [event('w', 'World Party', 0, 0, { live: true, world: true }), event('g', 'City Party', 5, 5, { live: true })] })
    renderMap()
    expect(await within(view()).findByRole('button', { name: 'City Party' })).toBeInTheDocument()
    expect(within(view()).queryByRole('button', { name: 'World Party' })).not.toBeInTheDocument()
  })

  it('starts with every pin layer on and the satellite map, and switches to parcels', async () => {
    serve()
    renderMap()
    await userEvent.click(screen.getByRole('button', { name: 'Layers' }))
    const layers = screen.getByRole('region', { name: 'Layers' })
    for (const name of ['Live Events', 'Points of interest', 'People']) {
      expect(within(layers).getByRole('switch', { name })).toBeChecked()
    }
    expect(within(layers).getByRole('radio', { name: 'Satellite' })).toBeChecked()
    await userEvent.click(within(layers).getByRole('radio', { name: 'Parcels' }))
    expect(within(layers).getByRole('radio', { name: 'Parcels' })).toBeChecked()
    expect(within(layers).getByRole('radio', { name: 'Satellite' })).not.toBeChecked()
    expect(view().querySelector('img[src*="/v1/map.png"]')).not.toBeNull()
  })
})

describe('map search', () => {
  it('lists the results of a search with their live badges, and marks them on the map', async () => {
    serve({ search: [PLAZA, CLUB], events: [event('party', 'Friday Party', -20, 60, { live: true })] })
    renderMap()
    await search('club')
    const results = await within(panel()).findByRole('list', { name: 'Results' })
    const rows = within(results).getAllByRole('button')
    expect(rows).toHaveLength(2)
    expect(within(rows[0]).queryByText('LIVE')).not.toBeInTheDocument()
    expect(within(rows[1]).getByText('LIVE')).toBeInTheDocument()
    expect(marker('Sugar Club')).toBeInTheDocument()
  })

  it('says so when a search finds nothing', async () => {
    serve({ search: [] })
    renderMap()
    await search('nowhere')
    expect(await within(panel()).findByText('No results founds')).toBeInTheDocument()
    expect(within(panel()).getByText('Make sure your search is spelled correctly.')).toBeInTheDocument()
  })

  it('shows placeholders until the search answers', async () => {
    let answer: (() => void) | null = null
    setMapTransport((request) => {
      if (request.kind === 'searchPlaces') answer = () => receiveMapReply({ kind: 'placeSearch', request: request.request, places: [PLAZA], total: 1 })
      else if (request.kind === 'getEvents') queueMicrotask(() => receiveMapReply({ kind: 'events', request: request.request, events: [] }))
      else if (request.kind === 'getPlacesAt') queueMicrotask(() => receiveMapReply({ kind: 'placesAt', request: request.request, places: [] }))
    })
    renderMap()
    await search('plaza')
    expect(within(panel()).getByRole('list', { name: 'Loading results' })).toBeInTheDocument()
    act(() => answer?.())
    expect(await within(panel()).findByRole('list', { name: 'Results' })).toBeInTheDocument()
  })

  it('searches again by another sort', async () => {
    const seen = serve({ search: [PLAZA] })
    renderMap()
    await search('plaza')
    await within(panel()).findByRole('list', { name: 'Results' })
    await userEvent.click(within(panel()).getByRole('button', { name: 'Newest' }))
    await waitFor(() => expect(seen.filter((r) => r.kind === 'searchPlaces').at(-1)).toMatchObject({ orderBy: 'created_at' }))
    expect(within(panel()).getByRole('button', { name: 'Newest' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('pages through more than fifty results', async () => {
    const seen = serve({ search: [PLAZA], total: 120 })
    renderMap()
    await search('plaza')
    await within(panel()).findByRole('list', { name: 'Results' })
    expect(within(panel()).getByRole('button', { name: 'Prev' })).toBeDisabled()
    await userEvent.click(within(panel()).getByRole('button', { name: 'Next' }))
    await waitFor(() => expect(seen.filter((r) => r.kind === 'searchPlaces').at(-1)).toMatchObject({ offset: 50 }))
  })

  it('searches a category from its chip and shows its name in the read-only field', async () => {
    const seen = serve({ search: [CLUB] })
    renderMap()
    await userEvent.click(screen.getByRole('button', { name: 'Music' }))
    expect(screen.getByRole('button', { name: 'Music' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(seen.find((r) => r.kind === 'searchPlaces')).toMatchObject({ category: 'music' }))
    const field = screen.getByRole('textbox', { name: 'Search' })
    expect(field).toHaveValue('Music')
    expect(field).toHaveAttribute('readonly')
    expect(await within(view()).findByRole('button', { name: 'Sugar Club' })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Music' }))
    expect(field).toHaveValue('')
    expect(within(view()).queryByRole('button', { name: 'Sugar Club' })).not.toBeInTheDocument()
  })

  it('clears the search from its clear button', async () => {
    serve({ search: [PLAZA] })
    renderMap()
    await search('plaza')
    await within(panel()).findByRole('list', { name: 'Results' })
    await userEvent.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(screen.getByRole('textbox', { name: 'Search' })).toHaveValue('')
    expect(within(view()).queryByRole('button', { name: 'Genesis Plaza' })).not.toBeInTheDocument()
  })
})

describe('map place panel', () => {
  it('opens the place under a clicked parcel and jumps in to it, closing the map', async () => {
    serve({ places: [place('p', 'Plaza', ['10,-5'])] })
    const map = renderMap()
    // 960,505 is the middle of the view, which opens on parcel 0,0: 10 parcels east and 5 south
    // of it at the opening zoom (7.37 px per parcel)
    fireEvent.click(view(), { clientX: 960 + 9.5 * 7.372, clientY: 505 + 5.5 * 7.372 })
    expect(await within(panel()).findByRole('heading', { name: 'Plaza' })).toBeInTheDocument()
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(vi.mocked(map.teleportToPlace)).toHaveBeenCalledWith(10, -5)
    expect(vi.mocked(map.toggle)).toHaveBeenCalledTimes(1)
  })

  it('shows an empty place for a parcel nothing is deployed on, with favourite blocked', async () => {
    serve()
    renderMap()
    fireEvent.click(view(), { clientX: 960, clientY: 505 })
    expect(await within(panel()).findByRole('heading', { name: 'Empty place' })).toBeInTheDocument()
    expect(within(panel()).getByRole('button', { name: 'Favorite' })).toBeDisabled()
    expect(within(panel()).getByText('No description')).toBeInTheDocument()
  })

  it('shows what the place is, who is there and where it appears', async () => {
    serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const p = panel()
    expect(await within(p).findByRole('heading', { name: 'Sugar Club' })).toBeInTheDocument()
    expect(within(p).getByText('75%')).toBeInTheDocument()
    expect(within(p).getByText('-20,60')).toBeInTheDocument()
    expect(within(p).getByText('About Sugar Club')).toBeInTheDocument()
    expect(within(p).getByText('social')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Search' })).toHaveValue('Sugar Club')
  })

  it('goes back from a place to the results it was picked from', async () => {
    serve({ search: [PLAZA, CLUB] })
    renderMap()
    await search('a')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await within(panel()).findByRole('heading', { name: 'Sugar Club' })
    await userEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect(await within(panel()).findByRole('list', { name: 'Results' })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Search' })).toHaveValue('a')
  })

  it('likes a place at once and takes the new rating from the service', async () => {
    const seen = serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const like = await within(panel()).findByRole('button', { name: 'Like' })
    await userEvent.click(like)
    expect(like).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(within(panel()).getByText('80%')).toBeInTheDocument())
    expect(seen).toContainEqual(expect.objectContaining({ kind: 'setPlaceLike', placeId: 'club', like: true }))

    await userEvent.click(like)
    expect(seen).toContainEqual(expect.objectContaining({ kind: 'setPlaceLike', placeId: 'club', like: null }))
  })

  it('puts a like back when the service refuses it', async () => {
    serve({ search: [CLUB], fail: ['setPlaceLike'] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const dislike = await within(panel()).findByRole('button', { name: 'Dislike' })
    await userEvent.click(dislike)
    await waitFor(() => expect(dislike).toHaveAttribute('aria-pressed', 'false'))
    expect(within(panel()).getByText('75%')).toBeInTheDocument()
  })

  it('favourites a place, and puts it back when the service refuses', async () => {
    const seen = serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const favourite = await within(panel()).findByRole('button', { name: 'Favorite' })
    await userEvent.click(favourite)
    expect(favourite).toHaveAttribute('aria-pressed', 'true')
    expect(seen).toContainEqual(expect.objectContaining({ kind: 'setPlaceFavorite', placeId: 'club', favorite: true }))

    serve({ search: [CLUB], fail: ['setPlaceFavorite'] })
    await userEvent.click(favourite)
    await waitFor(() => expect(favourite).toHaveAttribute('aria-pressed', 'true'))
  })

  it('makes a place home', async () => {
    serve({ search: [CLUB], places: [CLUB] })
    const map = renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const home = await within(panel()).findByRole('button', { name: 'Set as Home' })
    expect(home).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(home)
    expect(vi.mocked(map.setHome)).toHaveBeenCalledWith(DEFAULT_REALM, { x: -20, y: 60 })
  })

  it('marks the home on the map, and keeps it home when pressed again', async () => {
    serve({ search: [CLUB], places: [CLUB] })
    const map = renderMap({ home: { realm: null, parcel: '-20,60' } })
    expect(marker('Home')).toBeInTheDocument()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const home = await within(panel()).findByRole('button', { name: 'Set as Home' })
    expect(home).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(home)
    expect(vi.mocked(map.setHome)).not.toHaveBeenCalled()
  })

  it('makes a world home by its name, and jumps in to it by its realm', async () => {
    const world = place('w', 'Mansion', ['0,0'], { world_name: 'mansion.dcl.eth' })
    serve({ search: [world] })
    const map = renderMap()
    await search('mansion')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Mansion/ }))
    await within(panel()).findByRole('heading', { name: 'Mansion' })
    expect(within(panel()).queryByRole('button', { name: 'start navigation' })).not.toBeInTheDocument()
    expect(within(panel()).getByText('mansion.dcl.eth')).toBeInTheDocument()
    await userEvent.click(within(panel()).getByRole('button', { name: 'Set as Home' }))
    expect(vi.mocked(map.setHome)).toHaveBeenCalledWith('mansion.dcl.eth', { x: 0, y: 0 })
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(vi.mocked(map.changeRealm)).toHaveBeenCalledWith('mansion.dcl.eth')
  })

  it('opens the home place from the home control', async () => {
    serve({ places: [CLUB] })
    renderMap({ home: { realm: null, parcel: '-20,60' } })
    await userEvent.click(screen.getByRole('button', { name: 'Center on home' }))
    expect(await within(panel()).findByRole('heading', { name: 'Sugar Club' })).toBeInTheDocument()
    expect(within(panel()).getByRole('button', { name: 'Set as Home' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('offers to post the place on X or copy its link', async () => {
    serve({ search: [CLUB] })
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'Share' }))
    const menu = within(panel()).getByRole('menu')
    expect(within(menu).getByRole('menuitem', { name: 'Share on X' })).toBeInTheDocument()
    // the focus stays where the player moved it when something else on the page changes
    const copy = within(menu).getByRole('menuitem', { name: 'Copy Link' })
    copy.focus()
    act(() => mapLayers.set('people', false))
    expect(copy).toHaveFocus()
    fireEvent.click(copy)
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining('/jump/?position=-20,60'))
    expect(within(panel()).queryByRole('menu')).not.toBeInTheDocument()
    vi.unstubAllGlobals()
  })

  it("lists the place's photos with their count, and says so when there are none", async () => {
    serve({ search: [CLUB], photos: 2 })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('tab', { name: 'photos' }))
    expect(await within(panel()).findByRole('tab', { name: 'photos (2)' })).toBeInTheDocument()
    expect(within(panel()).getAllByRole('button', { name: 'Open photo' })).toHaveLength(2)

    serve({ search: [CLUB], photos: 0 })
    await userEvent.click(within(panel()).getByRole('tab', { name: 'overview' }))
    await userEvent.click(within(panel()).getByRole('tab', { name: 'photos' }))
    expect(await within(panel()).findByText('No photos here yet!')).toBeInTheDocument()
  })

  it("lists the place's events, or says there are none", async () => {
    serve({ search: [CLUB], events: [event('later', 'Trivia Night', -20, 60)] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('tab', { name: 'events' }))
    expect(await within(panel()).findByRole('button', { name: 'Trivia Night' })).toBeInTheDocument()

    serve({ search: [CLUB], events: [] })
    await userEvent.click(within(panel()).getByRole('tab', { name: 'overview' }))
    await userEvent.click(within(panel()).getByRole('tab', { name: 'events' }))
    expect(await within(panel()).findByText('No Upcoming Events')).toBeInTheDocument()
  })

  it('names the live event of a place', async () => {
    serve({ search: [CLUB], events: [event('party', 'Friday Party', -20, 60, { live: true })] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await within(panel()).findByRole('heading', { name: 'Sugar Club' })
    expect(await within(panel()).findByText('Friday Party')).toBeInTheDocument()
  })
})

describe('map event panel', () => {
  it('opens a live event from its marker and jumps in to it', async () => {
    serve({ places: [CLUB], events: [event('party', 'Friday Party', -20, 60, { live: true })] })
    const map = renderMap()
    await userEvent.click(await within(view()).findByRole('button', { name: 'Friday Party' }))
    const p = panel()
    expect(await within(p).findByRole('heading', { name: 'Friday Party' })).toBeInTheDocument()
    expect(within(p).getByText('Sugar Club (-20, 60)')).toBeInTheDocument()
    expect(within(p).queryByRole('button', { name: 'remind me' })).not.toBeInTheDocument()
    await userEvent.click(within(p).getByRole('button', { name: 'JUMP IN' }))
    expect(vi.mocked(map.teleportToPlace)).toHaveBeenCalledWith(-20, 60)
    expect(vi.mocked(map.toggle)).toHaveBeenCalledTimes(1)
  })

  it('remembers the interest in an event yet to start, and its dates for the calendar', async () => {
    const later = event('later', 'Trivia Night', -20, 60, {
      next_start_at: '2030-01-01T10:00:00.000Z',
      next_finish_at: '2030-01-01T11:00:00.000Z',
      recurrent_dates: ['2030-01-01T10:00:00.000Z', '2030-01-08T10:00:00.000Z']
    })
    const seen = serve({ search: [CLUB], events: [later] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('tab', { name: 'events' }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'Trivia Night' }))

    const p = panel()
    expect(await within(p).findByRole('heading', { name: 'Trivia Night' })).toBeInTheDocument()
    expect(within(p).queryByRole('button', { name: 'JUMP IN' })).not.toBeInTheDocument()
    const remind = within(p).getByRole('button', { name: 'remind me' })
    await userEvent.click(remind)
    expect(remind).toHaveAttribute('aria-pressed', 'true')
    expect(seen).toContainEqual({ kind: 'setEventAttending', eventId: 'later', attending: true })
    const links = within(p).getAllByRole('link', { name: 'add to calendar' })
    expect(links).toHaveLength(2)
    expect(links[0]).toHaveAttribute('href', expect.stringContaining('dates=20300101T100000Z/20300101T110000Z'))

    await userEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect(await within(panel()).findByRole('heading', { name: 'Sugar Club' })).toBeInTheDocument()
  })

  it('puts the interest back when the service refuses it', async () => {
    serve({ search: [CLUB], events: [event('later', 'Trivia Night', -20, 60)], fail: ['setEventAttending'] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('tab', { name: 'events' }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'Trivia Night' }))
    const remind = await within(panel()).findByRole('button', { name: 'remind me' })
    await userEvent.click(remind)
    await waitFor(() => expect(remind).toHaveAttribute('aria-pressed', 'false'))
  })
})

describe('map navigation', () => {
  it('starts a route to a place, names it at the bottom of the map, and ends it', async () => {
    serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'start navigation' }))
    expect(screen.getByText('DESTINATION')).toBeInTheDocument()
    expect(screen.getByText('Sugar Club (-20, 60)')).toBeInTheDocument()
    expect(within(view()).getByRole('img', { name: 'Destination' })).toBeInTheDocument()
    expect(within(panel()).getByRole('button', { name: 'exit navigation' })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Exit' }))
    expect(screen.queryByText('DESTINATION')).not.toBeInTheDocument()
    expect(within(view()).queryByRole('img', { name: 'Destination' })).not.toBeInTheDocument()
  })

  it('ends the route when the player jumps in to its place', async () => {
    serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'start navigation' }))
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(navigation.getSnapshot()).toBeNull()
  })

  it('ends the route to a one-parcel place whose base is elsewhere, as the place detail does', async () => {
    serve({ search: [{ ...CLUB, base_position: '-21,60' }] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'start navigation' }))
    expect(navigation.getSnapshot()?.parcel).toEqual({ x: -20, y: 60 })
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(navigation.getSnapshot()).toBeNull()
  })

  it("keeps the search and the route in the lobby's modal when the startup lobby refuses the jump", async () => {
    serve({ search: [CLUB] })
    const onTravel = vi.fn()
    const teleportToPlace = vi.fn((): TravelOutcome => 'refused')
    const map = renderMap({ teleportToPlace }, true, { onTravel })
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'start navigation' }))
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(teleportToPlace).toHaveBeenCalledWith(-20, 60)
    expect(onTravel).not.toHaveBeenCalled()
    expect(vi.mocked(map.toggle)).not.toHaveBeenCalled()
    expect(navigation.getSnapshot()).not.toBeNull()
    expect(screen.getByRole('textbox', { name: 'Search' })).toHaveValue('Sugar Club')
    teleportToPlace.mockReturnValue('sent')
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(onTravel).toHaveBeenCalledOnce()
    expect(navigation.getSnapshot()).toBeNull()
    expect(screen.getByRole('textbox', { name: 'Search' })).toHaveValue('')
  })

  it('offers to start or exit the route by whether one leads to the place', async () => {
    serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    await userEvent.click(await within(panel()).findByRole('button', { name: 'start navigation' }))
    // the route ending elsewhere than in the panel, as on arrival
    act(() => navigation.stop())
    expect(within(panel()).getByRole('button', { name: 'start navigation' })).toBeInTheDocument()

    await userEvent.click(within(panel()).getByRole('button', { name: 'start navigation' }))
    await userEvent.click(within(panel()).getByRole('button', { name: 'Back' }))
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    expect(await within(panel()).findByRole('button', { name: 'exit navigation' })).toBeInTheDocument()
  })
})

describe('map view size', () => {
  it('follows the size of the view each time the map opens', () => {
    const watched: Element[] = []
    const dropped: Element[][] = []
    class Observer {
      private readonly seen: Element[] = []
      observe(el: Element): void {
        this.seen.push(el)
        watched.push(el)
      }
      unobserve(): void {}
      disconnect(): void {
        dropped.push(this.seen)
      }
    }
    vi.stubGlobal('ResizeObserver', Observer)
    serve()
    const session = fakeSession()
    const closed: MapState = { ...session.map, open: false, teleportToPlace: vi.fn(() => 'sent' as const), changeRealm: vi.fn(() => 'sent' as const), toggle: vi.fn() }
    const rest = { minimap: session.minimap, profile: fakeProfileState(), gallery: session.gallery, onNavigate: vi.fn() }
    // the page is mounted with the map closed, as the HUD mounts it
    const { rerender } = render(<MapPage map={closed} {...rest} />)
    expect(watched).toEqual([])
    rerender(<MapPage map={{ ...closed, open: true }} {...rest} />)
    const first = view()
    expect(watched).toEqual([first])

    rerender(<MapPage map={closed} {...rest} />)
    expect(dropped).toEqual([[first]])
    rerender(<MapPage map={{ ...closed, open: true }} {...rest} />)
    expect(watched).toEqual([first, view()])
    vi.unstubAllGlobals()
  })
})

describe('map live events', () => {
  it('stops reading them when the page goes', () => {
    const started = vi.spyOn(globalThis, 'setInterval')
    const stopped = vi.spyOn(globalThis, 'clearInterval')
    serve()
    const session = fakeSession()
    const map: MapState = { ...session.map, open: true, teleportToPlace: vi.fn(() => 'sent' as const), changeRealm: vi.fn(() => 'sent' as const), toggle: vi.fn() }
    const { unmount } = render(<MapPage map={map} minimap={session.minimap} profile={fakeProfileState()} gallery={session.gallery} onNavigate={vi.fn()} />)
    const polls = started.mock.results.filter((_, i) => started.mock.calls[i][1] === 5 * 60 * 1000).map((r) => r.value as unknown)
    expect(polls).toHaveLength(1)
    unmount()
    expect(stopped.mock.calls.map((c) => c[0] as unknown)).toContain(polls[0])
  })
})

describe('map panel', () => {
  it('collapses and comes back from its handle', async () => {
    serve({ search: [CLUB] })
    renderMap()
    await search('club')
    await within(panel()).findByRole('list', { name: 'Results' })
    await userEvent.click(screen.getByRole('button', { name: 'Collapse panel' }))
    expect(screen.getByRole('button', { name: 'Expand panel' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Expand panel' }))
    expect(screen.getByRole('button', { name: 'Collapse panel' })).toBeInTheDocument()
  })
})

import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { MapApiRequest } from '@explorer-core/engine/protocol'
import { homeStore, NO_HOME } from '@explorer-core/features/map/home'
import { fixMapPublicReads, resolveMapApi, setMapApiTransport, type MapEvent, type MapPlace } from '@explorer-core/features/map/mapApi'
import { mapLayers } from '@explorer-core/features/map/mapLayersStore'
import { navigation } from '@explorer-core/features/map/navigation'
import { scenePins } from '@explorer-core/features/map/scenePins'
import { MapPage } from '../../features/map/MapPage'
import type { MapState } from '../../features/session/useEngineSession'
import { fakeProfileState, fakeSession } from '../harness'

const place = (id: string, title: string, positions: string[], extra: Partial<MapPlace> = {}): MapPlace => ({
  id,
  title,
  description: `About ${title}`,
  image: '',
  contact_name: 'Creator',
  positions,
  base_position: positions[0],
  categories: ['social'],
  likes: 3,
  dislikes: 1,
  like_rate: 0.75,
  user_count: 7,
  ...extra
})
const event = (id: string, name: string, x: number, y: number, extra: Partial<MapEvent> = {}): MapEvent => ({
  id,
  name,
  x,
  y,
  description: `About ${name}`,
  user_name: 'Host',
  total_attendees: 4,
  start_at: '2026-11-10T10:00:00.000Z',
  ...extra
})

const PLAZA = place('plaza', 'Genesis Plaza', ['0,0'])
const CLUB = place('club', 'Sugar Club', ['-20,60'])

interface Services {
  /** Places by the parcel they cover. */
  places?: MapPlace[]
  search?: MapPlace[]
  total?: number
  events?: MapEvent[]
  photos?: number
  /** Requests the service refuses. */
  fail?: MapApiRequest['op'][]
}

/** Answer the map's signed requests from fixed data, and record them. */
function serve(services: Services = {}): MapApiRequest[] {
  const seen: MapApiRequest[] = []
  const answer = (request: MapApiRequest): unknown => {
    if (services.fail?.includes(request.op)) throw new Error('HTTP 500')
    switch (request.op) {
      case 'places':
        return { ok: true, data: (services.places ?? []).filter((p) => p.positions.some((pos) => request.positions.includes(pos))) }
      case 'destinations':
        return { ok: true, data: services.search ?? [], total: services.total ?? services.search?.length ?? 0 }
      case 'events': {
        const live = request.query.includes('list=live')
        return { ok: true, data: (services.events ?? []).filter((e) => !live || e.live === true) }
      }
      case 'placeLike':
        return { ok: true, data: { likes: 4, dislikes: 1, user_like: request.like === true, user_dislike: request.like === false } }
      case 'placeFavorite':
        return { ok: true, data: { favorites: 1, user_favorite: request.favorite } }
      case 'eventAttend':
        return { ok: true, data: [] }
      case 'placePhotos':
        return { images: Array.from({ length: services.photos ?? 0 }, (_, i) => ({ id: `photo-${i}`, url: `https://photos.test/${i}.jpg` })), maxImages: services.photos ?? 0 }
    }
  }
  setMapApiTransport((id, request) => {
    seen.push(request)
    queueMicrotask(() => {
      try {
        resolveMapApi({ kind: 'mapApiResult', id, ok: true, data: answer(request) })
      } catch (e) {
        resolveMapApi({ kind: 'mapApiResult', id, ok: false, error: String(e) })
      }
    })
  })
  return seen
}

function renderMap(over: Partial<MapState> = {}, genesis = true): MapState {
  const session = fakeSession()
  const map: MapState = { ...session.map, open: true, teleportToPlace: vi.fn(), changeRealm: vi.fn(), toggle: vi.fn(), ...over }
  const minimap = genesis ? session.minimap : { ...session.minimap, satelliteView: null }
  render(<MapPage map={map} minimap={minimap} profile={fakeProfileState()} gallery={session.gallery} onNavigate={vi.fn()} />)
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
  setMapApiTransport(null)
  fixMapPublicReads(null)
  for (const layer of ['liveEvents', 'pois', 'pins', 'people'] as const) mapLayers.set(layer, true)
  mapLayers.setMapType('satellite')
  homeStore.setTransport(null)
  homeStore.receive(NO_HOME)
  navigation.stop()
  scenePins.receive([])
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
    for (const name of ['Live Events', 'Points of interest', 'Quest Mini-Games', 'People']) {
      expect(within(layers).getByRole('switch', { name })).toBeChecked()
    }
    expect(within(layers).getByRole('radio', { name: 'Satellite' })).toBeChecked()
    await userEvent.click(within(layers).getByRole('radio', { name: 'Parcels' }))
    expect(within(layers).getByRole('radio', { name: 'Parcels' })).toBeChecked()
    expect(within(layers).getByRole('radio', { name: 'Satellite' })).not.toBeChecked()
    expect(view().querySelector('img[src*="/v1/map.png"]')).not.toBeNull()
  })

  it('draws the pins scenes put on the map, and hides them with their layer', async () => {
    serve({ places: [PLAZA] })
    scenePins.receive([{ id: 'scene:1', x: 0, y: 0, title: 'Treasure Hunt', description: '', icon: null }])
    renderMap()
    await userEvent.click(marker('Treasure Hunt'))
    expect(await within(panel()).findByRole('heading', { name: 'Genesis Plaza' })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: 'Layers' }))
    await userEvent.click(screen.getByRole('switch', { name: 'Quest Mini-Games' }))
    expect(within(view()).queryByRole('button', { name: 'Treasure Hunt' })).not.toBeInTheDocument()
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
    setMapApiTransport((id, request) => {
      if (request.op === 'destinations') answer = () => resolveMapApi({ kind: 'mapApiResult', id, ok: true, data: { ok: true, data: [PLAZA], total: 1 } })
      else queueMicrotask(() => resolveMapApi({ kind: 'mapApiResult', id, ok: true, data: { ok: true, data: [] } }))
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
    await waitFor(() => expect(seen.filter((r) => r.op === 'destinations').at(-1)).toMatchObject({ query: expect.stringContaining('order_by=created_at') }))
    expect(within(panel()).getByRole('button', { name: 'Newest' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('pages through more than fifty results', async () => {
    const seen = serve({ search: [PLAZA], total: 120 })
    renderMap()
    await search('plaza')
    await within(panel()).findByRole('list', { name: 'Results' })
    expect(within(panel()).getByRole('button', { name: 'Prev' })).toBeDisabled()
    await userEvent.click(within(panel()).getByRole('button', { name: 'Next' }))
    await waitFor(() => expect(seen.filter((r) => r.op === 'destinations').at(-1)).toMatchObject({ query: expect.stringContaining('offset=50') }))
  })

  it('searches a category from its chip and shows its name in the read-only field', async () => {
    const seen = serve({ search: [CLUB] })
    renderMap()
    await userEvent.click(screen.getByRole('button', { name: 'Music' }))
    expect(screen.getByRole('button', { name: 'Music' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(seen.find((r) => r.op === 'destinations')).toMatchObject({ query: expect.stringContaining('categories=music') }))
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
    expect(seen).toContainEqual({ op: 'placeLike', id: 'club', like: true })

    await userEvent.click(like)
    expect(seen).toContainEqual({ op: 'placeLike', id: 'club', like: null })
  })

  it('puts a like back when the service refuses it', async () => {
    serve({ search: [CLUB], fail: ['placeLike'] })
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
    expect(seen).toContainEqual({ op: 'placeFavorite', id: 'club', favorite: true })

    serve({ search: [CLUB], fail: ['placeFavorite'] })
    await userEvent.click(favourite)
    await waitFor(() => expect(favourite).toHaveAttribute('aria-pressed', 'true'))
  })

  it('makes a place home, marks it on the map, and takes it back', async () => {
    serve({ search: [CLUB], places: [CLUB] })
    const sent: unknown[] = []
    homeStore.setTransport((change) => sent.push(change))
    renderMap()
    await search('club')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Sugar Club/ }))
    const home = await within(panel()).findByRole('button', { name: 'Set as Home' })
    expect(within(view()).queryByRole('button', { name: 'Home' })).not.toBeInTheDocument()
    await userEvent.click(home)
    expect(home).toHaveAttribute('aria-pressed', 'true')
    expect(sent).toEqual([{ parcel: { x: -20, y: 60 } }])
    expect(marker('Home')).toBeInTheDocument()

    await userEvent.click(home)
    expect(sent.at(-1)).toBeNull()
    expect(within(view()).queryByRole('button', { name: 'Home' })).not.toBeInTheDocument()
  })

  it('makes a world home by its name, and jumps in to it by its realm', async () => {
    const world = place('w', 'Mansion', ['0,0'], { world_name: 'mansion.dcl.eth' })
    serve({ search: [world] })
    const sent: unknown[] = []
    homeStore.setTransport((change) => sent.push(change))
    const map = renderMap()
    await search('mansion')
    await userEvent.click(await within(panel()).findByRole('button', { name: /Mansion/ }))
    await within(panel()).findByRole('heading', { name: 'Mansion' })
    expect(within(panel()).queryByRole('button', { name: 'start navigation' })).not.toBeInTheDocument()
    expect(within(panel()).getByText('mansion.dcl.eth')).toBeInTheDocument()
    await userEvent.click(within(panel()).getByRole('button', { name: 'Set as Home' }))
    expect(sent).toEqual([{ world: 'mansion.dcl.eth' }])
    await userEvent.click(within(panel()).getByRole('button', { name: 'JUMP IN' }))
    expect(vi.mocked(map.changeRealm)).toHaveBeenCalledWith('mansion.dcl.eth')
  })

  it('opens the home place from the home control', async () => {
    serve({ places: [CLUB] })
    homeStore.receive({ parcel: { x: -20, y: 60 }, world: null })
    renderMap()
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
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Copy Link' }))
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
    expect(seen).toContainEqual({ op: 'eventAttend', id: 'later', attending: true })
    const links = within(p).getAllByRole('link', { name: 'add to calendar' })
    expect(links).toHaveLength(2)
    expect(links[0]).toHaveAttribute('href', expect.stringContaining('dates=20300101T100000Z/20300101T110000Z'))

    await userEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect(await within(panel()).findByRole('heading', { name: 'Sugar Club' })).toBeInTheDocument()
  })

  it('puts the interest back when the service refuses it', async () => {
    serve({ search: [CLUB], events: [event('later', 'Trivia Night', -20, 60)], fail: ['eventAttend'] })
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

import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import { DEFAULT_REALM } from '@hud-host'
import { setPlaceFavorite, setPlaceLike } from '@explorer-core/features/map/mapApi'
import { navigation } from '@explorer-core/features/map/navigation'
import { openPlaceDetail, type DetailPlace } from '../../features/places/PlaceDetail'
import { SessionProvider } from '../../features/session/SessionContext'
import type { EngineSession } from '../../features/session/useEngineSession'
import { fakeSession } from '../harness'
import { eventAt } from './eventFixtures'

vi.mock('@explorer-core/features/map/mapApi', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@explorer-core/features/map/mapApi')>()),
  setPlaceLike: vi.fn(),
  setPlaceFavorite: vi.fn()
}))

const PLAZA: DetailPlace = {
  id: 'plaza',
  title: 'Genesis Plaza',
  description: '',
  image: 'https://example.com/plaza.png',
  positions: ['0,0', '0,1', '1,0'],
  base_position: '0,0',
  owner: '0x1234567890abcdef1234567890abcdef12345678',
  categories: ['social', 'poi', 'music'],
  favorites: 1520,
  user_count: 4,
  live: true,
  like_rate: 0.876,
  user_visits: 25300,
  updated_at: '2026-03-07T10:00:00Z'
}
const WORLD: DetailPlace = { ...PLAZA, id: 'w', title: 'Party World', world: true, world_name: 'party.dcl.eth', live: false }

function show(session: EngineSession, place: DetailPlace, onJumpIn = vi.fn(() => true)): (next: EngineSession) => void {
  const tree = (s: EngineSession) => (
    <SessionProvider value={s}>
      <PopupHost />
      <ToastHost />
    </SessionProvider>
  )
  const { rerender } = render(tree(session))
  act(() => openPlaceDetail({ place, onJumpIn }))
  return (next) => rerender(tree(next))
}

afterEach(() => {
  resetPopups()
  navigation.stop()
  vi.clearAllMocks()
})

describe('place detail', () => {
  it("shows the place with its fallbacks, its online friends, the event live there and only the categories it knows", async () => {
    const session = fakeSession()
    const store = session.events.data.store
    session.events.data.store = {
      ...store,
      byId: { l: eventAt('l', new Date(), { name: 'Sunset Set', live: true, place_id: 'plaza' }) },
      queries: { ...store.queries, lobby: { status: 'loaded', ids: ['l'] } }
    }
    session.friends.list = [
      { address: '0xAAA', name: 'Ana', status: 'online' },
      { address: '0xbbb', name: 'Bo', status: 'offline' }
    ] as EngineSession['friends']['list']
    vi.mocked(session.places.readSigned).mockResolvedValue({ id: 'plaza', connected_addresses: ['0xaaa', '0xbbb', '0xccc'] })
    show(session, PLAZA)
    const detail = within(screen.getByRole('dialog', { name: 'Genesis Plaza' }))
    expect(detail.getByText('0x123...45678')).toBeInTheDocument()
    expect(detail.getByText('No description.')).toBeInTheDocument()
    expect(detail.getByText('88%')).toBeInTheDocument()
    expect(detail.getByText('25.3K')).toBeInTheDocument()
    expect(detail.getByText('1.5K')).toBeInTheDocument()
    expect(detail.getByText('07/03/2026')).toBeInTheDocument()
    expect(detail.getByText('3')).toBeInTheDocument()
    expect(await detail.findByLabelText('3 online')).toBeInTheDocument()
    expect(detail.getByLabelText('1 friend here')).toBeInTheDocument()
    expect(detail.getByRole('tooltip')).toHaveTextContent('Sunset Set')
    expect(detail.getByText('SOCIAL')).toBeInTheDocument()
    expect(detail.getByText('MUSIC')).toBeInTheDocument()
    expect(detail.queryByText('POI')).toBeNull()
  })

  it('takes the signed read of the votes, and puts a refused like back', async () => {
    const session = fakeSession()
    vi.mocked(session.places.readSigned).mockResolvedValue({ id: 'plaza', user_like: false, user_dislike: true, user_favorite: false })
    vi.mocked(setPlaceLike).mockRejectedValue(new Error('refused'))
    show(session, PLAZA)
    const dislike = await screen.findByRole('button', { name: 'Dislike', pressed: true })
    fireEvent.click(screen.getByRole('button', { name: 'Like' }))
    expect(setPlaceLike).toHaveBeenCalledWith('plaza', true)
    expect(screen.getByRole('button', { name: 'Like' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(dislike).toHaveAttribute('aria-pressed', 'true'))
    expect(screen.getByRole('button', { name: 'Like' })).toHaveAttribute('aria-pressed', 'false')
    expect(await screen.findByText('There was an error liking the place. Please try again.')).toBeInTheDocument()
  })

  it('keeps a favourite the service confirms and drops one it refuses', async () => {
    vi.mocked(setPlaceFavorite).mockResolvedValueOnce({ favorites: 1521, user_favorite: true }).mockRejectedValueOnce(new Error('refused'))
    show(fakeSession(), PLAZA)
    const favorite = screen.getByRole('button', { name: 'Favorite' })
    fireEvent.click(favorite)
    await waitFor(() => expect(favorite).toHaveAttribute('aria-pressed', 'true'))
    await waitFor(() => expect(favorite).toBeEnabled())
    fireEvent.click(favorite)
    await waitFor(() => expect(favorite).toHaveAttribute('aria-pressed', 'true'))
    expect(await screen.findByText('There was an error setting the place as favorite. Please try again.')).toBeInTheDocument()
  })

  it('jumps in through the opener and closes, and stays open when the opener cannot travel', () => {
    const onJumpIn = vi.fn(() => false)
    show(fakeSession(), PLAZA, onJumpIn)
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Genesis Plaza' }))
    expect(onJumpIn).toHaveBeenCalledWith(PLAZA)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    onJumpIn.mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Genesis Plaza' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it("makes the place home by its base parcel", () => {
    const session = fakeSession()
    const update = show(session, PLAZA)
    const home = screen.getByRole('button', { name: 'Set as Home' })
    expect(home).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(home)
    expect(session.map.setHome).toHaveBeenCalledWith(DEFAULT_REALM, { x: 0, y: 0 })
    update({ ...session, homeScene: () => ({ realm: null, parcel: '0,0' }) })
    expect(screen.getByRole('button', { name: 'Set as Home' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('shows home pressed when any parcel of the place is home, known before the engine answers, and sets nothing then', () => {
    const session = fakeSession()
    session.homeScene = () => ({ realm: null, parcel: '1,0' })
    show(session, PLAZA)
    const home = screen.getByRole('button', { name: 'Set as Home' })
    expect(home).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(home)
    expect(session.map.setHome).not.toHaveBeenCalled()
  })

  it('makes a world home by its name, and offers it no navigation', () => {
    const session = fakeSession()
    show(session, WORLD)
    fireEvent.click(screen.getByRole('button', { name: 'Set as Home' }))
    expect(session.map.setHome).toHaveBeenCalledWith('party.dcl.eth', { x: 0, y: 0 })
    expect(screen.queryByRole('button', { name: /navigation/i })).toBeNull()
  })

  it('starts a route to the place and exits it', () => {
    show(fakeSession(), PLAZA)
    fireEvent.click(screen.getByRole('button', { name: /start navigation/i }))
    expect(navigation.getSnapshot()?.parcel).toEqual({ x: 0, y: 0 })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /exit navigation/i }))
    expect(navigation.getSnapshot()).toBeNull()
    expect(screen.getByRole('button', { name: /start navigation/i })).toBeInTheDocument()
  })

  it('ends the route to the place when a jump to it goes ahead, and keeps it otherwise', () => {
    const onJumpIn = vi.fn(() => false)
    show(fakeSession(), PLAZA, onJumpIn)
    fireEvent.click(screen.getByRole('button', { name: /start navigation/i }))
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Genesis Plaza' }))
    expect(navigation.getSnapshot()).not.toBeNull()
    onJumpIn.mockReturnValue(true)
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Genesis Plaza' }))
    expect(navigation.getSnapshot()).toBeNull()
  })

  it('keeps a route to another place when jumping in here', () => {
    act(() => navigation.start({ ...PLAZA, id: 'far', positions: ['40,40'], base_position: '40,40' }))
    show(fakeSession(), PLAZA)
    expect(screen.getByRole('button', { name: /start navigation/i })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Genesis Plaza' }))
    expect(navigation.getSnapshot()?.parcel).toEqual({ x: 40, y: 40 })
  })

  it('keeps a vote sent before the signed read lands', async () => {
    const session = fakeSession()
    let read: (place: { id: string; user_like: boolean; user_dislike: boolean; user_favorite: boolean }) => void = () => undefined
    vi.mocked(session.places.readSigned).mockReturnValue(new Promise((resolve) => (read = resolve)))
    vi.mocked(setPlaceLike).mockResolvedValue({ likes: 1, dislikes: 0, user_like: true, user_dislike: false })
    show(session, PLAZA)
    const like = screen.getByRole('button', { name: 'Like' })
    fireEvent.click(like)
    await waitFor(() => expect(like).toBeEnabled())
    await act(async () => read({ id: 'plaza', user_like: false, user_dislike: false, user_favorite: false }))
    expect(like).toHaveAttribute('aria-pressed', 'true')
  })

  it('takes the signed read for the fields not written, and a refused vote goes back to the read', async () => {
    const session = fakeSession()
    let read: (place: { id: string; user_like: boolean; user_dislike: boolean; user_favorite: boolean }) => void = () => undefined
    vi.mocked(session.places.readSigned).mockReturnValue(new Promise((resolve) => (read = resolve)))
    let refuse: (e: Error) => void = () => undefined
    vi.mocked(setPlaceFavorite).mockResolvedValue({ favorites: 1521, user_favorite: true })
    vi.mocked(setPlaceLike).mockReturnValue(new Promise((_, reject) => (refuse = reject)))
    show(session, PLAZA)
    const like = screen.getByRole('button', { name: 'Like' })
    const favorite = screen.getByRole('button', { name: 'Favorite' })
    fireEvent.click(favorite)
    await waitFor(() => expect(favorite).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'Dislike' }))
    await act(async () => read({ id: 'plaza', user_like: true, user_dislike: false, user_favorite: false }))
    expect(favorite).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Dislike' })).toHaveAttribute('aria-pressed', 'true')
    await act(async () => refuse(new Error('refused')))
    expect(like).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Dislike' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('ignores the signed read once a write answered, even while a later write is out or after it is refused', async () => {
    const session = fakeSession()
    let read: (place: { id: string; user_like: boolean; user_dislike: boolean; user_favorite: boolean }) => void = () => undefined
    vi.mocked(session.places.readSigned).mockReturnValue(new Promise((resolve) => (read = resolve)))
    let refuse: (e: Error) => void = () => undefined
    vi.mocked(setPlaceLike)
      .mockResolvedValueOnce({ likes: 1, dislikes: 0, user_like: true, user_dislike: false })
      .mockReturnValueOnce(new Promise((_, reject) => (refuse = reject)))
    show(session, PLAZA)
    const like = screen.getByRole('button', { name: 'Like' })
    fireEvent.click(like)
    await waitFor(() => expect(like).toBeEnabled())
    fireEvent.click(like)
    await act(async () => read({ id: 'plaza', user_like: false, user_dislike: false, user_favorite: false }))
    await act(async () => refuse(new Error('refused')))
    expect(like).toHaveAttribute('aria-pressed', 'true')
  })

  it('shows the like rate and favourites the write returns', async () => {
    vi.mocked(setPlaceLike).mockResolvedValue({ likes: 1, dislikes: 3, user_like: true, user_dislike: false })
    vi.mocked(setPlaceFavorite).mockResolvedValue({ favorites: 7, user_favorite: true })
    show(fakeSession(), PLAZA)
    expect(screen.getByLabelText('Like rate')).toHaveTextContent('88%')
    fireEvent.click(screen.getByRole('button', { name: 'Like' }))
    await waitFor(() => expect(screen.getByLabelText('Like rate')).toHaveTextContent('25%'))
    fireEvent.click(screen.getByRole('button', { name: 'Favorite' }))
    expect(await screen.findByText('7')).toBeInTheDocument()
  })

  it('keeps a favourite in flight when a vote is sent', async () => {
    vi.mocked(setPlaceFavorite).mockReturnValue(new Promise(() => undefined))
    vi.mocked(setPlaceLike).mockReturnValue(new Promise(() => undefined))
    show(fakeSession(), PLAZA)
    fireEvent.click(screen.getByRole('button', { name: 'Favorite' }))
    fireEvent.click(screen.getByRole('button', { name: 'Like' }))
    expect(screen.getByRole('button', { name: 'Favorite' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: 'Like' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('holds Jump in while a sign-in finishes', () => {
    const session = fakeSession()
    session.login.busy = true
    const onJumpIn = vi.fn(() => true)
    show(session, PLAZA, onJumpIn)
    const jump = screen.getByRole('button', { name: 'Jump in to Genesis Plaza' })
    expect(jump).toBeDisabled()
    fireEvent.click(jump)
    expect(onJumpIn).not.toHaveBeenCalled()
  })

  it('keeps Jump in when the engine cannot tell, and asks again for another wallet', async () => {
    const session = fakeSession()
    vi.mocked(session.places.worldAccess).mockResolvedValueOnce('failed').mockResolvedValueOnce('denied')
    const view = (s: EngineSession): React.JSX.Element => (
      <SessionProvider value={s}>
        <PopupHost />
      </SessionProvider>
    )
    const { rerender } = render(view(session))
    act(() => openPlaceDetail({ place: WORLD, onJumpIn: () => true }))
    expect(await screen.findByRole('button', { name: 'Jump in to Party World' })).toBeInTheDocument()
    const other = { ...session, profile: { ...session.profile, data: { ...session.profile.data, address: '0xother' } } } as EngineSession
    rerender(view(other))
    expect(await screen.findByText('INVITE ONLY')).toBeInTheDocument()
    expect(session.places.worldAccess).toHaveBeenCalledTimes(2)
    expect(screen.queryByRole('button', { name: 'Jump in to Party World' })).toBeNull()
  })

  it("holds a world's Jump in until its access is known, then follows it", async () => {
    let answer: (access: 'denied') => void = () => undefined
    const session = fakeSession()
    vi.mocked(session.places.worldAccess).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    show(session, WORLD)
    expect(screen.queryByRole('button', { name: /start navigation/i })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Jump in to Party World' })).toBeNull()
    expect(vi.mocked(session.places.worldAccess).mock.calls[0][0]).toBe('party.dcl.eth')
    await act(async () => answer('denied'))
    expect(screen.getByText('INVITE ONLY')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Jump in to Party World' })).toBeNull()
  })

  it('offers the password for a protected world, and Jump in to an invited one', async () => {
    const session = fakeSession()
    vi.mocked(session.places.worldAccess).mockResolvedValueOnce('password')
    show(session, WORLD)
    expect(await screen.findByText('PASSWORD REQUIRED')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'ENTER PASSWORD' })).toBeInTheDocument()
    act(() => resetPopups())
    vi.mocked(session.places.worldAccess).mockResolvedValueOnce('invited')
    act(() => openPlaceDetail({ place: WORLD, onJumpIn: () => true }))
    expect(await screen.findByText('YOU ARE INVITED')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Jump in to Party World' })).toBeInTheDocument()
  })
})

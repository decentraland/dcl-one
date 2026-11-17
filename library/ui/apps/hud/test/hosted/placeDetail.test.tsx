import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import { setPlaceFavorite, setPlaceLike } from '@explorer-core/features/map/mapApi'
import { fetchWorldAccess } from '@explorer-core/features/places/worldAccess'
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
vi.mock('@explorer-core/features/places/worldAccess', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@explorer-core/features/places/worldAccess')>()),
  fetchWorldAccess: vi.fn(async () => 'allowed')
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

function show(session: EngineSession, place: DetailPlace, onJumpIn = vi.fn(() => true)): void {
  render(
    <SessionProvider value={session}>
      <PopupHost />
      <ToastHost />
    </SessionProvider>
  )
  act(() => openPlaceDetail({ place, onJumpIn }))
}

afterEach(() => {
  resetPopups()
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

  it('leaves out Set as home and Start navigation until they can act', () => {
    show(fakeSession(), PLAZA)
    expect(screen.queryByRole('button', { name: 'Set as home' })).toBeNull()
    expect(screen.queryByRole('button', { name: /start navigation/i })).toBeNull()
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

  it("judges a world's access again once the player's communities arrive", async () => {
    const session = fakeSession()
    session.lobbyData.communitiesLoaded = false
    vi.mocked(fetchWorldAccess).mockResolvedValueOnce('failed').mockResolvedValueOnce('invited')
    const view = (s: EngineSession): React.JSX.Element => (
      <SessionProvider value={s}>
        <PopupHost />
      </SessionProvider>
    )
    const { rerender } = render(view(session))
    act(() => openPlaceDetail({ place: WORLD, onJumpIn: () => true }))
    await waitFor(() => expect(fetchWorldAccess).toHaveBeenCalledTimes(1))
    expect(vi.mocked(fetchWorldAccess).mock.calls[0][2]).toBeNull()
    expect(screen.queryByText('INVITE ONLY')).toBeNull()
    const loaded = { ...session, lobbyData: { ...session.lobbyData, communitiesLoaded: true }, communities: { ...session.communities, list: [{ id: 'c1' }] as EngineSession['communities']['list'] } }
    rerender(view(loaded))
    await waitFor(() => expect(fetchWorldAccess).toHaveBeenCalledTimes(2))
    expect(vi.mocked(fetchWorldAccess).mock.calls[1][2]).toEqual(['c1'])
    expect(await screen.findByText('YOU ARE INVITED')).toBeInTheDocument()
  })

  it("holds a world's Jump in until its access is known, then follows it", async () => {
    let answer: (access: 'denied') => void = () => undefined
    vi.mocked(fetchWorldAccess).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    show(fakeSession(), WORLD)
    expect(screen.queryByRole('button', { name: /start navigation/i })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Jump in to Party World' })).toBeNull()
    expect(vi.mocked(fetchWorldAccess).mock.calls[0][0]).toBe('party.dcl.eth')
    await act(async () => answer('denied'))
    expect(screen.getByText('INVITE ONLY')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Jump in to Party World' })).toBeNull()
  })

  it('offers the password for a protected world, and Jump in to an invited one', async () => {
    vi.mocked(fetchWorldAccess).mockResolvedValueOnce('password')
    show(fakeSession(), WORLD)
    expect(await screen.findByText('PASSWORD REQUIRED')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'ENTER PASSWORD' })).toBeInTheDocument()
    act(() => resetPopups())
    vi.mocked(fetchWorldAccess).mockResolvedValueOnce('invited')
    act(() => openPlaceDetail({ place: WORLD, onJumpIn: () => true }))
    expect(await screen.findByText('YOU ARE INVITED')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Jump in to Party World' })).toBeInTheDocument()
  })
})

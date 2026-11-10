import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, act, fireEvent, cleanup } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CommunitiesPage } from '../../features/communities/CommunitiesPage'
import { CommunityModal, openCommunityModal } from '../../features/communities/CommunityModal'
import { openCommunityCreateModal, openCommunityEditModal, readCommunityPicture } from '../../features/communities/CommunityCreateModal'
import { SessionProvider } from '../../features/session/SessionContext'
import { PopupHost, ToastHost, closeTopPopup, resetPopups } from '../../design'
import type { Community, CommunityDetailMessage, CommunityPlaceOption, CommunityPlaceOptionsMessage } from '../../engine/protocol'
import type { CommunitiesState, CommunitySave } from '../../features/session/useEngineSession'
import { fakeProfileState, fakeSession } from '../harness'
import type { CardAction } from '../../features/chat/ProfileCardPresentation'

afterEach(resetPopups) // the community modal now lives on the module-level popup stack

const community = (over: Partial<Community>): Community => ({
  id: 'c1',
  name: 'Builders',
  description: 'we build',
  membersCount: 12,
  role: 'none',
  ownerName: 'Owner',
  privacy: 'public',
  ...over
})

describe('communities page clicks', () => {
  // The community detail modal opens as a popup and reads live data via useSession(), so the page and
  // <PopupHost/> must share one SessionProvider (its `communities` is what the popup content reads).
  // `mine` is My Communities; `items` the browse grid's pages.
  function renderPage(items: Community[], opts: { mine?: Community[]; browse?: Partial<CommunitiesState['browse']> } = {}): CommunitiesState {
    const session = fakeSession()
    session.communities = {
      ...session.communities,
      open: true,
      list: opts.mine ?? [],
      browse: { search: '', items, total: items.length, loading: false, loadingMore: false, ...opts.browse },
      join: vi.fn(),
      requestToJoin: vi.fn(),
      cancelRequest: vi.fn(),
      loadDetail: vi.fn(),
      loadBrowse: vi.fn(),
      loadMoreBrowse: vi.fn()
    }
    render(
      <SessionProvider value={session}>
        <CommunitiesPage communities={session.communities} profile={fakeProfileState()} onNavigate={vi.fn()} />
        <PopupHost />
      </SessionProvider>
    )
    return session.communities
  }

  it('opening the page reads the first page of every community', () => {
    const communities = renderPage([])
    expect(vi.mocked(communities.loadBrowse)).toHaveBeenCalledWith('')
  })

  it('Join on a public browse card joins that community', async () => {
    const communities = renderPage([community({ id: 'c1', role: 'none' })])
    expect(screen.getByText(/Public · 12 Members/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Join' }))
    expect(vi.mocked(communities.join)).toHaveBeenCalledWith('c1')
  })

  it('a private card asks to join, and cancels a request in flight', async () => {
    const communities = renderPage([
      community({ id: 'p1', name: 'Locked', privacy: 'private' }),
      community({ id: 'p2', name: 'Asked', privacy: 'private', pendingRequestId: 'r2' })
    ])
    expect(screen.getAllByText(/Private · 12 Members/)).toHaveLength(2)
    expect(screen.queryByRole('button', { name: 'Join' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Request to Join' }))
    expect(vi.mocked(communities.requestToJoin)).toHaveBeenCalledWith('p1')
    await userEvent.click(screen.getByRole('button', { name: 'Cancel join request' }))
    expect(vi.mocked(communities.cancelRequest)).toHaveBeenCalledWith('p2', 'r2')
    // neither click opened the card
    expect(vi.mocked(communities.loadDetail)).not.toHaveBeenCalled()
  })

  it("shows the user's friends in the community and its description", () => {
    const friends = [
      { address: '0xa', name: 'Mojito', hasClaimedName: true },
      { address: '0xb', name: 'Kimbo', hasClaimedName: false }
    ]
    renderPage([community({ id: 'c1', friends })])
    expect(screen.getByLabelText('Friends in this community: Mojito, Kimbo')).toBeInTheDocument()
    expect(screen.getByRole('tooltip', { name: 'Mojito' })).toBeInTheDocument()
    expect(screen.getByText('we build')).toBeInTheDocument()
  })

  it('View on a joined community opens its modal and loads detail', async () => {
    const communities = renderPage([community({ id: 'c1', role: 'member' })])
    await userEvent.click(screen.getByRole('button', { name: 'View' }))
    expect(vi.mocked(communities.loadDetail)).toHaveBeenCalledWith('c1')
    expect(screen.getByRole('heading', { name: 'Builders' })).toBeInTheDocument()
  })

  // My Communities is its own list (every membership, an unlisted one too), apart from the grid
  it('My Communities lists every membership, an unlisted one included', async () => {
    const hidden = community({ id: 'c-unlisted', name: 'Secret Club', role: 'owner', privacy: 'private', visibility: 'unlisted' })
    const communities = renderPage([community({ id: 'c1', role: 'none' })], { mine: [hidden] })
    expect(screen.queryByText("You haven't joined any communities yet.")).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: /Secret Club/ }))
    expect(vi.mocked(communities.loadDetail)).toHaveBeenCalledWith('c-unlisted')
    expect(screen.getByRole('heading', { name: 'Secret Club' })).toBeInTheDocument()
  })

  it('VIEW ALL shows the communities the user is a member of', async () => {
    renderPage([community({ id: 'c1', role: 'none' })], { mine: [community({ id: 'c-unlisted', name: 'Secret Club', role: 'owner' })] })
    expect(screen.getByText('Browse Communities')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: /VIEW ALL/ }))
    expect(screen.getByText('My Communities', { selector: 'span:not([class*="sideHead"])' })).toBeInTheDocument()
    expect(screen.getByText('(1)')).toBeInTheDocument()
  })

  it("titles the grid with the service's total", () => {
    renderPage([community({ id: 'c1' })], { browse: { total: 57 } })
    expect(screen.getByText('Browse Communities')).toBeInTheDocument()
    expect(screen.getByText('(57)')).toBeInTheDocument()
  })

  it('searches a second after typing stops, at once on Enter, and lists everything when cleared', () => {
    vi.useFakeTimers()
    try {
      const communities = renderPage([])
      const loadBrowse = vi.mocked(communities.loadBrowse)
      loadBrowse.mockClear()
      const field = screen.getByRole('textbox', { name: 'Search' })
      fireEvent.change(field, { target: { value: 'pi' } })
      fireEvent.change(field, { target: { value: 'pix' } })
      act(() => vi.advanceTimersByTime(999))
      expect(loadBrowse).not.toHaveBeenCalled()
      act(() => vi.advanceTimersByTime(1))
      expect(loadBrowse).toHaveBeenCalledTimes(1)
      expect(loadBrowse).toHaveBeenCalledWith('pix')
      fireEvent.change(field, { target: { value: 'pixel' } })
      fireEvent.keyDown(field, { key: 'Enter' })
      expect(loadBrowse).toHaveBeenLastCalledWith('pixel')
      // the delayed search for the same text was cancelled
      act(() => vi.advanceTimersByTime(2000))
      expect(loadBrowse).toHaveBeenCalledTimes(2)
      fireEvent.click(screen.getByRole('button', { name: 'Clear search' }))
      expect(loadBrowse).toHaveBeenLastCalledWith('')
    } finally {
      vi.useRealTimers()
    }
  })

  it('the same search again reads nothing', () => {
    const communities = renderPage([], { browse: { search: 'pix' } })
    const loadBrowse = vi.mocked(communities.loadBrowse)
    loadBrowse.mockClear()
    const field = screen.getByRole('textbox', { name: 'Search' })
    fireEvent.change(field, { target: { value: 'pix' } })
    fireEvent.keyDown(field, { key: 'Enter' })
    expect(loadBrowse).not.toHaveBeenCalled()
  })

  it("titles search results, says when nothing matches, and goes back to every community", async () => {
    const communities = renderPage([], { browse: { search: 'zzzz' } })
    expect(screen.getByText("Results for 'zzzz'")).toBeInTheDocument()
    expect(screen.getByText('(0)')).toBeInTheDocument()
    expect(screen.getByText('No Communities match your search terms.')).toBeInTheDocument()
    vi.mocked(communities.loadBrowse).mockClear()
    await userEvent.click(screen.getByRole('button', { name: 'Back to Browse Communities' }))
    expect(vi.mocked(communities.loadBrowse)).toHaveBeenCalledWith('')
  })

  it('a failed first page shows no count and no empty state (the toast says it failed)', () => {
    renderPage([], { browse: { failed: true, total: 0 } })
    expect(screen.getByText('Browse Communities')).toBeInTheDocument()
    expect(screen.queryByText('(0)')).toBeNull()
    expect(screen.queryByText('No Communities match your search terms.')).toBeNull()
  })

  it('shows a spinner and no count while the first page is read', () => {
    renderPage([], { browse: { loading: true, total: 0 } })
    expect(screen.queryByText('(0)')).toBeNull()
    expect(screen.queryByText('No Communities match your search terms.')).toBeNull()
  })

  it("reads the next page when the grid's end comes into view, while there is one", () => {
    const observed: Array<(entries: Array<{ isIntersecting: boolean }>) => void> = []
    class FakeObserver {
      constructor(cb: (entries: Array<{ isIntersecting: boolean }>) => void) {
        observed.push(cb)
      }
      observe(): void {}
      disconnect(): void {}
    }
    vi.stubGlobal('IntersectionObserver', FakeObserver)
    try {
      const communities = renderPage([community({ id: 'c1' })], { browse: { total: 30 } })
      act(() => observed.at(-1)?.([{ isIntersecting: true }]))
      expect(vi.mocked(communities.loadMoreBrowse)).toHaveBeenCalledTimes(1)
      cleanup()
      observed.length = 0
      renderPage([community({ id: 'c1' })], { browse: { total: 1 } })
      expect(observed).toHaveLength(0)
    } finally {
      vi.unstubAllGlobals()
    }
  })
})

describe('community modal clicks', () => {
  const detail = (over: Partial<CommunityDetailMessage> = {}): CommunityDetailMessage => ({
    kind: 'communityDetail',
    id: 'c1',
    members: [],
    posts: [],
    places: [],
    events: [],
    photos: [],
    ...over
  })

  function renderModal(c: Community, d: CommunityDetailMessage | null = detail()) {
    const spies = { onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() }
    render(<CommunityModal community={c} detail={d} {...spies} />)
    return spies
  }

  it('Join (non-member) joins', async () => {
    const s = renderModal(community({ role: 'none', privacy: 'public' }))
    await userEvent.click(screen.getByRole('button', { name: 'Join' }))
    expect(s.onJoin).toHaveBeenCalledWith('c1')
  })

  it('member: open chat + leave via the more menu', async () => {
    const s = renderModal(community({ role: 'member' }))
    await userEvent.click(screen.getByRole('button', { name: 'Open chat' }))
    expect(s.onOpenChat).toHaveBeenCalledTimes(1)
    await userEvent.click(screen.getByRole('button', { name: 'More' }))
    await userEvent.click(screen.getByRole('button', { name: 'Leave community' }))
    expect(s.onLeave).toHaveBeenCalledWith('c1')
    expect(s.onClose).toHaveBeenCalledTimes(1)
  })

  it('close button closes the modal', async () => {
    const s = renderModal(community({}))
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(s.onClose).toHaveBeenCalledTimes(1)
  })
})

// The card's Places tab, as unity-explorer's: owners and moderators get an ADD PLACE tile that opens
// the Edit Community form, and can delete, after a confirmation, the linked places they own.
describe('community card places', () => {
  const mine = { id: 'pl-mine', title: 'My Plaza', positions: '1,2', isMine: true }
  const theirs = { id: 'pl-theirs', title: 'Their Plaza', positions: '3,4', isMine: false }
  const withPlaces = (places: CommunityDetailMessage['places']): CommunityDetailMessage => ({
    kind: 'communityDetail',
    id: 'c1',
    members: [],
    posts: [],
    places,
    events: [],
    photos: []
  })
  function renderCard(role: string, places: CommunityDetailMessage['places']) {
    const spies = { onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn(), onEdit: vi.fn(), onRemovePlace: vi.fn() }
    render(
      <>
        <PopupHost />
        <CommunityModal community={community({ id: 'c1', name: 'Builders', role })} detail={withPlaces(places)} {...spies} />
      </>
    )
    return spies
  }
  const openPlaces = async (): Promise<void> => userEvent.click(screen.getByRole('tab', { name: 'PLACES' }))

  it('puts ADD PLACE first for owners and moderators, opening the Edit Community form', async () => {
    for (const role of ['owner', 'moderator']) {
      const s = renderCard(role, [theirs])
      await openPlaces()
      const tiles = screen.getAllByRole('button', { name: /ADD PLACE|Delete/ })
      expect(tiles[0]).toHaveTextContent('ADD PLACE')
      await userEvent.click(screen.getByRole('button', { name: /ADD PLACE/ }))
      expect(s.onEdit).toHaveBeenCalledTimes(1)
      cleanup()
    }
  })

  it('shows members no ADD PLACE and no delete', async () => {
    renderCard('member', [mine])
    await openPlaces()
    expect(screen.queryByRole('button', { name: /ADD PLACE/ })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Delete My Plaza' })).toBeNull()
  })

  it('shows an owner of a community without places only the ADD PLACE tile, and a member the empty text', async () => {
    renderCard('owner', [])
    await openPlaces()
    expect(screen.getByRole('button', { name: /ADD PLACE/ })).toBeInTheDocument()
    expect(screen.queryByText('No places shared yet.')).toBeNull()
    cleanup()
    renderCard('member', [])
    await openPlaces()
    expect(screen.getByText('No places shared yet.')).toBeInTheDocument()
  })

  it('offers delete only on the places the user owns', async () => {
    renderCard('owner', [mine, theirs])
    await openPlaces()
    expect(screen.getByRole('button', { name: 'Delete My Plaza' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete Their Plaza' })).toBeNull()
  })

  it("asks with unity-explorer's texts, and deletes only on DELETE", async () => {
    const s = renderCard('moderator', [mine])
    await openPlaces()
    await userEvent.click(screen.getByRole('button', { name: 'Delete My Plaza' }))
    expect(screen.getByText("Are you sure you want to delete [My Plaza] from [Builders]'s Places?")).toBeInTheDocument()
    expect(screen.getByText("Photos and events associated with this Place won't appear on the Community's profile anymore.")).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    expect(s.onRemovePlace).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'Delete My Plaza' }))
    await userEvent.click(screen.getByRole('button', { name: 'DELETE' }))
    expect(s.onRemovePlace).toHaveBeenCalledWith('c1', 'pl-mine')
  })

  it('holds the delete while it is on its way, so it cannot be sent twice', async () => {
    renderCard('owner', [mine])
    await openPlaces()
    await userEvent.click(screen.getByRole('button', { name: 'Delete My Plaza' }))
    await userEvent.click(screen.getByRole('button', { name: 'DELETE' }))
    expect(screen.getByRole('button', { name: 'Delete My Plaza' })).toBeDisabled()
  })
})

// The create form, as unity-explorer's: it holds typed details (the scrim refuses, the deliberate
// closes ask), needs a name and a description, waits for the content check, and shows how it ended.
/** The form's places as the session holds them once read: none owned, none linked. */
const noPlaces = (over: Partial<CommunityPlaceOptionsMessage> = {}): CommunityPlaceOptionsMessage => ({
  kind: 'communityPlaceOptions',
  owned: [],
  linked: [],
  hiddenLinkedIds: [],
  linkedFailed: false,
  errors: [],
  ...over
})
const land: CommunityPlaceOption = { id: 'land-1', isWorld: false, label: 'Plaza (12,-34)', ownerName: 'Me' }
const world: CommunityPlaceOption = { id: 'world-1', isWorld: true, label: 'Mansion (me.dcl.eth)', ownerName: 'Me' }
const othersLand: CommunityPlaceOption = { id: 'land-9', isWorld: false, label: 'Their Plaza (1,1)', ownerName: 'Owner' }

describe('community create modal', () => {
  function renderCreate(initialPlaces: CommunityPlaceOptionsMessage | null = noPlaces()): {
    communities: CommunitiesState
    setSave: (save: CommunitySave | null) => void
    setPlaces: (places: CommunityPlaceOptionsMessage | null) => void
    onCreated: ReturnType<typeof vi.fn>
  } {
    const session = fakeSession()
    const onCreated = vi.fn()
    let places = initialPlaces
    let saved: CommunitySave | null = null
    const view = (save: CommunitySave | null): React.JSX.Element => {
      saved = save
      session.communities = { ...session.communities, save, placeOptions: places }
      return (
        <SessionProvider value={{ ...session }}>
          <PopupHost />
          <ToastHost />
        </SessionProvider>
      )
    }
    const { rerender } = render(view(null))
    act(() => {
      openCommunityCreateModal(true, onCreated)
    })
    return {
      communities: session.communities,
      setSave: (save) => rerender(view(save)),
      setPlaces: (next) => {
        places = next
        rerender(view(saved))
      },
      onCreated
    }
  }
  const fill = async (name = 'Builders', description = 'We build things'): Promise<void> => {
    await userEvent.type(screen.getByLabelText(/community name/i), name)
    await userEvent.type(screen.getByLabelText(/description/i), description)
  }

  it('ignores a backdrop click once something has been entered', async () => {
    renderCreate()
    await fill()
    fireEvent.click(document.querySelector('[class*="backdrop"]') as HTMLElement)
    expect(screen.queryByText('Discard this community?')).toBeNull()
    expect((screen.getByLabelText(/community name/i) as HTMLInputElement).value).toBe('Builders')
  })

  it('asks on CANCEL, and keeps the details when the answer is no', async () => {
    renderCreate()
    await fill()
    await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    await userEvent.click(screen.getByRole('button', { name: 'Keep editing' }))
    expect((screen.getByLabelText(/community name/i) as HTMLInputElement).value).toBe('Builders')
  })

  it('needs a description as well as a name, as the service does', async () => {
    renderCreate()
    await userEvent.type(screen.getByLabelText(/community name/i), 'Builders')
    expect(screen.getByRole('button', { name: 'CREATE' })).toBeDisabled()
    await userEvent.type(screen.getByLabelText(/description/i), '   ')
    expect(screen.getByRole('button', { name: 'CREATE' })).toBeDisabled()
    await userEvent.type(screen.getByLabelText(/description/i), 'We build things')
    expect(screen.getByRole('button', { name: 'CREATE' })).toBeEnabled()
  })

  it('sends membership and discoverability picked from our own listboxes', async () => {
    const { communities } = renderCreate()
    await fill()

    // A native <select> would open a Chromium popup, which the offscreen HUD cannot composite.
    expect(document.querySelector('select')).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Membership' }))
    await userEvent.click(screen.getByRole('option', { name: /^Private: Membership by request\/invite/ }))
    await userEvent.click(screen.getByRole('button', { name: 'Discoverability' }))
    await userEvent.click(screen.getByRole('option', { name: /^Unlisted: Hidden from non-members/ }))

    await userEvent.click(screen.getByRole('button', { name: 'CREATE' }))
    expect(communities.create).toHaveBeenCalledWith({
      name: 'Builders',
      description: 'We build things',
      privacy: 'private',
      discoverable: false,
      placeIds: []
    })
  })

  it('shows the counters only while their field has focus', async () => {
    renderCreate()
    expect(screen.queryByText('0/30')).toBeNull()
    await userEvent.type(screen.getByLabelText(/community name/i), 'Bui')
    expect(screen.getByText('3/30')).toBeInTheDocument()
    await userEvent.click(screen.getByLabelText(/description/i))
    expect(screen.queryByText('3/30')).toBeNull()
    expect(screen.getByText('0/500')).toBeInTheDocument()
  })

  it('waits for the content check, then opens the new community without asking', async () => {
    const { setSave, onCreated } = renderCreate()
    await fill()
    await userEvent.click(screen.getByRole('button', { name: 'CREATE' }))
    act(() => setSave({ status: 'saving' }))
    expect(screen.getByText('Running Content Policy check. This may take a moment.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'CREATE' })).toBeDisabled()

    act(() => setSave({ status: 'done', result: { kind: 'communitySaveResult', mode: 'create', id: 'c-new' } }))
    expect(onCreated).toHaveBeenCalledWith('c-new')
    expect(screen.queryByText('Discard this community?')).toBeNull()
    expect(screen.queryByRole('dialog', { name: 'Create a Community' })).toBeNull()
  })

  it('keeps the form and says so when the create fails', async () => {
    const { setSave, onCreated } = renderCreate()
    await fill()
    act(() => setSave({ status: 'done', result: { kind: 'communitySaveResult', mode: 'create', error: 'HTTP 500: boom' } }))
    expect(await screen.findByText('There was an error creating community. Please try again.')).toBeInTheDocument()
    expect(onCreated).not.toHaveBeenCalled()
    expect((screen.getByLabelText(/community name/i) as HTMLInputElement).value).toBe('Builders')
  })

  it("shows the content check's findings, and EDIT COMMUNITY goes back to the form", async () => {
    const { communities, setSave } = renderCreate()
    await fill()
    act(() =>
      setSave({
        status: 'done',
        result: { kind: 'communitySaveResult', mode: 'create', moderation: { issues: { name: ['Hate speech', 'Slur'], description: [], image: ['Violence'] }, unavailable: false } }
      })
    )
    expect(screen.getByRole('heading', { name: "Your Community Couldn't Be Created" })).toBeInTheDocument()
    expect(screen.getByText('COMMUNITY NAME: [Hate speech], [Slur]')).toBeInTheDocument()
    expect(screen.getByText('COMMUNITY IMAGE: [Violence]')).toBeInTheDocument()
    expect(screen.queryByText(/COMMUNITY DESCRIPTION/)).toBeNull()

    await userEvent.click(screen.getByRole('button', { name: 'EDIT COMMUNITY' }))
    expect(communities.clearSave).toHaveBeenCalled()
    act(() => setSave(null))
    expect((screen.getByLabelText(/community name/i) as HTMLInputElement).value).toBe('Builders')
  })

  it('offers RETRY when the content check could not run', async () => {
    const { communities, setSave } = renderCreate()
    await fill()
    act(() =>
      setSave({
        status: 'done',
        result: { kind: 'communitySaveResult', mode: 'create', moderation: { issues: { name: [], description: [], image: [] }, unavailable: true } }
      })
    )
    expect(screen.getByText('Content verification is temporarily unavailable. Please try again later.')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'RETRY' }))
    expect(communities.create).toHaveBeenCalledWith(expect.objectContaining({ name: 'Builders', description: 'We build things' }))
  })

  it('reads no places behind the NAME gate', () => {
    const session = fakeSession()
    render(
      <SessionProvider value={{ ...session }}>
        <PopupHost />
      </SessionProvider>
    )
    act(() => {
      openCommunityCreateModal(false, vi.fn())
    })
    expect(screen.getByRole('dialog', { name: 'Get a NAME' })).toBeInTheDocument()
    expect(session.communities.loadPlaceOptions).not.toHaveBeenCalled()
  })

  it('reads the places as it opens, and waits for them before it can be filled', () => {
    const { communities, setPlaces } = renderCreate(null)
    expect(communities.loadPlaceOptions).toHaveBeenCalledWith(undefined)
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByLabelText(/community name/i)).toBeNull()
    expect(screen.getByRole('button', { name: 'CREATE' })).toBeDisabled()
    act(() => setPlaces(noPlaces({ owned: [land] })))
    expect(screen.getByLabelText(/community name/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add a place' })).toHaveTextContent('Select LAND or World')
  })

  it("shows unity-explorer's message for each places read that failed", async () => {
    renderCreate(noPlaces({ errors: ['There was an error getting places. Please try again.'] }))
    expect(await screen.findByText('There was an error getting places. Please try again.')).toBeInTheDocument()
  })

  it('links places from the picker, hides the ones linked, and sends lands before worlds', async () => {
    const { communities } = renderCreate(noPlaces({ owned: [world, land] }))
    await fill()
    await userEvent.click(screen.getByRole('button', { name: 'Add a place' }))
    await userEvent.click(screen.getByRole('option', { name: world.label }))
    await userEvent.click(screen.getByRole('button', { name: 'Add a place' }))
    expect(screen.queryByRole('option', { name: world.label })).toBeNull()
    await userEvent.click(screen.getByRole('option', { name: land.label }))
    const tags = screen.getByRole('list', { name: 'Linked places' })
    expect(tags).toHaveTextContent(world.label)
    expect(tags).toHaveTextContent('Me')

    // removing one offers it again
    await userEvent.click(screen.getByRole('button', { name: `Remove ${land.label}` }))
    await userEvent.click(screen.getByRole('button', { name: 'Add a place' }))
    await userEvent.click(screen.getByRole('option', { name: land.label }))

    await userEvent.click(screen.getByRole('button', { name: 'CREATE' }))
    expect(communities.create).toHaveBeenCalledWith(expect.objectContaining({ placeIds: ['land-1', 'world-1'] }))
  })

  it('closes untouched, with nothing to lose', () => {
    renderCreate()
    act(() => closeTopPopup())
    expect(screen.queryByRole('button', { name: 'CANCEL' })).toBeNull()
  })
})

// Edit Community, as unity-explorer's: owners and moderators open it from the card, it loads the
// community, only the owner changes name, membership and discoverability, and only changes are sent.
describe('community edit', () => {
  const owned = community({ id: 'c9', name: 'Builders', description: 'We build', role: 'owner', privacy: 'private', visibility: 'unlisted', thumbnail: 'https://cdn/c9.png' })
  function renderEdit(
    c: Community,
    places: CommunityPlaceOptionsMessage | null = noPlaces({ communityId: c.id })
  ): { communities: CommunitiesState; setSave: (save: CommunitySave | null) => void } {
    const session = fakeSession()
    const view = (save: CommunitySave | null): React.JSX.Element => {
      session.communities = { ...session.communities, save, placeOptions: places }
      return (
        <SessionProvider value={{ ...session }}>
          <PopupHost />
          <ToastHost />
        </SessionProvider>
      )
    }
    const { rerender } = render(view(null))
    act(() => {
      openCommunityEditModal(c)
    })
    return { communities: session.communities, setSave: (save) => rerender(view(save)) }
  }

  it('offers Edit Community on the card to owners and moderators only', () => {
    const spies = { onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() }
    const onEdit = vi.fn()
    const { rerender } = render(<CommunityModal community={community({ role: 'moderator' })} detail={null} onEdit={onEdit} {...spies} />)
    expect(screen.getByRole('button', { name: 'Edit Community' })).toBeInTheDocument()
    rerender(<CommunityModal community={community({ role: 'member' })} detail={null} onEdit={onEdit} {...spies} />)
    expect(screen.queryByRole('button', { name: 'Edit Community' })).toBeNull()
  })

  it('opens with the community as it is, titled Edit Community, with SAVE', () => {
    renderEdit(owned)
    expect(screen.getByRole('heading', { name: 'Edit Community' })).toBeInTheDocument()
    expect((screen.getByLabelText(/community name/i) as HTMLInputElement).value).toBe('Builders')
    expect((screen.getByLabelText(/description/i) as HTMLTextAreaElement).value).toBe('We build')
    expect(screen.getByRole('button', { name: 'Membership' })).toHaveTextContent(/^Private/)
    expect(screen.getByRole('button', { name: 'Discoverability' })).toHaveTextContent(/^Unlisted/)
    expect(screen.getByRole('button', { name: 'SAVE' })).toBeEnabled()
  })

  it('lets a moderator change the description but not the name, membership or discoverability', () => {
    renderEdit({ ...owned, role: 'moderator' })
    expect(screen.getByLabelText(/community name/i)).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Membership' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Discoverability' })).toBeDisabled()
    expect(screen.getByLabelText(/description/i)).toBeEnabled()
  })

  it('sends only what changed', async () => {
    const { communities } = renderEdit(owned)
    await userEvent.clear(screen.getByLabelText(/description/i))
    await userEvent.type(screen.getByLabelText(/description/i), 'We build worlds')
    await userEvent.click(screen.getByRole('button', { name: 'Discoverability' }))
    await userEvent.click(screen.getByRole('option', { name: /^Discoverable/ }))
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).toHaveBeenCalledWith({ id: 'c9', description: 'We build worlds', discoverable: true })
  })

  it('closes without saving when nothing changed', async () => {
    const { communities } = renderEdit(owned)
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog', { name: 'Edit Community' })).toBeNull()
    expect(screen.queryByText('Discard your changes?')).toBeNull()
  })

  it('reads the community\'s places as it opens, and shows the linked ones', () => {
    const { communities } = renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land], linked: [othersLand] }))
    expect(communities.loadPlaceOptions).toHaveBeenCalledWith('c9')
    expect(screen.getByRole('list', { name: 'Linked places' })).toHaveTextContent(othersLand.label)
  })

  it('sends the whole new set of places when it changed, and leaves it out otherwise', async () => {
    const { communities } = renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land, world], linked: [world] }))
    await userEvent.type(screen.getByLabelText(/description/i), '!')
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).toHaveBeenLastCalledWith({ id: 'c9', description: 'We build!' })
  })

  it('unlinks a place and saves the rest', async () => {
    const { communities } = renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land, world], linked: [world, land] }))
    await userEvent.click(screen.getByRole('button', { name: `Remove ${world.label}` }))
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).toHaveBeenCalledWith({ id: 'c9', placeIds: ['land-1'] })
  })

  it('keeps the linked places it could not show when the places change', async () => {
    const { communities } = renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land, world], linked: [world, land], hiddenLinkedIds: ['pl-disabled'] }))
    await userEvent.click(screen.getByRole('button', { name: `Remove ${world.label}` }))
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).toHaveBeenCalledWith({ id: 'c9', placeIds: ['land-1', 'pl-disabled'] })
  })

  it('lets the owner unlink any place', () => {
    renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land], linked: [land, othersLand] }))
    expect(screen.getByRole('button', { name: `Remove ${land.label}` })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: `Remove ${othersLand.label}` })).toBeInTheDocument()
  })

  it('lets a moderator unlink only the places they own', () => {
    renderEdit({ ...owned, role: 'moderator' }, noPlaces({ communityId: 'c9', owned: [land], linked: [land, othersLand] }))
    expect(screen.getByRole('button', { name: `Remove ${land.label}` })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: `Remove ${othersLand.label}` })).toBeNull()
    expect(screen.getByRole('list', { name: 'Linked places' })).toHaveTextContent(othersLand.label)
  })

  it('cannot change the places when the linked ones could not be read', async () => {
    const { communities } = renderEdit(owned, noPlaces({ communityId: 'c9', owned: [land], linkedFailed: true, errors: ['There was an error getting the community places. Please try again.'] }))
    expect(await screen.findByText('There was an error getting the community places. Please try again.')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Add a place' })).toBeDisabled()
    await userEvent.type(screen.getByLabelText(/description/i), '!')
    await userEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(communities.update).toHaveBeenCalledWith({ id: 'c9', description: 'We build!' })
  })

  it('closes once saved, and keeps the form with a toast when the update fails', async () => {
    const { setSave } = renderEdit(owned)
    await userEvent.type(screen.getByLabelText(/description/i), '!')
    act(() => setSave({ status: 'done', result: { kind: 'communitySaveResult', mode: 'update', error: 'HTTP 500: boom' } }))
    expect(await screen.findByText('There was an error updating community. Please try again.')).toBeInTheDocument()
    expect(screen.getByRole('dialog', { name: 'Edit Community' })).toBeInTheDocument()
    act(() => setSave({ status: 'done', result: { kind: 'communitySaveResult', mode: 'update', id: 'c9' } }))
    expect(screen.queryByRole('dialog', { name: 'Edit Community' })).toBeNull()
  })
})

// Delete Community, as unity-explorer's: the owner's ⋮ menu, asked first, and the card closes once
// the community is gone.
describe('community delete', () => {
  const spies = () => ({ onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() })

  it('is in the owner\'s menu only', async () => {
    const onDelete = vi.fn()
    const { rerender } = render(<CommunityModal community={community({ role: 'moderator' })} detail={null} onDelete={onDelete} {...spies()} />)
    await userEvent.click(screen.getByRole('button', { name: 'More' }))
    expect(screen.queryByRole('button', { name: 'Delete Community' })).toBeNull()
    rerender(<CommunityModal community={community({ role: 'owner' })} detail={null} onDelete={onDelete} {...spies()} />)
    expect(screen.getByRole('button', { name: 'Delete Community' })).toBeInTheDocument()
  })

  it('does not offer the owner Leave community', async () => {
    render(<CommunityModal community={community({ role: 'owner' })} detail={null} {...spies()} />)
    await userEvent.click(screen.getByRole('button', { name: 'More' }))
    expect(screen.queryByRole('button', { name: 'Leave community' })).toBeNull()
  })

  it('asks first, and deletes only on CONTINUE', async () => {
    const onDelete = vi.fn()
    render(
      <>
        <CommunityModal community={community({ id: 'c7', name: 'Builders', role: 'owner' })} detail={null} onDelete={onDelete} {...spies()} />
        <PopupHost />
      </>
    )
    await userEvent.click(screen.getByRole('button', { name: 'More' }))
    await userEvent.click(screen.getByRole('button', { name: 'Delete Community' }))
    expect(screen.getByText('Are you sure you want to delete [Builders] Community?')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    expect(onDelete).not.toHaveBeenCalled()

    await userEvent.click(screen.getByRole('button', { name: 'More' }))
    await userEvent.click(screen.getByRole('button', { name: 'Delete Community' }))
    await userEvent.click(screen.getByRole('button', { name: 'CONTINUE' }))
    expect(onDelete).toHaveBeenCalledWith('c7')
  })

  it('closes the card once the community is deleted', () => {
    const session = fakeSession()
    const owned = community({ id: 'c7', role: 'owner' })
    const view = (deleted: string | null): React.JSX.Element => {
      session.communities = { ...session.communities, list: [owned], deleted, loadDetail: vi.fn() }
      return (
        <SessionProvider value={{ ...session }}>
          <PopupHost />
        </SessionProvider>
      )
    }
    const { rerender } = render(view(null))
    const onClosed = vi.fn()
    act(() => {
      openCommunityModal('c7', onClosed)
    })
    expect(screen.getByRole('heading', { name: 'Builders' })).toBeInTheDocument()
    rerender(view('c7'))
    expect(screen.queryByRole('heading', { name: 'Builders' })).toBeNull()
    expect(onClosed).toHaveBeenCalledTimes(1)
  })
})

// unity-explorer's checks on a picked picture, with its three messages.
describe('community picture checks', () => {
  const file = (bytes: number, type = 'image/png'): File => new File([new Uint8Array(bytes)], 'pic', { type })
  const stubImage = (w: number, h: number): void => {
    // jsdom has no object URLs: the check reads the dimensions from one
    URL.createObjectURL = vi.fn(() => 'blob:pic')
    URL.revokeObjectURL = vi.fn()
    vi.stubGlobal(
      'Image',
      class {
        onload: (() => void) | null = null
        naturalWidth = w
        naturalHeight = h
        set src(_: string) {
          setTimeout(() => this.onload?.())
        }
      }
    )
  }
  afterEach(() => vi.unstubAllGlobals())

  it('takes a PNG or JPEG within 500 KB and 512×512 px', async () => {
    stubImage(512, 512)
    expect(await readCommunityPicture(file(512000))).toEqual({ dataUrl: expect.stringMatching(/^data:image\/png;base64,/) })
    expect(await readCommunityPicture(file(1000, 'image/jpeg'))).toEqual({ dataUrl: expect.stringMatching(/^data:image\/jpeg;base64,/) })
  })

  it('refuses other types, and says which limit a picture breaks', async () => {
    stubImage(600, 400)
    expect(await readCommunityPicture(file(1000, 'image/gif'))).toEqual({ error: 'Invalid image file selected. Please check file type and size.' })
    expect(await readCommunityPicture(file(1000))).toEqual({ error: 'Selected image exceeds 512x512 px size limit.' })
    expect(await readCommunityPicture(file(512001))).toEqual({ error: 'Invalid image file selected. Please check file type and size.' })
    stubImage(100, 100)
    expect(await readCommunityPicture(file(512001))).toEqual({ error: 'Selected image exceeds 500KB size limit.' })
  })

  it('refuses a large picture without reading it as text, and frees its object URL', async () => {
    stubImage(4000, 3000)
    const read = vi.spyOn(FileReader.prototype, 'readAsDataURL')
    expect(await readCommunityPicture(file(5_000_000, 'image/jpeg'))).toEqual({ error: 'Invalid image file selected. Please check file type and size.' })
    expect(read).not.toHaveBeenCalled()
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:pic')
    read.mockRestore()
  })
})

// The card's Members tab, as unity-explorer's member list: the row opens the passport, carries the
// friendship button for the relationship, and a menu with View Profile, Chat, the owner's and
// moderators' actions, Block and Report.
describe('community card members', () => {
  const owner = { address: '0xowner', name: 'Boss', role: 'owner', hasClaimedName: true }
  const mod = { address: '0xmod', name: 'Mod', role: 'moderator', hasClaimedName: true }
  const plain = { address: '0xabcd1234', name: 'Plain', role: 'member', hasClaimedName: false }
  const me = { address: '0xme', name: 'Me', role: 'member', hasClaimedName: true }
  const members = [owner, mod, plain, me]
  const withMembers: CommunityDetailMessage = { kind: 'communityDetail', id: 'c1', members, posts: [], places: [], events: [], photos: [] }

  function renderCard(role: string, over: Partial<React.ComponentProps<typeof CommunityModal>> = {}) {
    const spies = {
      onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn(),
      onFriend: vi.fn(), onViewProfile: vi.fn(), onMemberMenu: vi.fn(), onMemberAction: vi.fn()
    }
    const card = (props: Partial<React.ComponentProps<typeof CommunityModal>>): React.JSX.Element => (
      <>
        <PopupHost />
        <CommunityModal community={community({ id: 'c1', name: 'Builders', role })} detail={withMembers} me={{ address: '0xME', name: 'Me' }} {...spies} {...props} />
      </>
    )
    const { rerender } = render(card(over))
    return { ...spies, rerender: (props: Partial<React.ComponentProps<typeof CommunityModal>>) => rerender(card({ ...over, ...props })) }
  }
  const openMembers = async (): Promise<void> => userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
  /** The member menu's rows as the card's opener got them. */
  const menuFor = async (s: ReturnType<typeof renderCard>, name: string): Promise<{ labels: string[]; actions: { label: string; onClick: () => void; disabledReason?: string }[] }> => {
    await userEvent.click(screen.getByRole('button', { name: `More options for ${name}` }))
    const actions: CardAction[] = vi.mocked(s.onMemberMenu).mock.calls.at(-1)?.[2] ?? []
    return { labels: actions.map((a) => a.label), actions }
  }

  it("opens a member's passport from the row, and shows the #tag of an unclaimed name", async () => {
    const s = renderCard('member')
    await openMembers()
    expect(screen.getByText('#1234')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: "View Plain's profile" }))
    expect(s.onViewProfile).toHaveBeenCalledWith('0xabcd1234')
  })

  it("gives the user's own row no friendship button and no menu", async () => {
    renderCard('member')
    await openMembers()
    expect(screen.queryByRole('button', { name: 'More options for Me' })).toBeNull()
    expect(screen.getAllByRole('button', { name: /ADD FRIEND/ })).toHaveLength(3)
  })

  it.each([
    ['none', /ADD FRIEND/, 'add'],
    ['incoming', /ACCEPT FRIEND/, 'accept'],
    ['requested', /Cancel Request/, 'cancel'],
    ['blocked', /Unblock/, 'unblock']
  ] as const)('a %s member gets the friendship button that runs %s', async (rel, name, action) => {
    const s = renderCard('member', { relationship: (a: string) => (a === plain.address ? rel : 'friend') })
    await openMembers()
    await userEvent.click(screen.getByRole('button', { name }))
    expect(s.onFriend).toHaveBeenCalledWith(plain, action)
  })

  it('gives a friend no friendship button', async () => {
    renderCard('member', { relationship: () => 'friend' })
    await openMembers()
    expect(screen.queryByRole('button', { name: /ADD FRIEND|ACCEPT FRIEND|Cancel Request|Unblock/ })).toBeNull()
  })

  it('a plain member gets no owner or moderator actions', async () => {
    const s = renderCard('member')
    await openMembers()
    expect((await menuFor(s, 'Mod')).labels).toEqual([])
  })

  it("the owner can demote a moderator, hand over, remove or ban; promote a member", async () => {
    const s = renderCard('owner')
    await openMembers()
    expect((await menuFor(s, 'Mod')).labels).toEqual(['Demote Moderator', 'Make Community Owner', 'Remove Member', 'Ban'])
    expect((await menuFor(s, 'Plain')).labels).toEqual(['Promote Moderator', 'Make Community Owner', 'Remove Member', 'Ban'])
  })

  it('a moderator can remove or ban plain members only', async () => {
    const s = renderCard('moderator')
    await openMembers()
    expect((await menuFor(s, 'Plain')).labels).toEqual(['Remove Member', 'Ban'])
    expect((await menuFor(s, 'Mod')).labels).toEqual([])
    expect((await menuFor(s, 'Boss')).labels).toEqual([])
  })

  it('greys out handing over to a member without a NAME, with unity-explorer\'s reason', async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Plain')
    expect(actions.find((a) => a.label === 'Make Community Owner')?.disabledReason).toBe('Community ownership can only be transferred to users who own a NAME.')
    expect(actions.find((a) => a.label === 'Remove Member')?.disabledReason).toBeUndefined()
  })

  it('promotes at once', async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Plain')
    await act(async () => actions.find((a) => a.label === 'Promote Moderator')?.onClick())
    expect(s.onMemberAction).toHaveBeenCalledWith('c1', '0xabcd1234', 'promote')
  })

  it.each([
    ['Remove Member', 'Are you sure you want to remove [Plain] from the [Builders] Community?', 'KICK', 'kick'],
    ['Ban', 'Are you sure you want to ban [Plain] from the [Builders] Community?', 'BAN', 'ban']
  ] as const)('%s asks first, with unity-explorer\'s question', async (label, question, confirm, action) => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Plain')
    act(() => actions.find((a) => a.label === label)?.onClick())
    expect(await screen.findByText(question)).toBeInTheDocument()
    expect(s.onMemberAction).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: confirm }))
    expect(s.onMemberAction).toHaveBeenCalledWith('c1', '0xabcd1234', action)
  })

  it('asks before handing over ownership, and does nothing on CANCEL', async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Mod')
    act(() => actions.find((a) => a.label === 'Make Community Owner')?.onClick())
    expect(await screen.findByText('Transferring Community Ownership to Mod')).toBeInTheDocument()
    expect(screen.getByText(/you will be demoted from Owner to Moderator/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    expect(s.onMemberAction).not.toHaveBeenCalled()
  })

  it('keeps the hold when the new action clears an earlier error', async () => {
    const s = renderCard('owner', { error: 'There was an error kicking the user. Please try again.' })
    await openMembers()
    const { actions } = await menuFor(s, 'Mod')
    await act(async () => actions.find((a) => a.label === 'Demote Moderator')?.onClick())
    s.rerender({ error: null })
    expect(screen.getByRole('button', { name: 'More options for Mod' })).toBeDisabled()
  })

  it("releases a member's menu when the action fails", async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Mod')
    await act(async () => actions.find((a) => a.label === 'Demote Moderator')?.onClick())
    s.rerender({ error: 'There was an error removing moderator from user. Please try again.' })
    expect(screen.getByRole('button', { name: 'More options for Mod' })).toBeEnabled()
  })

  it('does nothing when Remove Member is cancelled', async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Plain')
    act(() => actions.find((a) => a.label === 'Remove Member')?.onClick())
    await userEvent.click(await screen.findByRole('button', { name: 'CANCEL' }))
    expect(s.onMemberAction).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'More options for Plain' })).toBeEnabled()
  })

  it("offers no row controls until the user's own profile is known", async () => {
    renderCard('owner', { me: null })
    await openMembers()
    expect(screen.queryByRole('button', { name: /More options for/ })).toBeNull()
    expect(screen.queryByRole('button', { name: /ADD FRIEND/ })).toBeNull()
  })

  it("holds a member's menu while an action on them runs, until the members come back", async () => {
    const s = renderCard('owner')
    await openMembers()
    const { actions } = await menuFor(s, 'Mod')
    await act(async () => actions.find((a) => a.label === 'Demote Moderator')?.onClick())
    expect(s.onMemberAction).toHaveBeenCalledWith('c1', '0xmod', 'demote')
    expect(screen.getByRole('button', { name: 'More options for Mod' })).toBeDisabled()
    s.rerender({ detail: { ...withMembers, members: [owner, { ...mod, role: 'member' }, plain, me] } })
    expect(screen.getByRole('button', { name: 'More options for Mod' })).toBeEnabled()
  })
})

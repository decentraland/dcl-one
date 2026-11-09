import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, act, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CommunitiesPage } from '../features/communities/CommunitiesPage'
import { CommunityModal, openCommunityModal } from '../features/communities/CommunityModal'
import { openCommunityCreateModal, openCommunityEditModal, readCommunityPicture } from '../features/communities/CommunityCreateModal'
import { SessionProvider } from '../features/session/SessionContext'
import { PopupHost, ToastHost, closeTopPopup, resetPopups } from '../design'
import type { Community, CommunityDetailMessage } from '../engine/protocol'
import type { CommunitiesState, CommunitySave } from '../features/session/useEngineSession'
import { fakeProfileState, fakeSession } from './harness'

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
  function renderPage(list: Community[]): CommunitiesState {
    const session = fakeSession()
    session.communities = { ...session.communities, open: true, list, join: vi.fn(), loadDetail: vi.fn() }
    render(
      <SessionProvider value={session}>
        <CommunitiesPage communities={session.communities} profile={fakeProfileState()} onNavigate={vi.fn()} />
        <PopupHost />
      </SessionProvider>
    )
    return session.communities
  }

  it('Join on a browse card joins that community', async () => {
    const communities = renderPage([community({ id: 'c1', role: 'none' })])
    await userEvent.click(screen.getByRole('button', { name: 'Join' }))
    expect(vi.mocked(communities.join)).toHaveBeenCalledWith('c1')
  })

  it('View on a joined community opens its modal and loads detail', async () => {
    const communities = renderPage([community({ id: 'c1', role: 'member' })])
    await userEvent.click(screen.getByRole('button', { name: 'View' }))
    expect(vi.mocked(communities.loadDetail)).toHaveBeenCalledWith('c1')
    expect(screen.getByRole('heading', { name: 'Builders' })).toBeInTheDocument()
  })

  // the bridge adds the user's communities the browse page left out (unlisted ones too) to the list
  it('My Communities lists every membership in the list, an unlisted one included', async () => {
    const hidden = community({ id: 'c-unlisted', name: 'Secret Club', role: 'owner', privacy: 'private', visibility: 'unlisted' })
    const communities = renderPage([community({ id: 'c1', role: 'none' }), hidden])
    expect(screen.queryByText("You haven't joined any communities yet.")).toBeNull()
    // its row under My Communities, and its card at the end of the Browse grid
    const rows = screen.getAllByRole('button', { name: /Secret Club/ })
    expect(rows).toHaveLength(2)
    await userEvent.click(rows[0])
    expect(vi.mocked(communities.loadDetail)).toHaveBeenCalledWith('c-unlisted')
    expect(screen.getByRole('heading', { name: 'Secret Club' })).toBeInTheDocument()
  })

  it('VIEW ALL shows the communities the user is a member of', async () => {
    renderPage([community({ id: 'c1', role: 'none' }), community({ id: 'c-unlisted', name: 'Secret Club', role: 'owner' })])
    await userEvent.click(screen.getByRole('button', { name: /VIEW ALL/ }))
    expect(screen.getByText('My Communities (1)')).toBeInTheDocument()
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
    const spies = { onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onAddFriend: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() }
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

  it('Members tab: ADD FRIEND sends a request', async () => {
    const s = renderModal(
      community({ role: 'member' }),
      detail({ members: [{ address: '0xm', name: 'M', role: 'member', isFriend: false }] })
    )
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    await userEvent.click(screen.getByRole('button', { name: /ADD FRIEND/i }))
    expect(s.onAddFriend).toHaveBeenCalledWith('0xm')
  })

  it('close button closes the modal', async () => {
    const s = renderModal(community({}))
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(s.onClose).toHaveBeenCalledTimes(1)
  })
})

// The create form, as unity-explorer's: it holds typed details (the scrim refuses, the deliberate
// closes ask), needs a name and a description, waits for the content check, and shows how it ended.
describe('community create modal', () => {
  function renderCreate(): { communities: CommunitiesState; setSave: (save: CommunitySave | null) => void; onCreated: ReturnType<typeof vi.fn> } {
    const session = fakeSession()
    const onCreated = vi.fn()
    const view = (save: CommunitySave | null): React.JSX.Element => {
      session.communities = { ...session.communities, save }
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
    return { communities: session.communities, setSave: (save) => rerender(view(save)), onCreated }
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
      discoverable: false
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
  function renderEdit(c: Community): { communities: CommunitiesState; setSave: (save: CommunitySave | null) => void } {
    const session = fakeSession()
    const view = (save: CommunitySave | null): React.JSX.Element => {
      session.communities = { ...session.communities, save }
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
    const spies = { onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onAddFriend: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() }
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
  const spies = () => ({ onJoin: vi.fn(), onRequestToJoin: vi.fn(), onCancelRequest: vi.fn(), onLeave: vi.fn(), onAddFriend: vi.fn(), onOpenChat: vi.fn(), onClose: vi.fn() })

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
})

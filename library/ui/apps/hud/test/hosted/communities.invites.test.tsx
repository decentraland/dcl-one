// Community invites and join requests: the browser's Invites & Requests section
// and its counter, Accept and Reject on an invited card, the card's Members tab sections (Banned,
// Join Requests, Invited) for owners and moderators, the Invite to Community submenu, and the
// notifications that open a community's card.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CommunitiesPage } from '../../features/communities/CommunitiesPage'
import { CommunityModal, openCommunityModal } from '../../features/communities/CommunityModal'
import { ProfileCard } from '../../features/profileCard/ProfileCard'
import { communityTarget } from '../../features/notifications/summary'
import { SessionProvider } from '../../features/session/SessionContext'
import { INVITE_SENT, INVITES_LOAD_ERROR } from '../../features/session/useEngineSession'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import type { AppNotification, Community, CommunityInvitesMessage, CommunityMemberSectionMessage, CommunityRequestMember } from '@explorer-core/engine/protocol'
import type { CommunitiesState, EngineSession } from '../../features/session/useEngineSession'
import { enterAsGuest, fakeProfileState, fakeSession, renderSession } from '../harness'

afterEach(resetPopups)

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
const person = (n: number, over: Partial<CommunityRequestMember> = {}): CommunityRequestMember => ({
  address: `0x${n.toString(16).padStart(40, '0')}`,
  name: `Person${n}`,
  role: 'none',
  requestId: `rq${n}`,
  ...over
})
const ME = '0x' + 'e'.repeat(40)

function renderPage(over: Partial<CommunitiesState> = {}): CommunitiesState {
  const session = fakeSession()
  session.communities = { ...session.communities, open: true, manageRequest: vi.fn(), loadInvites: vi.fn(), loadInvitesCount: vi.fn(), ...over }
  render(
    <SessionProvider value={session}>
      <CommunitiesPage communities={session.communities} profile={fakeProfileState({ data: { address: ME, name: 'Me', hasClaimedName: true, isGuest: false } as never })} onNavigate={vi.fn()} />
      <PopupHost />
    </SessionProvider>
  )
  return session.communities
}

const invites = (over: Partial<CommunityInvitesMessage> = {}): CommunityInvitesMessage => ({
  kind: 'communityInvites',
  invites: [],
  requestsReceived: [],
  requestsSent: [],
  failed: [],
  ...over
})

describe('the browser’s Invites & Requests', () => {
  it('reads only the counter when the page opens, and shows it', () => {
    const c = renderPage({ pendingCount: 3 })
    expect(vi.mocked(c.loadInvitesCount)).toHaveBeenCalled()
    expect(vi.mocked(c.loadInvites)).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: 'Invites & Requests, 3 pending' })).toHaveTextContent('3')
  })

  it('lists invites, requests received and requests sent, each titled with its count', async () => {
    const c = renderPage({
      list: [community({ id: 'c8', name: 'Mine', role: 'owner' })],
      invites: invites({
        invites: [community({ id: 'c3', name: 'Invited', pendingInviteId: 'i3' })],
        // more requests than were read: the title counts the service's total, as the sidebar does
        requestsReceived: [{ community: community({ id: 'c8', name: 'Mine', role: 'owner' }), requests: [person(1)], total: 140 }],
        requestsSent: [community({ id: 'c2', name: 'Asked', privacy: 'private', pendingRequestId: 'r2' })]
      })
    })
    await userEvent.click(screen.getByRole('button', { name: /Invites & Requests/ }))
    expect(screen.getByRole('heading', { name: 'Invites (1)' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Requests Received (140)' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Requests Sent (1)' })).toBeInTheDocument()
    expect(screen.getByText('140 Requests Received')).toBeInTheDocument()
    // the section is read on opening it
    expect(vi.mocked(c.loadInvites)).toHaveBeenCalledTimes(1)
  })

  it('accepts an invite, and rejects one only after asking', async () => {
    const c = renderPage({ invites: invites({ invites: [community({ id: 'c3', name: 'Invited', pendingInviteId: 'i3' })] }) })
    await userEvent.click(screen.getByRole('button', { name: /Invites & Requests/ }))
    await userEvent.click(screen.getByRole('button', { name: 'Accept invite' }))
    expect(vi.mocked(c.manageRequest)).toHaveBeenCalledWith('c3', 'i3', 'accepted')
    await userEvent.click(screen.getByRole('button', { name: 'Reject invite' }))
    expect(screen.getByText('Are you sure you want to delete your invitation to the [Invited] Community?')).toBeInTheDocument()
    expect(vi.mocked(c.manageRequest)).not.toHaveBeenCalledWith('c3', 'i3', 'rejected')
    await userEvent.click(screen.getByRole('button', { name: 'YES' }))
    expect(vi.mocked(c.manageRequest)).toHaveBeenCalledWith('c3', 'i3', 'rejected')
  })

  it("answers a member's request with the member named", async () => {
    const p = person(1)
    const c = renderPage({
      list: [community({ id: 'c8', role: 'moderator' })],
      invites: invites({ requestsReceived: [{ community: community({ id: 'c8', role: 'moderator' }), requests: [p], total: 1 }] })
    })
    await userEvent.click(screen.getByRole('button', { name: /Invites & Requests/ }))
    await userEvent.click(screen.getByRole('button', { name: 'ACCEPT' }))
    expect(vi.mocked(c.manageRequest)).toHaveBeenCalledWith('c8', 'rq1', 'accepted', p.address)
  })

  it('hides Requests Received from a user who owns or moderates no community', async () => {
    renderPage({ invites: invites({ invites: [community({ id: 'c3', pendingInviteId: 'i3' })] }) })
    await userEvent.click(screen.getByRole('button', { name: /Invites & Requests/ }))
    expect(screen.queryByRole('heading', { name: /Requests Received/ })).toBeNull()
    expect(screen.getByRole('heading', { name: 'Requests Sent (0)' })).toBeInTheDocument()
    expect(screen.getByText('No Requests')).toBeInTheDocument()
  })

  it('says when nothing is pending', async () => {
    renderPage({ invites: invites() })
    await userEvent.click(screen.getByRole('button', { name: /Invites & Requests/ }))
    expect(screen.getByText('No pending invites or requests.')).toBeInTheDocument()
  })

  it('an invited card in the browse grid offers Accept and Reject instead of Join', () => {
    renderPage({ browse: { search: '', items: [community({ id: 'c3', pendingInviteId: 'i3' })], total: 1, loading: false, loadingMore: false } })
    expect(screen.getByRole('button', { name: 'Accept invite' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Join' })).toBeNull()
  })
})

function renderCard(c: Community, sections: Partial<Record<'banned' | 'requests' | 'invites', CommunityMemberSectionMessage>> = {}) {
  const props = { onAcceptInvite: vi.fn(), onRejectInvite: vi.fn(), onLoadSection: vi.fn(), onManageRequest: vi.fn(), onUnban: vi.fn(), onJoin: vi.fn() }
  const detail = { kind: 'communityDetail' as const, id: c.id, members: [], posts: [], places: [], events: [], photos: [] }
  render(
    <>
      <CommunityModal
        community={c}
        detail={detail}
        me={{ address: ME, name: 'Me' }}
        sections={sections}
        {...props}
        onRequestToJoin={vi.fn()}
        onCancelRequest={vi.fn()}
        onLeave={vi.fn()}
        onOpenChat={vi.fn()}
        onClose={vi.fn()}
      />
      <PopupHost />
    </>
  )
  return props
}
const section = (s: 'banned' | 'requests' | 'invites', members: CommunityRequestMember[]): CommunityMemberSectionMessage => ({ kind: 'communityMemberSection', id: 'c8', section: s, members, total: members.length })

describe("the community card's invite and Members sections", () => {
  it('a pending invite shows ACCEPT INVITE and REJECT INVITE instead of Join', async () => {
    const p = renderCard(community({ id: 'c3', name: 'Invited', pendingInviteId: 'i3' }))
    expect(screen.queryByRole('button', { name: 'Join' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'ACCEPT INVITE' }))
    expect(p.onAcceptInvite).toHaveBeenCalledWith('c3', 'i3')
    await userEvent.click(screen.getByRole('button', { name: 'REJECT INVITE' }))
    await userEvent.click(screen.getByRole('button', { name: 'YES' }))
    expect(p.onRejectInvite).toHaveBeenCalledWith('c3', 'i3')
  })

  it('owners of a private community read its join requests on opening, and see the four sections', async () => {
    const p = renderCard(community({ id: 'c8', role: 'owner', privacy: 'private' }), { requests: section('requests', [person(1), person(2)]) })
    expect(p.onLoadSection).toHaveBeenCalledWith('requests')
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    const tabs = within(screen.getByRole('tablist', { name: 'Member sections' })).getAllByRole('tab')
    expect(tabs.map((t) => t.textContent)).toEqual(['MEMBERS', 'BANNED', 'JOIN REQUESTS2', 'INVITED'])
  })

  it('a public community has no Join Requests section; a member sees no sections at all', async () => {
    renderCard(community({ id: 'c8', role: 'moderator' }))
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    expect(screen.queryByRole('tab', { name: /JOIN REQUESTS/ })).toBeNull()
    expect(screen.getByRole('tab', { name: 'INVITED' })).toBeInTheDocument()
  })

  it('accepts and rejects a join request, cancels an invite and unbans, each from its section', async () => {
    const p = renderCard(community({ id: 'c8', role: 'owner', privacy: 'private' }), {
      requests: section('requests', [person(1)]),
      invites: section('invites', [person(2)]),
      banned: section('banned', [person(3, { requestId: '' })])
    })
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    await userEvent.click(screen.getByRole('tab', { name: /JOIN REQUESTS/ }))
    expect(p.onLoadSection).toHaveBeenCalledWith('requests')
    await userEvent.click(screen.getByRole('button', { name: 'ACCEPT' }))
    expect(p.onManageRequest).toHaveBeenCalledWith(expect.objectContaining({ requestId: 'rq1' }), 'accepted')
    await userEvent.click(screen.getByRole('tab', { name: 'INVITED' }))
    await userEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    expect(p.onManageRequest).toHaveBeenCalledWith(expect.objectContaining({ requestId: 'rq2' }), 'cancelled')
    await userEvent.click(screen.getByRole('tab', { name: 'BANNED' }))
    expect(p.onLoadSection).toHaveBeenCalledWith('banned')
    await userEvent.click(screen.getByRole('button', { name: 'UNBAN' }))
    expect(p.onUnban).toHaveBeenCalledWith(expect.objectContaining({ address: person(3).address }))
  })

  it('a section that could not be read says so and reads it again on RETRY', async () => {
    const p = renderCard(community({ id: 'c8', role: 'owner', privacy: 'private' }), { banned: { ...section('banned', []), failed: true } })
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    await userEvent.click(screen.getByRole('tab', { name: 'BANNED' }))
    expect(screen.getByText('There was an error loading this list. Please try again.')).toBeInTheDocument()
    expect(screen.queryByText('No Banned Accounts')).toBeNull()
    p.onLoadSection.mockClear()
    await userEvent.click(screen.getByRole('button', { name: 'RETRY' }))
    expect(p.onLoadSection).toHaveBeenCalledWith('banned')
  })

  it('shows the empty text for each section', async () => {
    renderCard(community({ id: 'c8', role: 'owner', privacy: 'private' }), { banned: section('banned', []) })
    await userEvent.click(screen.getByRole('tab', { name: 'MEMBERS' }))
    await userEvent.click(screen.getByRole('tab', { name: 'BANNED' }))
    expect(screen.getByText('No Banned Accounts')).toBeInTheDocument()
    expect(screen.getByText("Banned accounts won't be able to find your Community.")).toBeInTheDocument()
  })
})

describe('opening a card', () => {
  it('reads the community again even when the grid lists it, for an invite that arrived since', () => {
    const session = fakeSession()
    session.communities = { ...session.communities, browse: { search: '', items: [community({ id: 'c3' })], total: 1, loading: false, loadingMore: false }, fetch: vi.fn() }
    render(
      <SessionProvider value={session}>
        <PopupHost />
      </SessionProvider>
    )
    act(() => {
      openCommunityModal('c3', () => undefined)
    })
    expect(vi.mocked(session.communities.fetch)).toHaveBeenCalledWith('c3')
  })
})

describe('the Invite to Community submenu', () => {
  function renderProfileCard(mutate: (s: EngineSession) => void, opts: { invite?: boolean } = {}): EngineSession {
    const s = fakeSession()
    s.profile = fakeProfileState({ data: { address: ME, name: 'Me', hasClaimedName: true, isGuest: false } as never })
    mutate(s)
    render(
      <SessionProvider value={s}>
        <ProfileCard userId="0xfriend" x={10} y={10} invite={opts.invite} onClose={vi.fn()} />
      </SessionProvider>
    )
    return s
  }

  it("reads the person's invitable communities, and invites them to one", async () => {
    const s = renderProfileCard((s) => {
      s.communities.invitable = { kind: 'invitableCommunities', address: '0xfriend', communities: [{ id: 'c8', name: 'Builders' }] }
    })
    expect(vi.mocked(s.communities.loadInvitable)).toHaveBeenCalledWith('0xfriend')
    await userEvent.click(screen.getByRole('menuitem', { name: /Invite to Community/ }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Builders' }))
    expect(vi.mocked(s.communities.invite)).toHaveBeenCalledWith('c8', '0xfriend')
  })

  it('opens from the keyboard, moves through its communities and closes on Escape', async () => {
    renderProfileCard((s) => {
      s.communities.invitable = { kind: 'invitableCommunities', address: '0xfriend', communities: [{ id: 'c8', name: 'Builders' }, { id: 'c9', name: 'Crafters' }] }
    })
    const row = screen.getByRole('menuitem', { name: /Invite to Community/ })
    row.focus()
    await userEvent.keyboard('{ArrowRight}')
    expect(screen.getByRole('menuitem', { name: 'Builders' })).toHaveFocus()
    await userEvent.keyboard('{ArrowDown}')
    expect(screen.getByRole('menuitem', { name: 'Crafters' })).toHaveFocus()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('menuitem', { name: 'Builders' })).toBeNull()
    expect(row).toHaveFocus()
  })

  it('is hidden when there is no community to invite them to', () => {
    renderProfileCard((s) => {
      s.communities.invitable = { kind: 'invitableCommunities', address: '0xfriend', communities: [] }
    })
    expect(screen.queryByRole('menuitem', { name: /Invite to Community/ })).toBeNull()
  })

  it('reads nothing for a guest, nor from a menu that has no invite (the voice panel)', () => {
    const guest = renderProfileCard((s) => {
      s.profile = fakeProfileState({ data: { address: ME, name: 'Me', hasClaimedName: false, isGuest: true } as never })
    })
    expect(vi.mocked(guest.communities.loadInvitable)).not.toHaveBeenCalled()
    const voice = renderProfileCard(() => undefined, { invite: false })
    expect(vi.mocked(voice.communities.loadInvitable)).not.toHaveBeenCalled()
  })
})

describe('the session’s invites and requests', () => {
  it('sends the reads and answers, and toasts a failed list and a sent invite', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadInvites())
    act(() => h.session().communities.manageRequest('c8', 'rq1', 'accepted', '0xa'))
    act(() => h.session().communities.loadMemberSection('c8', 'banned'))
    expect(h.driver.sentOf('getCommunityInvites')).toHaveLength(1)
    expect(h.driver.sentOf('manageCommunityRequest')).toEqual([{ kind: 'manageCommunityRequest', id: 'c8', requestId: 'rq1', intention: 'accepted', address: '0xa' }])
    expect(h.driver.sentOf('getCommunityMemberSection')).toEqual([{ kind: 'getCommunityMemberSection', id: 'c8', section: 'banned' }])
    act(() => h.driver.emit({ kind: 'communityInvitesCount', count: 4 }))
    expect(h.session().communities.pendingCount).toBe(4)
    act(() => h.driver.emit(invites({ failed: ['invites'] })))
    expect(await screen.findByText(INVITES_LOAD_ERROR.invites)).toBeInTheDocument()
    expect(h.session().communities.invites?.invites).toEqual([])
    // the full read sets the counter too
    expect(h.session().communities.pendingCount).toBe(0)
    act(() => h.driver.emit({ kind: 'communityInviteSent', id: 'c8', address: '0xa', ok: true }))
    expect(await screen.findByText(INVITE_SENT)).toBeInTheDocument()
  })

  it('toasts a failed answer instead of showing it on a card', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c3', action: 'acceptInvite', message: 'HTTP 500' }))
    expect(await screen.findByText('There was an error accepting community invitation. Please try again.')).toBeInTheDocument()
    expect(h.session().communities.error).toBeNull()
  })

  it('sends one answer per request until the section is read again', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.manageRequest('c3', 'i3', 'accepted'))
    act(() => h.session().communities.manageRequest('c3', 'i3', 'accepted'))
    expect(h.driver.sentOf('manageCommunityRequest')).toHaveLength(1)
    act(() => h.driver.emit(invites()))
    act(() => h.session().communities.manageRequest('c3', 'i3', 'rejected'))
    expect(h.driver.sentOf('manageCommunityRequest')).toHaveLength(2)
    // a failure releases it too
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c3', action: 'rejectInvite', message: 'HTTP 500' }))
    act(() => h.session().communities.manageRequest('c3', 'i3', 'rejected'))
    expect(h.driver.sentOf('manageCommunityRequest')).toHaveLength(3)
  })

  it("an accepted request reads that community's open card again", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadDetail('c2'))
    const before = h.driver.sentOf('getCommunity').length
    act(() =>
      h.driver.emit({
        kind: 'notificationsNew',
        notifications: [{ id: 'n-acc', type: 'community_request_to_join_accepted', read: false, timestamp: new Date().toISOString(), metadata: { communityId: 'c2', communityName: 'Chiri' } }]
      })
    )
    expect(h.driver.sentOf('getCommunity').slice(before)).toEqual([{ kind: 'getCommunity', id: 'c2' }])
    expect(h.driver.sentOf('getCommunityDetail').at(-1)).toEqual({ kind: 'getCommunityDetail', id: 'c2' })
  })

  it("keeps only the open card's sections", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadDetail('c8'))
    act(() => h.driver.emit(section('banned', [person(3)])))
    act(() => h.driver.emit({ ...section('requests', [person(1)]), id: 'other' }))
    expect(Object.keys(h.session().communities.memberSections)).toEqual(['banned'])
    act(() => h.session().communities.loadDetail('c9'))
    expect(h.session().communities.memberSections).toEqual({})
  })
})

describe('community notifications', () => {
  const n = (type: string): AppNotification => ({ id: type, type, read: false, timestamp: new Date().toISOString(), metadata: { communityId: 'c8', communityName: 'Builders' } })

  it('an invite, a request received and a request accepted open the community’s card', () => {
    for (const type of ['community_invite_received', 'community_request_to_join_received', 'community_request_to_join_accepted']) {
      expect(communityTarget(n(type))).toBeTypeOf('function')
    }
    expect(communityTarget(n('community_post_added'))).toBeUndefined()
  })
})

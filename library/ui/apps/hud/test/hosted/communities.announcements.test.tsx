// The community card's Announcements tab: pages of 10, posting for owners and moderators, deleting
// (the owner any, an author their own, asked first), liking one request at a time, the author's marks,
// the chat's ":" emoji suggestions, and every failure toasted with its own text.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CommunityModal, openCommunityModal } from '../../features/communities/CommunityModal'
import { SessionProvider } from '../../features/session/SessionContext'
import { NO_ANNOUNCEMENTS, postAge } from '../../features/communities/Announcements'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import type { Community, CommunityPost } from '@explorer-core/engine/protocol'
import type { CommunityPosts } from '../../features/session/useEngineSession'
import { enterAsGuest, fakeSession, renderSession } from '../harness'

afterEach(resetPopups)

const ME = '0x' + 'e'.repeat(40)
const OTHER = '0x' + '7'.repeat(40)
const post = (n: number, over: Partial<CommunityPost> = {}): CommunityPost => ({
  id: `p${n}`,
  author: 'Kimbo',
  authorAddress: OTHER,
  hasClaimedName: true,
  official: false,
  text: `Announcement ${n}`,
  timestamp: Date.now() - 3 * 60 * 60 * 1000,
  likes: 2,
  likedByMe: false,
  ...over
})
const posts = (list: CommunityPost[], over: Partial<CommunityPosts> = {}): CommunityPosts => ({ id: 'c1', posts: list, total: list.length, loading: false, loadingMore: false, posting: false, posted: 0, liking: [], deleting: [], ...over })
const community = (role: string, privacy = 'public'): Community => ({ id: 'c1', name: 'Builders', description: '', membersCount: 3, role, ownerName: 'Owner', privacy })

function renderCard(role: string, state: CommunityPosts | null, privacy = 'public') {
  const props = { onPost: vi.fn(), onDeletePost: vi.fn(), onLikePost: vi.fn(), onLoadMorePosts: vi.fn() }
  const view = render(
    <>
      <CommunityModal
        community={community(role, privacy)}
        detail={{ kind: 'communityDetail', id: 'c1', members: [], places: [], events: [], photos: [] }}
        me={{ address: ME, name: 'Me' }}
        posts={state}
        {...props}
        onJoin={vi.fn()}
        onRequestToJoin={vi.fn()}
        onCancelRequest={vi.fn()}
        onLeave={vi.fn()}
        onOpenChat={vi.fn()}
        onClose={vi.fn()}
      />
      <PopupHost />
    </>
  )
  return { ...props, rerender: (next: CommunityPosts) => view.rerender(
    <>
      <CommunityModal
        community={community(role, privacy)}
        detail={{ kind: 'communityDetail', id: 'c1', members: [], places: [], events: [], photos: [] }}
        me={{ address: ME, name: 'Me' }}
        posts={next}
        {...props}
        onJoin={vi.fn()}
        onRequestToJoin={vi.fn()}
        onCancelRequest={vi.fn()}
        onLeave={vi.fn()}
        onOpenChat={vi.fn()}
        onClose={vi.fn()}
      />
      <PopupHost />
    </>
  ) }
}

describe('announcements session', () => {
  it('reads the first page, then the next one, and stops when all are read', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadPosts('c1'))
    expect(h.driver.last('getCommunityPosts')).toEqual({ kind: 'getCommunityPosts', id: 'c1', page: 1 })
    expect(h.session().communities.posts?.loading).toBe(true)
    act(() => h.driver.emit({ kind: 'communityPosts', id: 'c1', page: 1, posts: Array.from({ length: 10 }, (_, i) => post(i)), total: 12 }))
    act(() => h.session().communities.loadMorePosts())
    expect(h.driver.last('getCommunityPosts')).toEqual({ kind: 'getCommunityPosts', id: 'c1', page: 2 })
    act(() => h.driver.emit({ kind: 'communityPosts', id: 'c1', page: 2, posts: [post(10), post(11)], total: 12 }))
    expect(h.session().communities.posts?.posts).toHaveLength(12)
    act(() => h.session().communities.loadMorePosts())
    expect(h.driver.sentOf('getCommunityPosts')).toHaveLength(2)
  })

  it("keeps another card's page out", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadPosts('c1'))
    act(() => h.driver.emit({ kind: 'communityPosts', id: 'c2', page: 1, posts: [post(1)], total: 1 }))
    expect(h.session().communities.posts?.posts).toEqual([])
  })

  it('posts once at a time, and counts a post that went through', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadPosts('c1'))
    act(() => h.session().communities.createPost('c1', 'Hello'))
    act(() => h.session().communities.createPost('c1', 'Hello'))
    expect(h.driver.sentOf('createCommunityPost')).toEqual([{ kind: 'createCommunityPost', id: 'c1', content: 'Hello' }])
    act(() => h.driver.emit({ kind: 'communityPostCreated', id: 'c1', ok: true }))
    expect(h.session().communities.posts).toMatchObject({ posting: false, posted: 1 })
    act(() => h.session().communities.createPost('c1', 'Again'))
    act(() => h.driver.emit({ kind: 'communityPostCreated', id: 'c1', ok: false }))
    expect(h.session().communities.posts).toMatchObject({ posting: false, posted: 1 })
    expect(await screen.findByText('There was an error creating the community announcement. Please try again.')).toBeInTheDocument()
  })

  it('likes one request at a time, and counts the like when it went through', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadPosts('c1'))
    act(() => h.driver.emit({ kind: 'communityPosts', id: 'c1', page: 1, posts: [post(1)], total: 1 }))
    act(() => h.session().communities.likePost('c1', 'p1', true))
    act(() => h.session().communities.likePost('c1', 'p1', true))
    expect(h.driver.sentOf('likeCommunityPost')).toHaveLength(1)
    act(() => h.driver.emit({ kind: 'communityPostLiked', id: 'c1', postId: 'p1', liked: true, ok: true }))
    expect(h.session().communities.posts?.posts[0]).toMatchObject({ likes: 3, likedByMe: true })
    act(() => h.session().communities.likePost('c1', 'p1', false))
    act(() => h.driver.emit({ kind: 'communityPostLiked', id: 'c1', postId: 'p1', liked: false, ok: false }))
    expect(h.session().communities.posts?.posts[0]).toMatchObject({ likes: 3, likedByMe: true })
    expect(h.session().communities.posts?.liking).toEqual([])
    expect(await screen.findByText('There was an error sending the like status to the community announcement. Please try again.')).toBeInTheDocument()
  })

  it('toasts a failed read or delete with their texts', async () => {
    render(<ToastHost />)
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().communities.loadPosts('c1'))
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c1', action: 'fetchPosts', message: 'x' }))
    expect(h.session().communities.posts?.loading).toBe(false)
    expect(await screen.findByText('There was an error fetching the community announcements. Please try again.')).toBeInTheDocument()
    expect(h.session().communities.posts?.failed).toBe(true)
    act(() => h.session().communities.deletePost('c1', 'p1'))
    act(() => h.session().communities.deletePost('c1', 'p1'))
    expect(h.driver.sentOf('deleteCommunityPost')).toEqual([{ kind: 'deleteCommunityPost', id: 'c1', postId: 'p1' }])
    expect(h.session().communities.posts?.deleting).toEqual(['p1'])
    act(() => h.driver.emit({ kind: 'communityActionFailed', id: 'c1', action: 'deletePost', message: 'x' }))
    expect(h.session().communities.posts?.deleting).toEqual([])
    expect(await screen.findByText('There was an error deleting the community announcement. Please try again.')).toBeInTheDocument()
    expect(h.session().communities.error).toBeNull()
  })
})

describe('announcements tab', () => {
  it('a member reads and likes, with no composer and no delete on others', async () => {
    const card = renderCard('member', posts([post(1)]))
    expect(screen.queryByRole('textbox', { name: 'New announcement' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Delete announcement' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Like' }))
    expect(card.onLikePost).toHaveBeenCalledWith('p1', true)
  })

  it('says there are none to a member, but not when the read failed', () => {
    renderCard('member', posts([]))
    expect(screen.getByText(NO_ANNOUNCEMENTS)).toBeInTheDocument()
    resetPopups()
    document.body.innerHTML = ''
    renderCard('member', posts([], { failed: true }))
    expect(screen.queryByText(NO_ANNOUNCEMENTS)).toBeNull()
  })

  it('holds a delete while it runs', () => {
    renderCard('owner', posts([post(1)], { deleting: ['p1'] }))
    expect(screen.getByRole('button', { name: 'Delete announcement' })).toBeDisabled()
  })

  it('a member deletes their own announcement, asked first', async () => {
    const card = renderCard('member', posts([post(1, { authorAddress: ME })]))
    await userEvent.click(screen.getByRole('button', { name: 'Delete announcement' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('Are you sure you want to delete this Announcement?')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'DELETE' }))
    expect(card.onDeletePost).toHaveBeenCalledWith('p1')
  })

  it("a moderator posts but deletes only their own; the owner deletes anyone's", () => {
    renderCard('moderator', posts([post(1)]))
    expect(screen.getByRole('textbox', { name: 'New announcement' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete announcement' })).toBeNull()
    resetPopups()
    document.body.innerHTML = ''
    renderCard('owner', posts([post(1)]))
    expect(screen.getByRole('button', { name: 'Delete announcement' })).toBeInTheDocument()
  })

  it('posts the text, held while it posts, and clears it once it went through', async () => {
    const card = renderCard('owner', posts([]))
    const post_ = screen.getByRole('button', { name: 'Post announcement' })
    expect(post_).toBeDisabled()
    await userEvent.type(screen.getByRole('textbox', { name: 'New announcement' }), 'Hello all')
    expect(screen.getByText('9/500')).toBeInTheDocument()
    await userEvent.click(post_)
    expect(card.onPost).toHaveBeenCalledWith('Hello all')
    card.rerender(posts([], { posting: true }))
    expect(screen.getByRole('button', { name: 'Post announcement' })).toBeDisabled()
    expect(screen.getByRole('textbox', { name: 'New announcement' })).toHaveValue('Hello all')
    card.rerender(posts([], { posted: 1 }))
    expect(screen.getByRole('textbox', { name: 'New announcement' })).toHaveValue('')
  })

  it('keeps POST off for a draft of only spaces and newlines', async () => {
    renderCard('owner', posts([]))
    await userEvent.type(screen.getByRole('textbox', { name: 'New announcement' }), '   {Enter}  ')
    expect(screen.getByRole('button', { name: 'Post announcement' })).toBeDisabled()
  })

  it('counts an emoji as two characters, as the field limit does', async () => {
    renderCard('owner', posts([]))
    await userEvent.type(screen.getByRole('textbox', { name: 'New announcement' }), 'hi 😀')
    expect(screen.getByText('5/500')).toBeInTheDocument()
  })

  it('suggests emojis for a ":word", as the chat does', async () => {
    renderCard('owner', posts([]))
    const field = screen.getByRole('textbox', { name: 'New announcement' })
    await userEvent.type(field, 'hi :smil')
    const list = await screen.findByRole('listbox')
    await userEvent.click(within(list).getAllByRole('button')[0])
    expect((field as HTMLTextAreaElement).value).not.toContain(':smil')
  })

  it("shows the author's marks: verified or #tag, and official", () => {
    renderCard('member', posts([post(1, { official: true }), post(2, { hasClaimedName: false, author: 'Anon' })]))
    const [first, second] = screen.getAllByRole('article')
    expect(within(first).getByLabelText('Official')).toBeInTheDocument()
    expect(within(second).getByText(`#${OTHER.slice(-4)}`)).toBeInTheDocument()
    expect(within(second).queryByLabelText('Official')).toBeNull()
  })

  it('writes ages as minutes, hours, then dates', () => {
    const now = Date.parse('2026-11-12T12:00:00Z')
    expect(postAge(now - 30_000, now)).toBe('Now')
    expect(postAge(now - 5 * 60_000, now)).toBe('5m')
    expect(postAge(now - 3 * 3_600_000, now)).toBe('3h')
    expect(postAge(Date.parse('2026-03-04T10:00:00Z'), now)).toBe('Mar 4')
    expect(postAge(Date.parse('2025-03-04T10:00:00Z'), now)).toBe('Mar 4, 2025')
  })
})

describe('a private community', () => {
  const open = (role: string, privacy: string) => {
    const session = fakeSession()
    const loadPosts = vi.fn()
    session.communities = { ...session.communities, cards: { c1: community(role, privacy) }, fetch: vi.fn(), loadPosts }
    render(
      <SessionProvider value={session}>
        <PopupHost />
      </SessionProvider>
    )
    act(() => {
      openCommunityModal('c1', () => undefined)
    })
    return loadPosts
  }

  it("reads no announcements for someone who isn't a member", () => {
    expect(open('none', 'private')).not.toHaveBeenCalled()
  })

  it('reads them for a member, and for anyone on a public one', () => {
    expect(open('member', 'private')).toHaveBeenCalledWith('c1')
    resetPopups()
    document.body.innerHTML = ''
    expect(open('none', 'public')).toHaveBeenCalledWith('c1')
  })

  it("shows a non-member no announcements, no composer and no loading", () => {
    renderCard('none', null, 'private')
    expect(screen.queryByText('Loading…')).toBeNull()
    expect(screen.queryByText(NO_ANNOUNCEMENTS)).toBeNull()
  })
})

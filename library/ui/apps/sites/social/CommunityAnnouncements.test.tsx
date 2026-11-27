import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CommunityAnnouncements, type CommunityAnnouncementsProps } from './CommunityAnnouncements'
import type { HubPostView } from './hub-format'

afterEach(cleanup)

const viewer = '0x1111111111111111111111111111111111111111'
const mine: HubPostView = {
  id: 'mine',
  authorAddress: viewer,
  authorName: 'Moderator',
  content: 'My post',
  createdAt: '2026-09-18T17:40:00Z',
  likesCount: 0
}
const theirs: HubPostView = {
  id: 'theirs',
  authorAddress: '0x2222222222222222222222222222222222222222',
  authorName: 'Owner',
  content: 'Their post',
  createdAt: '2024-06-01T10:00:00Z',
  likesCount: 1
}

function mount(props: Partial<CommunityAnnouncementsProps> = {}) {
  const all: CommunityAnnouncementsProps = {
    posts: [mine, theirs],
    viewer,
    canPost: true,
    isOwner: false,
    canLike: true,
    pending: false,
    onRetry: vi.fn(),
    hasMore: false,
    loadingMore: false,
    onLoadMore: vi.fn(),
    postHref: (id) => `/social?community=club&post=${id}`,
    onPublish: vi.fn(async () => {}),
    onEdit: vi.fn(async () => {}),
    onDelete: vi.fn(async () => {}),
    onLike: vi.fn(async () => {}),
    ...props
  }
  return { props: all, ...render(<CommunityAnnouncements {...all} />) }
}

const article = (text: string) => screen.getByText(text).closest('article') as HTMLElement

describe('when a moderator publishes an announcement', () => {
  it('should keep the draft after a failure and send nothing twice while it is on its way', async () => {
    const user = userEvent.setup()
    let fail!: (error: Error) => void
    const onPublish = vi.fn(() => new Promise<void>((_, reject) => (fail = reject)))
    mount({ onPublish })
    await user.click(screen.getByRole('button', { name: 'New post' }))
    await user.type(screen.getByRole('textbox', { name: 'Post text' }), 'Keep this draft')
    await user.dblClick(screen.getByRole('button', { name: 'Publish post' }))
    expect(onPublish).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled()
    await act(async () => fail(new Error('Storage unavailable')))
    expect(screen.getByRole('alert')).toHaveTextContent('Storage unavailable')
    expect(screen.getByRole('textbox', { name: 'Post text' })).toHaveValue('Keep this draft')
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('textbox', { name: 'Post text' })).toBeNull()
    expect(onPublish).toHaveBeenCalledTimes(1)
  })

  it('should close the form once the announcement is published', async () => {
    const user = userEvent.setup()
    const { props } = mount()
    await user.click(screen.getByRole('button', { name: 'New post' }))
    await user.type(screen.getByRole('textbox', { name: 'Post text' }), 'Hello')
    await user.click(screen.getByRole('button', { name: 'Publish post' }))
    expect(props.onPublish).toHaveBeenCalledWith('Hello')
    await waitFor(() => expect(screen.queryByRole('form', { name: 'New post' })).toBeNull())
  })

  it('should not publish an empty announcement', async () => {
    const user = userEvent.setup()
    mount()
    await user.click(screen.getByRole('button', { name: 'New post' }))
    await user.type(screen.getByRole('textbox', { name: 'Post text' }), '   ')
    expect(screen.getByRole('button', { name: 'Publish post' })).toBeDisabled()
  })
})

describe('when the reader may or may not change announcements', () => {
  it('should let a moderator act on their own announcement only', () => {
    mount()
    expect(within(article('My post')).getByRole('button', { name: 'Post actions' })).toBeTruthy()
    expect(within(article('Their post')).queryByRole('button', { name: 'Post actions' })).toBeNull()
  })

  it("should let an owner delete anyone's announcement but edit only their own", async () => {
    const user = userEvent.setup()
    mount({ isOwner: true })
    expect(screen.getAllByRole('button', { name: 'Post actions' })).toHaveLength(2)
    await user.click(within(article('Their post')).getByRole('button', { name: 'Post actions' }))
    expect(screen.queryByRole('menuitem', { name: 'Edit post' })).toBeNull()
    expect(screen.getByRole('menuitem', { name: 'Delete post' })).toBeTruthy()
  })

  it('should offer a member neither publishing nor post actions', () => {
    mount({ canPost: false })
    expect(screen.queryByRole('button', { name: 'New post' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Post actions' })).toBeNull()
    expect(screen.getByText('Only owners and moderators can publish announcements.')).toBeTruthy()
  })

  it('should offer no likes to a visitor', () => {
    mount({ viewer: null, canPost: false, canLike: false })
    expect(screen.queryByRole('button', { name: 'Like' })).toBeNull()
    expect(within(article('Their post')).getByLabelText('Likes')).toHaveTextContent('1 like')
  })
})

describe('when an announcement is edited', () => {
  it('should keep the new words after a refusal and save only that post', async () => {
    const user = userEvent.setup()
    let fail!: (error: Error) => void
    const onEdit = vi.fn(() => new Promise<void>((_, reject) => (fail = reject)))
    mount({ onEdit })
    await user.click(within(article('My post')).getByRole('button', { name: 'Post actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Edit post' }))
    const form = screen.getByRole('form', { name: 'Edit post' })
    const text = within(form).getByRole('textbox', { name: 'Post text' })
    expect(text).toHaveValue('My post')
    await user.clear(text)
    expect(within(form).getByRole('button', { name: 'Save changes' })).toBeDisabled()
    await user.type(text, 'Revised words')
    await user.dblClick(within(form).getByRole('button', { name: 'Save changes' }))
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ id: 'mine' }), 'Revised words')
    await act(async () =>
      fail(new Error('You no longer have permission to change posts in this community.'))
    )
    expect(screen.getByRole('alert')).toHaveTextContent('no longer have permission')
    expect(text).toHaveValue('Revised words')
  })

  it('should write nothing when the edit is cancelled, and start the next post empty', async () => {
    const user = userEvent.setup()
    const { props } = mount()
    await user.click(within(article('My post')).getByRole('button', { name: 'Post actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Edit post' }))
    await user.type(screen.getByRole('textbox', { name: 'Post text' }), ' unsaved')
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(props.onEdit).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'New post' }))
    expect(screen.getByRole('textbox', { name: 'Post text' })).toHaveValue('')
  })
})

describe('when an announcement is deleted', () => {
  it('should ask first, and keep the post when the reader changes their mind', async () => {
    const user = userEvent.setup()
    const { props } = mount()
    await user.click(within(article('My post')).getByRole('button', { name: 'Post actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Delete post' }))
    const dialog = screen.getByRole('dialog', { name: 'Delete post?' })
    await user.click(within(dialog).getByRole('button', { name: 'Keep post' }))
    expect(props.onDelete).not.toHaveBeenCalled()
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('should keep the dialog open with the reason when the delete fails', async () => {
    const user = userEvent.setup()
    const onDelete = vi.fn(async () => {
      throw new Error('Storage unavailable')
    })
    mount({ onDelete })
    await user.click(within(article('My post')).getByRole('button', { name: 'Post actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Delete post' }))
    await user.click(screen.getByRole('button', { name: 'Delete post' }))
    const dialog = screen.getByRole('dialog', { name: 'Delete post?' })
    expect(within(dialog).getByRole('alert')).toHaveTextContent('Storage unavailable')
  })
})

describe('when the feed loads', () => {
  it('should read oldest first, with the full date on hover', () => {
    mount()
    const [first, second] = screen.getAllByRole('article')
    expect(first).toHaveTextContent('Their post')
    expect(second).toHaveTextContent('My post')
    expect(second?.getAttribute('title')).toMatch(/2026/)
  })

  it('should offer older pages and show that they load', async () => {
    const user = userEvent.setup()
    const { props, rerender } = mount({ hasMore: true })
    await user.click(screen.getByRole('button', { name: 'Load older posts' }))
    expect(props.onLoadMore).toHaveBeenCalledTimes(1)
    rerender(<CommunityAnnouncements {...props} hasMore loadingMore />)
    expect(screen.getByRole('button', { name: 'Loading older posts…' })).toBeDisabled()
  })

  it('should show a failed read with its retry and no posts', async () => {
    const user = userEvent.setup()
    const { props } = mount({ posts: [], error: 'Membership required' })
    expect(screen.getByRole('alert')).toHaveTextContent('Membership required')
    expect(screen.queryByRole('article')).toBeNull()
    expect(screen.queryByText('No announcements yet.')).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(props.onRetry).toHaveBeenCalledTimes(1)
  })

  it('should say where a linked announcement is when it is not loaded', () => {
    const { props, rerender } = mount({ focusPostId: 'gone', hasMore: true })
    expect(screen.getByRole('status')).toHaveTextContent(
      'Load older posts to find this announcement.'
    )
    rerender(<CommunityAnnouncements {...props} hasMore={false} />)
    expect(screen.getByRole('status')).toHaveTextContent('This announcement is unavailable.')
  })

  it('should link each announcement to itself', () => {
    mount()
    expect(
      within(article('My post')).getByRole('link', { name: 'Post link' }).getAttribute('href')
    ).toBe('/social?community=club&post=mine')
  })
})

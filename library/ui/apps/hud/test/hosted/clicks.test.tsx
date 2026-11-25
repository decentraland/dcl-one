import { describe, it, expect, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '../../features/sidebar/Sidebar'
import { Chat } from '../../features/chat/Chat'
import { FriendsPanel } from '../../features/friends/FriendsPanel'
import { EmotesWheel } from '../../features/emotes/EmotesWheel'
import type { EngineSession, FriendsState } from '../../features/session/useEngineSession'
import { renderSettled, fakeSession } from '../harness'
import { openFriendRequest } from '../../features/friends/FriendRequestPopup'

vi.mock('../../features/friends/FriendRequestPopup', () => ({ openFriendRequest: vi.fn() }))
vi.mock('../../features/friends/friendDialogs', () => ({ confirmUnblock: vi.fn(async () => true) }))

// CLICK coverage: render the real components and assert each click invokes the
// session method. Tier 1 proves every method posts the right wire message, so
// these complete the click → API-call chain through real DOM events.
describe('sidebar clicks', () => {
  const cases: [string, (s: EngineSession) => () => void][] = [
    ['Notifications', (s) => s.notifications.toggle],
    ['Emotes', (s) => s.emotes.toggle],
    ['Friends', (s) => s.friends.toggle],
    ['Chat', (s) => s.chat.toggle]
  ]

  it.each(cases)('clicking %s triggers its session action', async (name, pick) => {
    const s = fakeSession()
    await renderSettled(<Sidebar session={s} />)
    await userEvent.click(screen.getByRole('button', { name }))
    expect(vi.mocked(pick(s))).toHaveBeenCalledTimes(1)
  })

  // The pages open from a row of the panel a bar button opens (a dialog named as the button).
  const rows: [string, string, (s: EngineSession) => (...args: never[]) => void][] = [
    ['Me', 'Backpack', (s) => s.backpack.toggle],
    ['Me', 'Profile', (s) => s.profile.toggle],
    ['Discover', 'Events', (s) => s.events.toggle],
    ['Discover', 'Places', (s) => s.places.toggle],
    ['Discover', 'Communities', (s) => s.communities.toggle],
    ['System', 'Settings', (s) => s.settings.toggle],
    ['Camera', 'Gallery', (s) => s.gallery.toggle]
  ]

  it.each(rows)('%s, then its %s row, triggers the session action and closes the panel', async (button, row, pick) => {
    const s = fakeSession()
    await renderSettled(<Sidebar session={s} />)
    await userEvent.click(screen.getByRole('button', { name: button }))
    await userEvent.click(within(screen.getByRole('dialog', { name: button })).getByRole('button', { name: row }))
    expect(vi.mocked(pick(s))).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('dialog', { name: button })).toBeNull()
  })

  it('the Events row opens the page as the sidebar\'s entry', async () => {
    const s = fakeSession()
    await renderSettled(<Sidebar session={s} />)
    await userEvent.click(screen.getByRole('button', { name: 'Discover' }))
    await userEvent.click(screen.getByRole('button', { name: 'Events' }))
    expect(vi.mocked(s.events.toggle)).toHaveBeenCalledWith('sidebar')
  })

  it('lists the bar\'s buttons in order: the top group, then the bottom group', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels).toEqual([
      'Sidebar settings', 'Me', 'Map', 'Discover', 'Shop', 'System', 'Notifications',
      'Scene permissions', 'Skybox', 'Volume', 'Emotes', 'Friends', 'Camera', 'Nearby Voice', 'Chat'
    ])
  })

  it('the "..." panel closes on an outside click', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Sidebar settings' }))
    await userEvent.click(screen.getByRole('switch', { name: 'Hide Sidebar Automatically' }))
    expect(screen.getByRole('dialog', { name: 'Sidebar settings' })).toBeInTheDocument()
    await userEvent.click(document.body)
    expect(screen.queryByRole('dialog', { name: 'Sidebar settings' })).toBeNull()
  })

  it('every rail button has a tooltip naming it', async () => {
    // with the chat closed: a button whose own panel is open drops its tooltip (sidebar.test.tsx)
    const s = fakeSession()
    s.chat = { ...s.chat, open: false }
    await renderSettled(<Sidebar session={s} />)
    // a hint that says more than the name: Camera names both of its rows
    const hints: Record<string, string> = { Camera: 'Camera / Gallery' }
    for (const b of screen.getAllByRole('button')) {
      const label = b.getAttribute('aria-label') ?? ''
      if (label === 'Sidebar settings') continue
      expect(b.parentElement?.querySelector('[role="tooltip"]')).toHaveTextContent(hints[label] ?? label)
    }
  })

  it('the Me panel\'s Profile row opens the passport (onViewProfile) when provided, not the small panel', async () => {
    const s = fakeSession()
    const onViewProfile = vi.fn()
    await renderSettled(<Sidebar session={s} onViewProfile={onViewProfile} />)
    await userEvent.click(screen.getByRole('button', { name: 'Me' }))
    await userEvent.click(screen.getByRole('button', { name: 'Profile' }))
    expect(onViewProfile).toHaveBeenCalledTimes(1)
    expect(vi.mocked(s.profile.toggle)).not.toHaveBeenCalled()
  })

  it('Me shows the player avatar once the profile has loaded', async () => {
    const s = fakeSession()
    s.profile.data = { address: '0xabc', name: 'Mojito', picture: 'https://example.com/face.png', hasClaimedName: true, isGuest: false }
    await renderSettled(<Sidebar session={s} />)
    const button = screen.getByRole('button', { name: 'Me' })
    expect(button.querySelector('img')).toHaveAttribute('src', 'https://example.com/face.png')
  })

  it('Me keeps the generic icon while the profile is loading', async () => {
    const s = fakeSession()
    s.profile.data = null
    await renderSettled(<Sidebar session={s} />)
    expect(screen.getByRole('button', { name: 'Me' }).querySelector('img')).toBeNull()
  })
})

describe('chat input', () => {
  it('typing a message and pressing Enter sends it', async () => {
    const s = fakeSession()
    await renderSettled(<Chat chat={s.chat} />)
    await userEvent.type(screen.getByRole('textbox'), 'gm frens{Enter}')
    expect(vi.mocked(s.chat.send)).toHaveBeenCalledWith('gm frens')
  })

  it('does not send an empty message', async () => {
    const s = fakeSession()
    await renderSettled(<Chat chat={s.chat} />)
    await userEvent.type(screen.getByRole('textbox'), '{Enter}')
    expect(vi.mocked(s.chat.send)).not.toHaveBeenCalled()
  })
})

describe('friends panel action clicks', () => {
  const onMessage = vi.fn()
  function renderPanel(over: Partial<FriendsState>): FriendsState {
    const friends: FriendsState = { ...fakeSession().friends, open: true, available: true, act: vi.fn(), ...over }
    render(<FriendsPanel friends={friends} onMessage={onMessage} />)
    return friends
  }

  it("a friend row's envelope opens a DM with them", async () => {
    renderPanel({ list: [{ address: '0xf', name: 'F', status: 'online' }] })
    await userEvent.click(screen.getByRole('button', { name: /^Chat with F/ }))
    expect(onMessage).toHaveBeenCalledWith('0xf')
  })

  it('accept / reject a received request', async () => {
    const friends = renderPanel({ received: [{ address: '0xr', name: 'R', id: 'r1' }] })
    await userEvent.click(screen.getByRole('tab', { name: /Requests/ }))
    await userEvent.click(screen.getByRole('button', { name: 'Accept' }))
    expect(vi.mocked(openFriendRequest)).toHaveBeenCalledWith('accept', expect.objectContaining({ address: '0xr' }))
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }))
    expect(vi.mocked(friends.act)).toHaveBeenCalledWith('reject', '0xr')
  })

  it('cancel a sent request', async () => {
    const friends = renderPanel({ sent: [{ address: '0xs', name: 'S', id: 's1' }] })
    await userEvent.click(screen.getByRole('tab', { name: /Requests/ }))
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(vi.mocked(friends.act)).toHaveBeenCalledWith('cancel', '0xs')
  })

  it('unblock a blocked user', async () => {
    const friends = renderPanel({ blocked: ['0xb'] })
    await userEvent.click(screen.getByRole('tab', { name: /Blocked/ }))
    await userEvent.click(screen.getByRole('button', { name: 'Unblock' }))
    await vi.waitFor(() => expect(vi.mocked(friends.act)).toHaveBeenCalledWith('unblock', '0xb'))
  })
})

describe('emotes wheel clicks', () => {
  it('close button toggles the wheel', async () => {
    const s = fakeSession()
    s.emotes = { ...s.emotes, open: true, toggle: vi.fn() }
    await renderSettled(<EmotesWheel emotes={s.emotes} />)
    await userEvent.click(screen.getByRole('button', { name: 'Close emotes' }))
    expect(vi.mocked(s.emotes.toggle)).toHaveBeenCalled()
  })
})

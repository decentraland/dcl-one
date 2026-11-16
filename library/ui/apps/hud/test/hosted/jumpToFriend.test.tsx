// Jump to Location: offered for a friend who is not offline — in the profile card (the friend row's
// menu, chat names, nametags, the voice panel), on the friend row, and in the passport — and it
// travels to where the friend is, as unity-explorer does.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProfileCard } from '../../features/profileCard/ProfileCard'
import { Passport } from '../../features/profile/Passport'
import { FriendsPanel } from '../../features/friends/FriendsPanel'
import { jumpToFriend } from '../../features/friends/jumpToFriend'
import { SessionProvider } from '../../features/session/SessionContext'
import { fakeSession, renderSettled } from '../harness'
import { resetPopups } from '../../design'
import { locateFriend, type FriendLocation } from '@explorer-core/features/friends/friendLocation'
import type { EngineSession, FriendsState } from '../../features/session/useEngineSession'
import type { Friend } from '@explorer-core/engine/protocol'

vi.mock('@explorer-core/features/friends/friendLocation', () => ({ locateFriend: vi.fn() }))

afterEach(() => {
  resetPopups()
  vi.mocked(locateFriend).mockReset()
})

const ONLINE: Friend = { address: '0xf', name: 'Fran', status: 'online' }
const AWAY: Friend = { address: '0xa', name: 'Ada', status: 'away' }
const OFFLINE: Friend = { address: '0xo', name: 'Oli', status: 'offline' }

function withSession(node: React.ReactNode, list: Friend[] = [ONLINE, AWAY, OFFLINE]): EngineSession {
  const s = fakeSession()
  s.friends.list = list
  render(<SessionProvider value={s}>{node}</SessionProvider>)
  return s
}

describe('jumpToFriend', () => {
  it("teleports to the friend's Genesis parcel, or changes realm to their World", async () => {
    const map = { teleportToPlace: vi.fn(), changeRealm: vi.fn() }
    vi.mocked(locateFriend).mockResolvedValueOnce({ kind: 'parcel', x: -9, y: 4 })
    expect(await jumpToFriend(map, '0xf')).toEqual({ kind: 'parcel', x: -9, y: 4 })
    expect(map.teleportToPlace).toHaveBeenCalledWith(-9, 4)
    vi.mocked(locateFriend).mockResolvedValueOnce({ kind: 'world', world: 'boedo.dcl.eth' })
    await jumpToFriend(map, '0xf')
    expect(map.changeRealm).toHaveBeenCalledWith('boedo.dcl.eth')
  })

  it('does nothing when the friend is not found', async () => {
    const map = { teleportToPlace: vi.fn(), changeRealm: vi.fn() }
    vi.mocked(locateFriend).mockResolvedValueOnce(null)
    expect(await jumpToFriend(map, '0xf')).toBeNull()
    expect(map.teleportToPlace).not.toHaveBeenCalled()
    expect(map.changeRealm).not.toHaveBeenCalled()
  })

  it('a second jump drops the first one still looking up', async () => {
    const map = { teleportToPlace: vi.fn(), changeRealm: vi.fn() }
    let first!: (v: { kind: 'parcel'; x: number; y: number }) => void
    vi.mocked(locateFriend)
      .mockImplementationOnce(() => new Promise((resolve) => (first = resolve)))
      .mockResolvedValueOnce({ kind: 'parcel', x: 2, y: 2 })
    const a = jumpToFriend(map, '0xf')
    await jumpToFriend(map, '0xa')
    first({ kind: 'parcel', x: 1, y: 1 })
    expect(await a).toBeNull()
    expect(map.teleportToPlace).toHaveBeenCalledTimes(1)
    expect(map.teleportToPlace).toHaveBeenCalledWith(2, 2)
  })
})

describe('profile card', () => {
  const card = (userId: string): React.JSX.Element => <ProfileCard userId={userId} x={10} y={10} onClose={vi.fn()} />

  it('offers Jump to Location for a friend who is online or away, and jumps', async () => {
    vi.mocked(locateFriend).mockResolvedValue({ kind: 'parcel', x: 3, y: -7 })
    const s = withSession(card('0xf'))
    await userEvent.click(screen.getByRole('button', { name: 'Jump to Location' }))
    expect(locateFriend).toHaveBeenCalledWith('0xf', expect.anything())
    await vi.waitFor(() => expect(s.map.teleportToPlace).toHaveBeenCalledWith(3, -7))
  })

  it('offers it for an away friend too', () => {
    withSession(card('0xa'))
    expect(screen.getByRole('button', { name: 'Jump to Location' })).toBeTruthy()
  })

  it('does not offer it for an offline friend or someone who is not a friend', () => {
    withSession(
      <>
        {card('0xo')}
        {card('0xstranger')}
      </>
    )
    expect(screen.queryByRole('button', { name: 'Jump to Location' })).toBeNull()
  })

  it("is not on the community card's member menu", () => {
    withSession(<ProfileCard userId="0xf" x={10} y={10} context="community" onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Jump to Location' })).toBeNull()
  })
})

describe('friends panel row', () => {
  type OnJump = (address: string) => Promise<FriendLocation | null>
  function renderPanel(list: Friend[], onJump = vi.fn<OnJump>(async () => ({ kind: 'parcel', x: 0, y: 0 }))): { friends: FriendsState; onJump: typeof onJump } {
    const friends: FriendsState = { ...fakeSession().friends, open: true, available: true, list, toggle: vi.fn() }
    render(<FriendsPanel friends={friends} onMessage={vi.fn()} onJump={onJump} />)
    return { friends, onJump }
  }

  it('has a Jump button for a friend who is not offline, and the panel closes once the parcel is known', async () => {
    const { friends, onJump } = renderPanel([ONLINE, OFFLINE])
    const buttons = screen.getAllByRole('button', { name: /^Jump to .*'s location$/ })
    expect(buttons).toHaveLength(1)
    await userEvent.click(buttons[0])
    expect(onJump).toHaveBeenCalledWith('0xf')
    await vi.waitFor(() => expect(friends.toggle).toHaveBeenCalledTimes(1))
  })

  it('stays open when the jump goes to a World or finds no one', async () => {
    const onJump = vi.fn<OnJump>(async () => ({ kind: 'world', world: 'boedo.dcl.eth' }))
    const { friends } = renderPanel([ONLINE], onJump)
    await userEvent.click(screen.getByRole('button', { name: /^Jump to .*'s location$/ }))
    await vi.waitFor(() => expect(onJump).toHaveBeenCalled())
    await Promise.resolve()
    expect(friends.toggle).not.toHaveBeenCalled()
  })
})

describe('passport', () => {
  it('shows Jump to Location in the header and the options menu for an online friend, and closes after jumping', async () => {
    vi.mocked(locateFriend).mockResolvedValue({ kind: 'world', world: 'boedo.dcl.eth' })
    const s = fakeSession()
    s.friends.list = [ONLINE]
    const onClose = vi.fn()
    await renderSettled(
      <SessionProvider value={s}>
        <Passport userId="0xf" onClose={onClose} />
      </SessionProvider>
    )
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    expect(screen.getByRole('menuitem', { name: 'Jump to Location' })).toBeTruthy()
    await userEvent.click(screen.getAllByRole('button', { name: 'Jump to Location' })[0])
    await vi.waitFor(() => expect(s.map.changeRealm).toHaveBeenCalledWith('boedo.dcl.eth'))
    expect(onClose).toHaveBeenCalled()
  })

  it('has no Jump for an offline friend', async () => {
    const s = fakeSession()
    s.friends.list = [OFFLINE]
    await renderSettled(
      <SessionProvider value={s}>
        <Passport userId="0xo" onClose={vi.fn()} />
      </SessionProvider>
    )
    expect(screen.queryByRole('button', { name: 'Jump to Location' })).toBeNull()
  })
})

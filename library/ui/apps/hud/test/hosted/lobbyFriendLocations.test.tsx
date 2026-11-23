import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fetchWhereabouts, type FriendWhere } from '@explorer-core/features/lobby/lobbyApi'
import { useFriendLocations } from '../../features/lobby/useLobbyData'

vi.mock('@explorer-core/features/lobby/lobbyApi', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@explorer-core/features/lobby/lobbyApi')>()),
  fetchWhereabouts: vi.fn(),
  whereTitle: vi.fn(async (where: FriendWhere) => (where.kind === 'genesis' ? `${where.parcel.x},${where.parcel.y}` : 'Lobby'))
}))

type Friend = Parameters<typeof useFriendLocations>[0][number]
const friend = { address: '0xAAA' } as Friend
const at = (x: number): Map<string, FriendWhere> => new Map([['0xaaa', { kind: 'genesis', parcel: { x, y: 0 } }]])

// lets the read's promise chain (whereabouts, then titles) settle under fake timers
const settle = async (): Promise<void> => {
  for (let i = 0; i < 5; i++) await act(async () => undefined)
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(fetchWhereabouts).mockReset()
})
afterEach(() => {
  vi.useRealTimers()
})

describe('lobby friend locations', () => {
  it('reads an online friend once, and again after a minute while the lobby stays open', async () => {
    vi.mocked(fetchWhereabouts).mockResolvedValueOnce(at(1)).mockResolvedValueOnce(at(2))
    const online = [friend]
    const { result } = renderHook(() => useFriendLocations(online))
    await settle()
    expect(result.current.get('0xaaa')?.title).toBe('1,0')
    await act(async () => void vi.advanceTimersByTime(30_000))
    expect(fetchWhereabouts).toHaveBeenCalledTimes(1)
    await act(async () => void vi.advanceTimersByTime(45_000))
    await settle()
    expect(fetchWhereabouts).toHaveBeenCalledTimes(2)
    expect(result.current.get('0xaaa')?.title).toBe('2,0')
  })

  it('does not count a friend as read when the answer to its read was dropped', async () => {
    let answer: (found: Map<string, FriendWhere>) => void = () => undefined
    vi.mocked(fetchWhereabouts)
      .mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
      .mockResolvedValue(at(3))
    const { result, rerender } = renderHook(({ online }) => useFriendLocations(online), { initialProps: { online: [friend] } })
    // the set of online friends changes while the first read is out: its answer is dropped
    rerender({ online: [friend, { address: '0xBBB' } as Friend] })
    answer(at(9))
    await settle()
    expect(fetchWhereabouts).toHaveBeenCalledTimes(2)
    expect(result.current.get('0xaaa')?.title).toBe('3,0')
  })
})

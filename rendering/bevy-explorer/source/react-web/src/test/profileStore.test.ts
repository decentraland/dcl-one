import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import {
  peekProfile,
  profileChanged,
  receiveProfile,
  requestPassport,
  resetProfileStore,
  seedProfiles,
  setOwnProfile,
  setProfileRequester,
  subscribeProfile,
  useProfile
} from '../features/session/profileStore'
import type { Profile, Wearable } from '../engine/protocol'

// STORE: the one address→identity map. Requests go out only for what's shown, once; a list's seed
// never overwrites the engine's copy; the engine's change stream re-reads only what's still shown.
const full = (over: Partial<Profile> = {}): Profile => ({
  address: '0xabc',
  name: 'Alice',
  picture: 'a.png',
  hasClaimedName: true,
  isGuest: false,
  version: 3,
  ...over
})

const request = vi.fn()
beforeEach(() => {
  resetProfileStore()
  request.mockClear()
  setProfileRequester(request)
})

describe('profile store', () => {
  it('asks the engine once for a shown address, however many subscribers arrive', () => {
    const off1 = subscribeProfile('0xabc', vi.fn())
    const off2 = subscribeProfile('0xabc', vi.fn())
    expect(request).toHaveBeenCalledTimes(1)
    expect(request).toHaveBeenCalledWith('0xabc', false)
    off1()
    off2()
  })

  it('does not ask for an address it already holds the engine copy of', () => {
    receiveProfile('0xABC', full())
    subscribeProfile('0xabc', vi.fn())
    expect(request).not.toHaveBeenCalled()
    expect(peekProfile('0xAbC')?.name).toBe('Alice') // matched case-insensitively
  })

  it('notifies subscribers when what it holds changes', () => {
    const cb = vi.fn()
    subscribeProfile('0xabc', cb)
    seedProfiles([{ address: '0xabc', name: 'Alice' }])
    expect(cb).toHaveBeenCalledTimes(1)
    seedProfiles([{ address: '0xabc', name: 'Alice' }]) // unchanged → no re-render
    expect(cb).toHaveBeenCalledTimes(1)
    receiveProfile('0xabc', full())
    expect(cb).toHaveBeenCalledTimes(2)
  })

  it('a list seed never overwrites the engine copy, and keeps a passport\'s extras through a re-read', () => {
    receiveProfile('0xabc', full({ badges: [{ id: 'b', name: 'Badge' }] }))
    seedProfiles([{ address: '0xabc', name: 'Stale' }])
    expect(peekProfile('0xabc')?.name).toBe('Alice')
    receiveProfile('0xabc', full({ name: 'Alicia', version: 4 })) // a plain identity re-read: no badges key
    expect(peekProfile('0xabc')?.name).toBe('Alicia')
    expect(peekProfile('0xabc')?.badges?.length).toBe(1)
  })

  it('the engine copy replaces a seed only where it says something', () => {
    seedProfiles([{ address: '0xabc', name: 'Alice', picture: 'a.png' }])
    receiveProfile('0xabc', full({ name: 'Alicia', picture: undefined }))
    expect(peekProfile('0xabc')).toMatchObject({ name: 'Alicia', picture: 'a.png', version: 3 })
    receiveProfile('0xabc', full({ name: '0xabc', picture: 'b.png', description: undefined }))
    expect(peekProfile('0xabc')).toMatchObject({ name: 'Alicia', picture: 'b.png' })
    expect(peekProfile('0xabc')?.description).toBeUndefined() // a cleared field is the engine's word
  })

  it('a "no profile" answer stands for a while rather than being re-asked per subscriber', () => {
    subscribeProfile('0xabc', vi.fn())
    receiveProfile('0xabc', null)
    subscribeProfile('0xabc', vi.fn())
    expect(request).toHaveBeenCalledTimes(1)
  })

  it('a passport request always goes out, with extras', () => {
    receiveProfile('0xabc', full())
    requestPassport('0xabc')
    expect(request).toHaveBeenCalledWith('0xabc', true)
  })

  it('profileChanged re-reads a shown profile whatever version the engine reports', () => {
    subscribeProfile('0xabc', vi.fn())
    receiveProfile('0xabc', full({ version: 3 }))
    request.mockClear()
    // a lower version: the engine put an earlier profile back
    profileChanged('0xabc', 2)
    expect(request).toHaveBeenCalledWith('0xabc', false)
    request.mockClear()
    // the same version: server-filled fields (the snapshots) may have landed
    profileChanged('0xabc', 3)
    expect(request).toHaveBeenCalledWith('0xabc', false)
    request.mockClear()
    profileChanged('0xabc', 4)
    expect(request).toHaveBeenCalledWith('0xabc', false)
    expect(peekProfile('0xabc')?.version).toBe(3) // the old copy stays up until the reply lands
  })

  it('profileChanged drops a profile nobody is showing, and ignores one never seen', () => {
    receiveProfile('0xabc', full())
    profileChanged('0xabc', 9)
    expect(peekProfile('0xabc')).toBeUndefined()
    profileChanged('0xnew', 1)
    expect(request).not.toHaveBeenCalled()
  })

  it('does not ask the engine about anything but an address', () => {
    subscribeProfile('system', vi.fn())
    subscribeProfile('', vi.fn())
    expect(request).not.toHaveBeenCalled()
  })
})

// A Backpack edit of your own profile whose deploy fails: the engine puts the profile from before it
// back and announces that version, lower than the edit's. The passport and the profile card both
// read useProfile; the passport also asks for the extras (the equipped set the Backpack changes).
describe('profile store: your own edit rejected by the deploy', () => {
  const hat: Wearable = { urn: 'urn:test:hat', name: 'Hat', rarity: 'base', category: 'hat', equipped: true }
  const cap: Wearable = { urn: 'urn:test:cap', name: 'Cap', rarity: 'base', category: 'hat', equipped: true }
  const mine = (version: number, worn: Wearable): Profile => ({
    address: '0xme',
    name: 'Me#ab12',
    hasClaimedName: false,
    isGuest: false,
    version,
    equippedWearables: [worn]
  })
  // the session tells the store whose profile is the local player's
  beforeEach(() => setOwnProfile('0xme'))

  it('re-reads the lower version, and what the engine put back reaches the passport and profile card', () => {
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => {
      requestPassport('0xme')
      receiveProfile('0xme', mine(3, hat))
    })
    // the Backpack's setAvatar: the engine announces the edit and the store reads it
    act(() => {
      profileChanged('0xme', 4)
      receiveProfile('0xme', mine(4, cap))
    })
    expect(result.current?.equippedWearables).toEqual([cap])
    request.mockClear()

    act(() => profileChanged('0xme', 3))
    expect(request).toHaveBeenCalledWith('0xme', true)
    act(() => receiveProfile('0xme', mine(3, hat)))
    expect(result.current).toMatchObject({ version: 3, equippedWearables: [hat] })
  })

  it('drops a read of the edit that is answered after the engine put the earlier profile back', () => {
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => {
      requestPassport('0xme')
      receiveProfile('0xme', mine(3, hat))
    })
    // the edit is announced and read, but its deploy fails before that read is answered
    act(() => {
      profileChanged('0xme', 4)
      profileChanged('0xme', 3)
    })
    // a passport read also waits on the badges and photos services, so the read made after the
    // revert can be answered before the one made before it
    act(() => {
      receiveProfile('0xme', mine(3, hat))
      receiveProfile('0xme', mine(4, cap))
    })
    expect(result.current).toMatchObject({ version: 3, equippedWearables: [hat] })
  })

  it('recognises the revert when the edit was announced while nobody showed the profile', () => {
    // your profile is shown nowhere when the Backpack's edit is announced: the copy is forgotten
    receiveProfile('0xme', mine(3, hat))
    profileChanged('0xme', 4)
    // the passport opens while the edit deploys
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => requestPassport('0xme'))
    // the deploy fails; the read made after the revert is answered before the passport's first one
    act(() => {
      profileChanged('0xme', 3)
      receiveProfile('0xme', mine(3, hat))
      receiveProfile('0xme', mine(4, cap))
    })
    expect(result.current).toMatchObject({ version: 3, equippedWearables: [hat] })
  })

  it('recognises the revert when the edit was learned from a read, not an announcement', () => {
    // the store had no entry when the edit was announced; a reader then fetched it
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => {
      receiveProfile('0xme', mine(4, cap))
      requestPassport('0xme')
    })
    // the deploy fails; the passport's read of the edit is answered last
    act(() => {
      profileChanged('0xme', 3)
      receiveProfile('0xme', mine(3, hat))
      receiveProfile('0xme', mine(4, cap))
    })
    expect(result.current).toMatchObject({ version: 3, equippedWearables: [hat] })
  })

  it('keeps your own entry, and its revert, through the idle sweep', () => {
    vi.useFakeTimers()
    try {
      const off = subscribeProfile('0xme', vi.fn())
      act(() => {
        receiveProfile('0xme', mine(4, cap))
        profileChanged('0xme', 4)
        profileChanged('0xme', 3)
        receiveProfile('0xme', mine(3, hat))
      })
      off()
      // a passport read of the edit outlasts the idle time and the sweep
      vi.advanceTimersByTime(200_000)
      seedProfiles([]) // any store call sweeps
      receiveProfile('0xme', mine(4, cap))
      expect(peekProfile('0xme')).toMatchObject({ version: 3, equippedWearables: [hat] })
    } finally {
      vi.useRealTimers()
    }
  })

  it('takes the edit again once the engine announces it after a revert', () => {
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => {
      receiveProfile('0xme', mine(3, hat))
      profileChanged('0xme', 4)
      profileChanged('0xme', 3)
      receiveProfile('0xme', mine(3, hat))
    })
    // a retry: the engine applies the edit again, under a version above the one that failed
    act(() => {
      profileChanged('0xme', 5)
      receiveProfile('0xme', mine(5, cap))
    })
    expect(result.current).toMatchObject({ version: 5, equippedWearables: [cap] })
  })

  it('drops reads from before the retry once the retry is announced', () => {
    const { result } = renderHook(() => useProfile('0xme'))
    act(() => {
      requestPassport('0xme')
      receiveProfile('0xme', mine(3, hat))
    })
    // the edit, its failed deploy and the retry are announced before the passport's reads come back
    act(() => {
      profileChanged('0xme', 4)
      profileChanged('0xme', 3)
      profileChanged('0xme', 5)
    })
    // the retry's read is answered first; the restored copy and a late badge-tier copy of the
    // rejected edit after it
    act(() => {
      receiveProfile('0xme', mine(5, cap))
      receiveProfile('0xme', mine(3, hat))
      receiveProfile('0xme', mine(4, cap))
    })
    expect(result.current).toMatchObject({ version: 5, equippedWearables: [cap] })
  })

  it('takes a copy newer than the announced version when nothing was put back', () => {
    // another player's profile can reach the engine's cache before their avatar announces it
    const { result } = renderHook(() => useProfile('0xabc'))
    act(() => {
      profileChanged('0xabc', 3)
      receiveProfile('0xabc', full({ version: 4, name: 'Alicia' }))
    })
    expect(result.current).toMatchObject({ version: 4, name: 'Alicia' })
  })

  // another player's profile can reach the engine's cache before their avatar announces an older
  // version; only the local player's own failed deploy puts a version back
  it('does not take another player\'s late, lower announcement for a revert', () => {
    const { result } = renderHook(() => useProfile('0xabc'))
    act(() => {
      receiveProfile('0xabc', full({ version: 4 }))
      profileChanged('0xabc', 3)
      requestPassport('0xabc')
      receiveProfile('0xabc', full({ version: 4, badges: [{ id: 'b', name: 'Badge' }] }))
    })
    expect(result.current).toMatchObject({ version: 4, badges: [{ id: 'b', name: 'Badge' }] })
  })

  it('a later reader gets another player\'s copy after their late, lower announcement', () => {
    // nobody shows them when the older version is announced: the copy is forgotten
    receiveProfile('0xabc', full({ version: 4 }))
    profileChanged('0xabc', 3)
    const { result } = renderHook(() => useProfile('0xabc'))
    act(() => receiveProfile('0xabc', full({ version: 4 })))
    expect(result.current).toMatchObject({ version: 4 })
  })
})

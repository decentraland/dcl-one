// Donate in the shared HUD (features/donate/useDonate, notifications/summary's tipTarget): shown only where the
// host can tip and the place has a creator, and a tip's notification opens the tipper's passport.
import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const host = vi.hoisted(() => ({
  donations: null as null | {
    available: ReturnType<typeof vi.fn>
    tippable: ReturnType<typeof vi.fn>
    open: ReturnType<typeof vi.fn>
    onSent: ReturnType<typeof vi.fn>
  }
}))
vi.mock('@hud-host', async (importOriginal) => {
  const real = await importOriginal<object>()
  return {
    ...real,
    get donations() {
      return host.donations
    }
  }
})
const passport = vi.hoisted(() => vi.fn())
vi.mock('../features/profile/Passport', async (importOriginal) => ({ ...(await importOriginal<object>()), openPassport: passport }))

import { TIP_EMOTE, useDonate, useTipEmote } from '../features/donate/useDonate'
import { tipTarget } from '../features/notifications/summary'

const PLACE = { parcel: { x: 1, y: 2 } }
const settle = () => act(async () => {})

describe('Donate', () => {
  beforeEach(() => {
    host.donations = {
      available: vi.fn(async () => true),
      tippable: vi.fn(async () => true),
      open: vi.fn(async () => {}),
      onSent: vi.fn(() => () => {})
    }
  })

  it('shows for a place with a creator, where the host can tip', async () => {
    const { result } = renderHook(() => useDonate(PLACE))
    await settle()
    expect(result.current.visible).toBe(true)
    result.current.open('pointer')
    expect(host.donations!.open).toHaveBeenCalledWith(PLACE, 'pointer')
  })

  it('hides where the host can’t tip, the session may not, or the place has no creator', async () => {
    host.donations!.tippable.mockResolvedValue(false)
    const noCreator = renderHook(() => useDonate(PLACE))
    await settle()
    expect(noCreator.result.current.visible).toBe(false)
    host.donations!.available.mockResolvedValue(false)
    host.donations!.tippable.mockResolvedValue(true)
    const notWeb3 = renderHook(() => useDonate(PLACE))
    await settle()
    expect(notWeb3.result.current.visible).toBe(false)
    const nowhere = renderHook(() => useDonate(null))
    await settle()
    expect(nowhere.result.current.visible).toBe(false)
    host.donations = null
    const desktop = renderHook(() => useDonate(PLACE))
    await settle()
    expect(desktop.result.current.visible).toBe(false)
  })

  it('plays the tip emote when the host says a tip was sent', () => {
    let sent: () => void = () => {}
    host.donations!.onSent.mockImplementation((fn: () => void) => {
      sent = fn
      return () => {}
    })
    const play = vi.fn()
    renderHook(() => useTipEmote(play))
    sent()
    expect(play).toHaveBeenCalledWith(TIP_EMOTE)
  })

  it('opens the tipper’s passport from a tip’s notification', () => {
    const open = tipTarget({ id: '1', type: 'tip_received', address: '0x', timestamp: '0', read: false, metadata: { senderAddress: '0xabc' } } as never)
    open?.()
    expect(passport).toHaveBeenCalledWith('0xabc')
    expect(tipTarget({ id: '2', type: 'badge_granted', address: '0x', timestamp: '0', read: false, metadata: {} } as never)).toBeUndefined()
  })
})

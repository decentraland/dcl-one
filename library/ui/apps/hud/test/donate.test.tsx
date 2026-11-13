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
import { summarize, tipTarget } from '../features/notifications/summary'

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
    host.donations!.tippable.mockClear()
    host.donations!.tippable.mockResolvedValue(true)
    const notWeb3 = renderHook(() => useDonate(PLACE))
    await settle()
    expect(notWeb3.result.current.visible).toBe(false)
    // The place is looked up only for a session that may tip.
    expect(host.donations!.tippable).not.toHaveBeenCalled()
    const nowhere = renderHook(() => useDonate(null))
    await settle()
    expect(nowhere.result.current.visible).toBe(false)
    host.donations = null
    const desktop = renderHook(() => useDonate(PLACE))
    await settle()
    expect(desktop.result.current.visible).toBe(false)
  })

  it('hides on the map as soon as another place is picked, and keeps the last answer on the minimap', async () => {
    const OTHER = { parcel: { x: 9, y: 9 } }
    let answer: (v: boolean) => void = () => {}
    const map = renderHook(({ place }) => useDonate(place), { initialProps: { place: PLACE as typeof PLACE | typeof OTHER } })
    const minimap = renderHook(({ place }) => useDonate(place, { keepWhileAsking: true }), { initialProps: { place: PLACE as typeof PLACE | typeof OTHER } })
    await settle()
    expect(map.result.current.visible).toBe(true)
    host.donations!.tippable.mockImplementation(() => new Promise<boolean>((r) => (answer = r)))
    map.rerender({ place: OTHER })
    minimap.rerender({ place: OTHER })
    await settle()
    expect(map.result.current.visible).toBe(false)
    expect(minimap.result.current.visible).toBe(true)
    answer(false)
    await settle()
    expect(minimap.result.current.visible).toBe(false)
  })

  it('asks whether the session may tip once, not again for each place', async () => {
    const hook = renderHook(({ place }) => useDonate(place, { keepWhileAsking: true }), { initialProps: { place: PLACE as { parcel: { x: number; y: number } } } })
    await settle()
    hook.rerender({ place: { parcel: { x: 9, y: 9 } } })
    await settle()
    expect(host.donations!.available).toHaveBeenCalledOnce()
    expect(host.donations!.tippable).toHaveBeenCalledTimes(2)
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
    // The notification carries the tipper's address only: their name when the HUD already has their profile, else
    // the short address.
    const sender = '0x' + 'ab'.repeat(20)
    expect(summarize({ id: '3', type: 'tip_received', address: '0x', timestamp: '0', read: false, metadata: { senderAddress: sender, amount: '166.456' } } as never)).toMatchObject({
      title: 'New Tip received!',
      body: '0xabab…abab sent you a 166.46 MANA tip for one of your scenes!'
    })
    expect(summarize({ id: '4', type: 'tip_received', address: '0x', timestamp: '0', read: false, metadata: { senderAddress: sender } } as never).body).toBe(
      '0xabab…abab sent you a MANA tip for one of your scenes!'
    )
  })
})

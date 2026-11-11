import { describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProfileCardPresentation } from '../features/chat/ProfileCardPresentation'
import { pending } from '../lib/analytics'
import { enterAsGuest, renderSession } from './harness'

const ADDR = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const ALICE = { address: '0xalice', name: 'Alice' }

function renderCard(props: Partial<React.ComponentProps<typeof ProfileCardPresentation>> = {}): { onClose: () => void } {
  const onClose = vi.fn()
  render(<ProfileCardPresentation user={ALICE} x={20} y={20} me={{ address: '0xme' }} onClose={onClose} {...props} />)
  return { onClose }
}

// DOMAIN: nearby-mutes — saved mutes for nearby voice: the profile menu's Hush / Hear, backed by
// the engine's per-account list.
describe('saved mutes for nearby voice', () => {
  it("follows the engine's list, in any case", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    expect(h.session().nearbyMutes.muted.size).toBe(0)
    act(() => h.driver.emit({ kind: 'nearbyMutes', addresses: [ADDR.toUpperCase().replace('0X', '0x')] }))
    expect(h.session().nearbyMutes.muted.has(ADDR)).toBe(true)
    // the list is whole on every change: an account change empties it
    act(() => h.driver.emit({ kind: 'nearbyMutes', addresses: [] }))
    expect(h.session().nearbyMutes.muted.size).toBe(0)
  })

  it('a mute shows at once and asks the engine', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().nearbyMutes.set(ADDR.toUpperCase().replace('0X', '0x'), true))
    expect(h.session().nearbyMutes.muted.has(ADDR)).toBe(true)
    expect(h.driver.last('setNearbyMute')).toEqual({ kind: 'setNearbyMute', address: ADDR, muted: true })
    act(() => h.session().nearbyMutes.set(ADDR, false))
    expect(h.session().nearbyMutes.muted.has(ADDR)).toBe(false)
    expect(h.driver.last('setNearbyMute')).toEqual({ kind: 'setNearbyMute', address: ADDR, muted: false })
  })

  it('reports the change once, and nothing for a state the list already has', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    const events = (): unknown[] =>
      pending()
        .filter((e) => e.event === 'nearby_voice_user_mute')
        .map((e) => [e.properties.wallet_id, e.properties.is_muted])
    const from = events().length
    const asked = (): number => h.driver.sent.filter((m) => m.kind === 'setNearbyMute').length
    const before = asked()
    // twice in one turn: the second sees the first
    act(() => {
      h.session().nearbyMutes.set(ADDR, true)
      h.session().nearbyMutes.set(ADDR, true)
    })
    act(() => h.session().nearbyMutes.set('0x00000000000000000000000000000000000000aa', false))
    expect(events().slice(from)).toEqual([[ADDR, true]])
    expect(asked()).toBe(before + 1)
  })

  it("the engine's list puts back a mute it refused, and the next Hush is reported again", async () => {
    const h = renderSession()
    await enterAsGuest(h)
    act(() => h.session().nearbyMutes.set(ADDR, true))
    act(() => h.driver.emit({ kind: 'nearbyMutes', addresses: [] }))
    expect(h.session().nearbyMutes.muted.has(ADDR)).toBe(false)
    act(() => h.session().nearbyMutes.set(ADDR, true))
    expect(h.session().nearbyMutes.muted.has(ADDR)).toBe(true)
    expect(h.driver.last('setNearbyMute')).toEqual({ kind: 'setNearbyMute', address: ADDR, muted: true })
  })

  it('the menu offers Hush after Call, and closes on it', async () => {
    const onSetHushed = vi.fn()
    const { onClose } = renderCard({ onCall: vi.fn(), onMention: vi.fn(), onSetHushed })
    const rows = screen.getAllByRole('button').map((b) => b.textContent)
    expect(rows.indexOf('Hush')).toBe(rows.indexOf('Call') + 1)
    expect(screen.queryByRole('button', { name: 'Hear' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Hush' }))
    expect(onClose).toHaveBeenCalled()
    expect(onSetHushed).toHaveBeenCalledWith(expect.objectContaining({ address: '0xalice' }), true)
  })

  it('a hushed user gets Hear instead', async () => {
    const onSetHushed = vi.fn()
    renderCard({ hushed: true, onSetHushed })
    expect(screen.queryByRole('button', { name: 'Hush' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Hear' }))
    expect(onSetHushed).toHaveBeenCalledWith(expect.objectContaining({ address: '0xalice' }), false)
  })

  it('a blocked user can still be hushed', () => {
    renderCard({ relationship: 'blocked', onSetHushed: vi.fn() })
    expect(screen.getByRole('button', { name: 'Hush' })).toBeInTheDocument()
  })

  it('the own card and the other menus offer neither', () => {
    const onSetHushed = vi.fn()
    const { unmount } = render(<ProfileCardPresentation user={ALICE} x={0} y={0} me={{ address: '0xALICE' }} onSetHushed={onSetHushed} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Hush' })).toBeNull()
    unmount()
    for (const context of ['request', 'blocked', 'community'] as const) {
      const card = render(<ProfileCardPresentation user={ALICE} x={0} y={0} me={{ address: '0xme' }} context={context} onViewProfile={vi.fn()} onSetHushed={onSetHushed} onClose={vi.fn()} />)
      expect(screen.queryByRole('button', { name: 'Hush' })).toBeNull()
      card.unmount()
    }
  })
})

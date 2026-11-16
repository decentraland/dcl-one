// Where Gift is offered (unity-explorer's user menus): the profile card of chat names, nametags and the friend row,
// and the community card's member menu (greyed for a blocked member); not the friend-request or blocked lists' menus,
// nor your own card. The passport's options menu offers it too.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { Profile } from '@explorer-core/engine/protocol'
import { ProfileCardPresentation, type MenuContext } from '../../features/chat/ProfileCardPresentation'
import { ProfilePassport } from '../../features/profile/ProfilePassport'

const ALICE = { address: '0xa11ce00000000000000000000000000000000001', name: 'Alice' }

function card(props: { context?: MenuContext; relationship?: 'none' | 'blocked'; blockedBy?: boolean; me?: string; onGift?: () => void } = {}): ReturnType<typeof vi.fn> {
  const onGift = vi.fn()
  render(
    <ProfileCardPresentation
      user={ALICE}
      x={0}
      y={0}
      me={{ address: props.me ?? '0xme' }}
      context={props.context}
      relationship={props.relationship ?? 'none'}
      blockedBy={props.blockedBy}
      onViewProfile={vi.fn()}
      onGift={props.onGift === undefined ? onGift : props.onGift}
      onClose={vi.fn()}
    />
  )
  return onGift
}

describe('Gift in the profile card', () => {
  it('opens the picker for the user', () => {
    const onGift = card()
    fireEvent.click(screen.getByRole('button', { name: 'Gift' }))
    expect(onGift).toHaveBeenCalledWith(expect.objectContaining({ address: ALICE.address }))
  })

  it('is greyed in the member menu for a blocked member, either way', () => {
    const onGift = card({ context: 'community', blockedBy: true })
    const gift = screen.getByRole('button', { name: 'Gift' })
    expect(gift.getAttribute('aria-disabled')).toBe('true')
    fireEvent.click(gift)
    expect(onGift).not.toHaveBeenCalled()
  })

  it("isn't offered in the request or blocked lists' menus, on your own card, or where the session can't gift", () => {
    for (const context of ['request', 'blocked'] as const) {
      const { unmount } = render(<ProfileCardPresentation user={ALICE} x={0} y={0} me={{ address: '0xme' }} context={context} onViewProfile={vi.fn()} onGift={vi.fn()} onClose={vi.fn()} />)
      expect(screen.queryByRole('button', { name: 'Gift' })).toBeNull()
      unmount()
    }
    const own = render(<ProfileCardPresentation user={ALICE} x={0} y={0} me={{ address: ALICE.address }} onViewProfile={vi.fn()} onGift={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Gift' })).toBeNull()
    own.unmount()
    render(<ProfileCardPresentation user={ALICE} x={0} y={0} me={{ address: '0xme' }} onViewProfile={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByRole('button', { name: 'Gift' })).toBeNull()
  })
})

describe('Gift in the passport', () => {
  const profile: Profile = { address: ALICE.address, name: 'Alice', hasClaimedName: false, isGuest: false }

  it("is in another player's options menu, before Block", () => {
    const onGift = vi.fn()
    render(<ProfilePassport profile={profile} relationship="none" onGift={onGift} onBlock={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /options|more/i }))
    const items = screen.getAllByRole('menuitem').map((m) => m.textContent)
    expect(items.indexOf('Gift')).toBeLessThan(items.indexOf('Block'))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Gift' }))
    expect(onGift).toHaveBeenCalledWith(ALICE.address)
  })
})

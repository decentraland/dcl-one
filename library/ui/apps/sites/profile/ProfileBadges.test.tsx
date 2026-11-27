// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { ProfileBadges, type ProfileBadgeItem } from './ProfileBadges'

afterEach(cleanup)

const BADGES: readonly ProfileBadgeItem[] = [
  {
    id: 'walker',
    name: 'Walker',
    imageUrl: 'https://example.com/walker.png',
    description: 'Walk 10 km',
    unlocked: 'Unlocked on Mar 3, 2026'
  },
  { id: 'explorer', name: 'Explorer', tierName: 'Gold', description: '100 scenes' }
]

describe('ProfileBadges', () => {
  let user: ReturnType<typeof userEvent.setup>

  beforeEach(() => {
    user = userEvent.setup()
  })

  describe('when the member has badges', () => {
    beforeEach(() => {
      render(<ProfileBadges badges={BADGES} loading={false} emptyText="No badges yet." />)
    })

    it('should list each badge with no tip open', () => {
      expect(screen.getByRole('img', { name: 'Walker' })).toBeTruthy()
      expect(screen.getByRole('img', { name: 'Explorer' }).textContent).toBe('E')
      expect(screen.queryByRole('tooltip')).toBeNull()
      expect(screen.getByRole('status').textContent).toBe('2 badges')
    })

    it('should open the tip on hover with the name, blurb and date, and close it when the pointer leaves', async () => {
      const slot = screen.getByRole('img', { name: 'Walker' }).parentElement!
      await user.hover(slot)
      const tip = screen.getByRole('tooltip')
      expect(tip.textContent).toBe('WalkerWalk 10 kmUnlocked on Mar 3, 2026')
      // 224px of text plus the 8px padding on each side keeps the 240px tip.
      expect(tip.style.getPropertyValue('--tt-max-width')).toBe('224px')
      await user.unhover(slot)
      expect(screen.queryByRole('tooltip')).toBeNull()
    })

    it('should open the tip on keyboard focus, describe the focused slot with it and close it on Escape', async () => {
      await user.tab()
      await user.tab()
      const slot = screen.getByRole('img', { name: 'Explorer' }).parentElement!
      expect(document.activeElement).toBe(slot)
      const tip = screen.getByRole('tooltip')
      expect(tip.textContent).toBe('ExplorerGold100 scenes')
      expect(slot.getAttribute('aria-describedby')).toBe(tip.id)

      await user.keyboard('{Escape}')
      expect(screen.queryByRole('tooltip')).toBeNull()
      expect(slot.hasAttribute('aria-describedby')).toBe(false)
    })
  })

  describe('when the badges are loading', () => {
    beforeEach(() => {
      render(<ProfileBadges badges={BADGES} loading emptyText="No badges yet." />)
    })

    it('should show the loading status and no badge', () => {
      expect(screen.getByRole('status').textContent).toBe('Loading badges…')
      expect(screen.queryByRole('list')).toBeNull()
    })
  })

  describe('when the badges could not be read', () => {
    beforeEach(() => {
      render(
        <ProfileBadges
          badges={[]}
          loading={false}
          emptyText="No badges yet."
          errorText="Badges are unavailable."
        />
      )
    })

    it('should show the error instead of the empty text', () => {
      expect(screen.getByRole('status').textContent).toBe('Badges are unavailable.')
      expect(screen.queryByRole('tooltip')).toBeNull()
    })
  })
})

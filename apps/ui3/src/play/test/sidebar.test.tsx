// /play's own Sidebar behaviour: its Shop is the in-world Shop. The shared Sidebar tests are
// library/ui/apps/hud/test/hosted/sidebar.test.tsx.
import { describe, it, expect } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '@hud/features/sidebar/Sidebar'
import { fakeSession } from '@hud/test/harness'

// Unity parity for the nav rail (unity-explorer DCL/UI/Sidebar/SidebarView.cs).
describe('Sidebar on /play', () => {
  it('Shop sits between Discover and System and opens its panel, whose EXPLORE WEARABLES opens the in-world Shop', async () => {
    const session = fakeSession()
    render(<Sidebar session={session} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Discover'), labels.indexOf('System') + 1)).toEqual(['Discover', 'Shop', 'System'])
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(screen.getByRole('button', { name: 'Shop' })).toHaveAttribute('aria-pressed', 'true')
    expect(session.shop.toggle).not.toHaveBeenCalled()
    await userEvent.click(within(screen.getByRole('dialog', { name: 'Shop' })).getByRole('button', { name: 'EXPLORE WEARABLES' }))
    expect(session.shop.toggle).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('dialog', { name: 'Shop' })).toBeNull()
  })

})

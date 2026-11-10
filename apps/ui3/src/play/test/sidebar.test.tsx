// /play's own Sidebar behaviour: its Shop is the in-world Shop. The shared Sidebar tests are
// library/ui/apps/hud/test/hosted/sidebar.test.tsx.
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Sidebar } from '@hud/features/sidebar/Sidebar'
import { fakeSession } from '@hud/test/harness'

// Unity parity for the nav rail (unity-explorer DCL/UI/Sidebar/SidebarView.cs).
describe('Sidebar on /play', () => {
  it('Shop sits between Backpack and Camera and opens the in-world Shop', async () => {
    const session = fakeSession()
    render(<Sidebar session={session} />)
    const labels = screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))
    expect(labels.slice(labels.indexOf('Backpack'), labels.indexOf('Camera') + 1)).toEqual(['Backpack', 'Shop', 'Camera'])
    await userEvent.click(screen.getByRole('button', { name: 'Shop' }))
    expect(session.shop.toggle).toHaveBeenCalled()
  })

})

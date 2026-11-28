import { describe, it, expect } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { Tabs } from '../design'

// The HUD's tab strip is the shared TabsBar and Tabs (library/ui/components, tested there). The HUD sits over the
// game: a key press after a click is the game's, so a tab a pointer pressed keeps no focus for Chrome to ring.
function Strip(): React.JSX.Element {
  const [value, setValue] = useState('passport')
  return (
    <Tabs
      aria-label="Pages"
      items={[
        { id: 'passport', label: 'Passport' },
        { id: 'backpack', label: 'Backpack' },
        { id: 'badges', label: 'Badges' }
      ]}
      value={value}
      onChange={setValue}
    />
  )
}

describe('Tabs', () => {
  it('selects a tab the pointer clicks and leaves no focus on it, so a later key press rings nothing', async () => {
    render(<Strip />)
    const backpack = screen.getByRole('tab', { name: 'Backpack' })
    await userEvent.click(backpack)
    expect(backpack).toHaveAttribute('aria-selected', 'true')
    expect(backpack).not.toHaveFocus()
    expect(document.body).toHaveFocus()
    await userEvent.keyboard('w')
    expect(document.body).toHaveFocus()
    expect(backpack).toHaveAttribute('aria-selected', 'true')
  })

  it('keeps the focus on the tab the keyboard reaches with Tab or the arrows, or presses (a click with no count)', async () => {
    render(<Strip />)
    await userEvent.tab()
    const passport = screen.getByRole('tab', { name: 'Passport' })
    expect(passport).toHaveFocus()
    await userEvent.keyboard('{ArrowRight}')
    const backpack = screen.getByRole('tab', { name: 'Backpack' })
    expect(backpack).toHaveFocus()
    expect(backpack).toHaveAttribute('aria-selected', 'true')
    await userEvent.keyboard('{End}')
    const badges = screen.getByRole('tab', { name: 'Badges' })
    expect(badges).toHaveFocus()
    expect(badges).toHaveAttribute('aria-selected', 'true')
    // Enter and Space click the focused tab with no click count (`detail` 0)
    passport.focus()
    fireEvent.click(passport, { detail: 0 })
    expect(passport).toHaveFocus()
    expect(passport).toHaveAttribute('aria-selected', 'true')
  })
})

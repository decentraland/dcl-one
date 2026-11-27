// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { HelpTabs, type HelpTab } from './HelpTabs'

afterEach(cleanup)

type Id = 'faq' | 'fund' | 'updates'

const TABS: readonly HelpTab<Id>[] = [
  { id: 'faq', label: 'FAQ', icon: 'faq' },
  { id: 'fund', label: 'CREATOR FUND', icon: 'creatorFund' },
  { id: 'updates', label: 'SUPPORT UPDATES', icon: 'supportUpdates' }
]

function Harness({ onChange }: { onChange: (id: Id) => void }) {
  const [active, setActive] = useState<Id>('faq')
  return (
    <HelpTabs
      tabs={TABS}
      active={active}
      onChange={(id) => {
        setActive(id)
        onChange(id)
      }}
      label="Help Center"
      tabId={(id) => `tab-${id}`}
      panelId="panel"
    />
  )
}

describe('HelpTabs', () => {
  let user: ReturnType<typeof userEvent.setup>
  let onChange: ReturnType<typeof vi.fn<(id: Id) => void>>

  beforeEach(() => {
    user = userEvent.setup()
    onChange = vi.fn<(id: Id) => void>()
    render(<Harness onChange={onChange} />)
  })

  describe('when it renders', () => {
    it('should name the tab list and mark only the active tab selected and in the tab order', () => {
      expect(screen.getByRole('tablist', { name: 'Help Center' })).toBeTruthy()
      const tabs = screen.getAllByRole('tab')
      expect(tabs.map((tab) => tab.textContent)).toEqual(['FAQ', 'CREATOR FUND', 'SUPPORT UPDATES'])
      expect(tabs.map((tab) => tab.getAttribute('aria-selected'))).toEqual([
        'true',
        'false',
        'false'
      ])
      expect(tabs.map((tab) => tab.tabIndex)).toEqual([0, -1, -1])
    })

    it('should give each tab its id and point it at the panel', () => {
      const tab = screen.getByRole('tab', { name: 'CREATOR FUND' })
      expect(tab.id).toBe('tab-fund')
      expect(tab.getAttribute('aria-controls')).toBe('panel')
    })
  })

  describe('when a tab is clicked', () => {
    it('should select it', async () => {
      await user.click(screen.getByRole('tab', { name: 'SUPPORT UPDATES' }))
      expect(onChange).toHaveBeenCalledWith('updates')
      expect(
        screen.getByRole('tab', { name: 'SUPPORT UPDATES' }).getAttribute('aria-selected')
      ).toBe('true')
    })
  })

  describe('when it is operated from the keyboard', () => {
    beforeEach(async () => {
      await user.tab()
    })

    it('should move focus and the selection with the arrows, Home and End', async () => {
      await user.keyboard('{ArrowRight}')
      expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'CREATOR FUND' }))
      expect(onChange).toHaveBeenLastCalledWith('fund')
      await user.keyboard('{End}')
      expect(onChange).toHaveBeenLastCalledWith('updates')
      await user.keyboard('{Home}')
      expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'FAQ' }))
      expect(onChange).toHaveBeenLastCalledWith('faq')
    })

    it('should wrap from the first tab to the last with the left arrow', async () => {
      await user.keyboard('{ArrowLeft}')
      expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'SUPPORT UPDATES' }))
      expect(onChange).toHaveBeenLastCalledWith('updates')
    })

    it('should ignore keys the tab list does not handle', async () => {
      await user.keyboard('{ArrowDown}')
      expect(onChange).not.toHaveBeenCalled()
      expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'FAQ' }))
    })
  })
})

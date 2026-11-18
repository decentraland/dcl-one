import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Tabs } from '../components/Tabs/Tabs'
import { TabsBar } from '../components/TabsBar/TabsBar'
const NavigationTab = Tabs
const NavigationBar = TabsBar
const items = [
  { id: 'a', label: 'Alpha', href: '/alpha' },
  { id: 'b', label: 'Beta', href: '/beta' }
]
describe('when preserving the panel contract', () => {
  it('should keep native panel tabs and arrow activation', () => {
    const select = vi.fn()
    render(
      <TabsBar label="Panels">
        <Tabs label="First" selected onClick={select} />
        <Tabs label="Second" />
      </TabsBar>
    )
    const second = screen.getByRole('tab', { name: 'Second' })
    second.focus()
    fireEvent.keyDown(second, { key: 'Home' })
    expect(screen.getByRole('tab', { name: 'First' })).toHaveFocus()
    expect(select).toHaveBeenCalledOnce()
  })
  it('should preserve href panel tabs without guessing route intent', () => {
    render(<Tabs label="Panel link" href="#panel" selected />)
    expect(screen.getByRole('tab')).toHaveAttribute('aria-selected', 'true')
  })
})
describe('when explicitly navigating routes', () => {
  it('should expose a normal current-page anchor without panel tab semantics', () => {
    render(<NavigationTab mode="navigation" label="Route" href="/route" selected target="_blank" />)
    expect(screen.getByRole('link')).toHaveAttribute('aria-current', 'page')
    expect(screen.queryByRole('tab')).toBeNull()
    expect(screen.getByRole('link')).not.toHaveAttribute('tabindex', '-1')
    expect(screen.getByRole('link')).toHaveAttribute('rel', 'noopener noreferrer')
  })
  it('should render controlled route items and app-owned controls', () => {
    render(
      <NavigationBar
        mode="navigation"
        label="Store"
        page="shop"
        items={items}
        activeId="b"
        search={<input aria-label="Search" />}
        actions={<button>Filter</button>}
      />
    )
    expect(screen.getByRole('navigation')).toHaveAccessibleName('Store')
    expect(screen.getByRole('link', { name: 'Beta' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('textbox')).toHaveAccessibleName('Search')
    expect(screen.queryByRole('tablist')).toBeNull()
  })
  it('should preserve native modifiers and claimed clicks', () => {
    const select = vi.fn()
    render(<NavigationBar mode="navigation" items={items} activeId="a" onSelect={select} />)
    const link = screen.getByRole('link', { name: 'Beta' })
    fireEvent.click(link, { ctrlKey: true })
    expect(select).not.toHaveBeenCalled()
    fireEvent.click(link)
    expect(select).toHaveBeenCalledWith('b')
  })
})

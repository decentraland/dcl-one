import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BrowseControl, BrowseLoading, BrowsePanel, BrowseToolbar } from './BrowsePage'

describe('BrowseToolbar', () => {
  it('writes the tabs first, then the controls in their own group', () => {
    render(
      <BrowseToolbar tabs={<nav aria-label="Sections" />}>
        <BrowseControl size="search">
          <input aria-label="Search" />
        </BrowseControl>
      </BrowseToolbar>
    )
    const tabs = screen.getByRole('navigation', { name: 'Sections' })
    const control = screen.getByRole('textbox', { name: 'Search' }).parentElement!
    expect(control).toHaveAttribute('data-size', 'search')
    expect(tabs.nextElementSibling).toBe(control.parentElement)
  })

  it('writes no controls group without controls', () => {
    render(<BrowseToolbar tabs={<nav aria-label="Sections" />} />)
    expect(screen.getByRole('navigation').parentElement!.children).toHaveLength(1)
  })
})

describe('BrowsePanel', () => {
  it('is a plain box without a label', () => {
    render(<BrowsePanel>results</BrowsePanel>)
    const panel = screen.getByText('results')
    expect(panel).not.toHaveAttribute('role')
    expect(panel).not.toHaveAttribute('aria-busy')
  })

  it('is a named region, busy while its results are stale', () => {
    render(
      <BrowsePanel label="Results" busy>
        results
      </BrowsePanel>
    )
    expect(screen.getByRole('region', { name: 'Results' })).toHaveAttribute('aria-busy', 'true')
  })
})

describe('BrowseLoading', () => {
  it('draws a spinner', () => {
    const { container } = render(<BrowseLoading />)
    expect(container.firstElementChild!.children).toHaveLength(1)
  })
})

describe('when a caller passes its own class', () => {
  it('keeps it beside the own class of the toolbar, a control and the panel', () => {
    render(
      <>
        <BrowseToolbar tabs={<nav aria-label="Sections" />} className="bar">
          <BrowseControl size="search" className="slot">
            search
          </BrowseControl>
        </BrowseToolbar>
        <BrowsePanel className="results">results</BrowsePanel>
        <BrowsePanel>plain</BrowsePanel>
      </>
    )
    const toolbar = screen.getByRole('navigation').parentElement!
    expect(toolbar).toHaveClass('bar')
    expect(toolbar.classList).toHaveLength(2)
    expect(screen.getByText('search')).toHaveClass('slot')
    expect(screen.getByText('search').classList).toHaveLength(2)
    expect(screen.getByText('results')).toHaveClass('results')
    expect(screen.getByText('plain').classList).toHaveLength(1)
  })
})

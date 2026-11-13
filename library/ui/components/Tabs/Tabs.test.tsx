import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Tabs } from './Tabs'
describe('when a captured Tabs item navigates a route', () => {
  it('should use a native link with an explicit current-page state', () => {
    render(<Tabs href="/profile" label="Overview" selected />)
    expect(screen.getByRole('link')).toHaveAttribute('href', '/profile')
    expect(screen.getByRole('link')).toHaveAttribute('aria-current', 'page')
    expect(screen.queryByRole('tab')).not.toBeInTheDocument()
  })
  it('should preserve modifier clicks and caller cancellation', () => {
    const click = vi.fn((event) => event.preventDefault())
    render(<Tabs href="/profile" label="Overview" onClick={click} />)
    const result = fireEvent.click(screen.getByRole('link'), { ctrlKey: true })
    expect(click).toHaveBeenCalledOnce()
    expect(result).toBe(false)
    expect(screen.getByRole('link')).not.toHaveAttribute('aria-current')
  })
})

it('should expose one visible label to native navigation and event consumers', () => {
  render(<Tabs href="/profile" label="Overview" />)
  const link = screen.getByRole('link', { name: 'Overview' })
  expect(link.textContent).toBe('Overview')
})

it('should preserve its owned content when untyped callers provide HTML overrides', () => {
  const override = { dangerouslySetInnerHTML: { __html: '<b>Injected content</b>' } }
  render(<Tabs href="/profile" label="Overview" {...override} />)
  expect(screen.getByRole('link')).toHaveTextContent('Overview')
  expect(screen.queryByText('Injected content')).toBeNull()
})

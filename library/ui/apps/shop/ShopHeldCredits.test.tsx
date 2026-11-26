import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ShopHeldCredits } from './ShopHeldCredits'

const held = (release: number | null) => (
  <ShopHeldCredits credits={12} purchaseCount={1} releasesAtSeconds={release}>
    12
  </ShopHeldCredits>
)

describe('ShopHeldCredits', () => {
  afterEach(() => vi.useRealTimers())

  it('should start at the current time when a reservation gains a release date', () => {
    vi.useFakeTimers()
    vi.setSystemTime(100_000)
    const view = render(held(null))
    fireEvent.focus(screen.getByRole('button'))
    expect(screen.getByRole('tooltip')).toHaveTextContent("can't say exactly when")

    vi.setSystemTime(130_000)
    view.rerender(held(160))
    expect(screen.getByRole('tooltip')).toHaveTextContent('0:30')
    act(() => vi.advanceTimersByTime(1000))
    expect(screen.getByRole('tooltip')).toHaveTextContent('0:29')
  })

  it('should dismiss the help with Escape while its trigger retains focus', () => {
    render(held(null))
    const trigger = screen.getByRole('button')
    act(() => trigger.focus())
    expect(trigger).toHaveAccessibleDescription(/network confirms/)
    fireEvent.keyDown(trigger, { key: 'Escape' })
    expect(screen.queryByRole('tooltip')).toBeNull()
    expect(trigger).toHaveFocus()
  })
})

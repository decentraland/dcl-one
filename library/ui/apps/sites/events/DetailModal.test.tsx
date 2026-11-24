import '@testing-library/jest-dom/vitest'
import userEvent from '@testing-library/user-event'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EventDetailModal } from './DetailModal'

afterEach(cleanup)

describe('when an upcoming event offers a reminder', () => {
  let onToggle: ReturnType<typeof vi.fn>
  let showReminder: (active: boolean, pending?: boolean) => React.JSX.Element

  beforeEach(() => {
    onToggle = vi.fn()
    const onClose = vi.fn()
    showReminder = (active, pending = false) => (
      <EventDetailModal
        title="Open studio"
        image={null}
        live={false}
        location={{ label: '0,0', world: false }}
        remind={{ active, pending, onToggle }}
        calendarHref="https://example.com/calendar"
        shareUrl="https://example.com/event"
        onClose={onClose}
      />
    )
  })

  it('should show the secondary-light medium variant before the reminder is set', () => {
    render(showReminder(false))
    const button = screen.getByRole('button', { name: 'Remind me', pressed: false })
    expect(button.getAttribute('data-variant')).toBe('secondaryLight')
    expect(button.getAttribute('data-size')).toBe('md')
  })

  it('should keep focus and ignore repeated activation while a reminder is saving', async () => {
    const user = userEvent.setup()
    const { rerender } = render(showReminder(false))
    const button = screen.getByRole('button', { name: 'Remind me', pressed: false })

    await user.click(button)
    rerender(showReminder(false, true))
    expect(button).toHaveFocus()
    expect(button).toHaveAttribute('aria-disabled', 'true')
    // jsdom keeps focus on native disabled controls. Chromium blurs them.
    expect(button).not.toBeDisabled()
    await user.keyboard('{Enter} ')
    await user.click(button)
    expect(onToggle).toHaveBeenCalledTimes(1)

    rerender(showReminder(true))
    const active = screen.getByRole('button', { name: 'Remind me', pressed: true })
    expect(active).toHaveFocus()
    expect(active).toHaveAttribute('data-variant', 'primary')
    await user.keyboard('{Enter}')
    expect(onToggle).toHaveBeenCalledTimes(2)
  })

  it('should align Calendar and Copy with the responsive reminder action', () => {
    render(showReminder(false))

    const calendar = screen.getByRole('link', { name: 'Add to calendar' })
    const copy = screen.getByRole('button', { name: 'Copy link' })
    expect(calendar).toHaveAttribute('href', 'https://example.com/calendar')
    expect(calendar).toHaveAttribute('target', '_blank')
    expect(calendar).toHaveAttribute('data-viewport', 'responsive')
    expect(copy).toHaveAttribute('data-viewport', 'responsive')
  })

  it('should allow keyboard activation again when a save settles without changing the reminder', async () => {
    const user = userEvent.setup()
    const { rerender } = render(showReminder(false))
    const button = screen.getByRole('button', { name: 'Remind me', pressed: false })
    await user.click(button)
    rerender(showReminder(false, true))
    rerender(showReminder(false))

    expect(button).toHaveFocus()
    expect(button).not.toHaveAttribute('aria-disabled', 'true')
    await user.keyboard(' ')
    expect(onToggle).toHaveBeenCalledTimes(2)
  })
})

describe('when an event does not offer a reminder', () => {
  beforeEach(() => {
    render(
      <EventDetailModal
        title="Open studio"
        image={null}
        live
        location={{ label: '0,0', world: false }}
        calendarHref="https://example.com/calendar"
        shareUrl="https://example.com/event"
        onClose={vi.fn()}
      />
    )
  })

  it('should keep the existing Calendar and Copy controls', () => {
    const calendar = screen.getByRole('link', { name: 'Add to calendar' })
    const copy = screen.getByRole('button', { name: 'Copy link' })
    expect(calendar).not.toHaveAttribute('data-viewport')
    expect(copy).not.toHaveAttribute('data-viewport')
  })
})

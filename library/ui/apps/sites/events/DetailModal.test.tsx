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
    expect(button).not.toBeDisabled()
    await user.keyboard('{Enter} ')
    await user.click(button)
    expect(onToggle).toHaveBeenCalledTimes(1)

    rerender(showReminder(true))
    expect(screen.getByRole('button', { name: 'Remind me', pressed: true })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(onToggle).toHaveBeenCalledTimes(2)
  })

  it('should allow a keyboard retry from the same control after a failed save', async () => {
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

import { useState } from 'react'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Toast } from './Toast'

afterEach(() => vi.useRealTimers())

describe('Toast', () => {
  describe('when rendered with a message', () => {
    it('shows the message in a status region', () => {
      render(<Toast>Link copied</Toast>)
      expect(screen.getByRole('status')).toHaveTextContent('Link copied')
      expect(screen.getByText('Link copied')).toBeVisible()
    })

    it.each(['text', 'confirmation', 'error'] as const)('keeps the status role for the %s variant', (variant) => {
      render(<Toast variant={variant}>Saved</Toast>)
      expect(screen.getByRole('status')).toHaveTextContent('Saved')
    })

    it('is an alert region when the caller asks for one', () => {
      render(<Toast role="alert" variant="error">Could not save</Toast>)
      expect(screen.getByRole('alert')).toHaveTextContent('Could not save')
      expect(screen.queryByRole('status')).toBeNull()
    })

    it('renders on the server without a browser', () => {
      expect(renderToString(<Toast placement="inline">Saved</Toast>)).toContain('role="status"')
    })
  })

  describe('when it has no message', () => {
    it('keeps the status region in the page, empty, so the next message is announced', () => {
      const { rerender } = render(<Toast />)
      const region = screen.getByRole('status')
      expect(region).toBeEmptyDOMElement()
      rerender(<Toast>Profile link copied</Toast>)
      expect(screen.getByRole('status')).toBe(region)
      expect(region).toHaveTextContent('Profile link copied')
    })
  })

  describe('when its time ends', () => {
    it('asks the caller to close it after the default four seconds', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      render(<Toast onClose={close}>Saved</Toast>)
      act(() => void vi.advanceTimersByTime(3999))
      expect(close).not.toHaveBeenCalled()
      act(() => void vi.advanceTimersByTime(1))
      expect(close).toHaveBeenCalledTimes(1)
    })

    it('takes another duration, and starts the time again for a new message', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      const { rerender } = render(
        <Toast duration={1000} onClose={close}>
          First
        </Toast>
      )
      act(() => void vi.advanceTimersByTime(900))
      rerender(
        <Toast duration={1000} onClose={close}>
          Second
        </Toast>
      )
      act(() => void vi.advanceTimersByTime(900))
      expect(close).not.toHaveBeenCalled()
      act(() => void vi.advanceTimersByTime(100))
      expect(close).toHaveBeenCalledTimes(1)
    })

    it('preserves the timer when the same rich message rerenders', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      const view = () => <Toast duration={1000} messageKey="saved" onClose={close}><strong>Saved</strong></Toast>
      const { rerender } = render(view())
      act(() => void vi.advanceTimersByTime(900))
      rerender(view())
      act(() => void vi.advanceTimersByTime(100))
      expect(close).toHaveBeenCalledTimes(1)
    })

    it('stays with a duration of 0', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      render(
        <Toast duration={0} onClose={close}>
          Saved
        </Toast>
      )
      act(() => void vi.advanceTimersByTime(60_000))
      expect(close).not.toHaveBeenCalled()
    })

    it('starts no time without a message', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      render(<Toast onClose={close} />)
      act(() => void vi.advanceTimersByTime(60_000))
      expect(close).not.toHaveBeenCalled()
    })

    it('waits while the focus is on the toast', () => {
      vi.useFakeTimers()
      const close = vi.fn()
      render(
        <Toast duration={1000} closeButton onClose={close}>
          Could not save
        </Toast>
      )
      act(() => screen.getByRole('button', { name: 'Close' }).focus())
      act(() => void vi.advanceTimersByTime(5000))
      expect(close).not.toHaveBeenCalled()
      act(() => screen.getByRole('button', { name: 'Close' }).blur())
      act(() => void vi.advanceTimersByTime(250))
      expect(close).toHaveBeenCalledTimes(1)
    })
  })

  describe('when it has a close button', () => {
    it('removes the message when the caller closes it from the button', async () => {
      const user = userEvent.setup()
      function Page() {
        const [message, setMessage] = useState<string | null>('Could not load your home scene')
        return (
          <Toast variant="error" duration={0} closeButton closeLabel="Dismiss" onClose={() => setMessage(null)}>
            {message}
          </Toast>
        )
      }
      render(<Page />)
      await user.click(screen.getByRole('button', { name: 'Dismiss' }))
      expect(screen.getByRole('status')).toBeEmptyDOMElement()
    })

    it('shows no close button without a way to close', () => {
      render(<Toast closeButton>Saved</Toast>)
      expect(screen.queryByRole('button')).toBeNull()
    })
  })

  describe('when it has an action', () => {
    it('shows the action after the message', async () => {
      const user = userEvent.setup()
      const retry = vi.fn()
      render(
        <Toast
          variant="error"
          action={
            <button type="button" onClick={retry}>
              Retry
            </button>
          }
        >
          Could not save
        </Toast>
      )
      await user.click(screen.getByRole('button', { name: 'Retry' }))
      expect(retry).toHaveBeenCalledTimes(1)
    })
  })
})

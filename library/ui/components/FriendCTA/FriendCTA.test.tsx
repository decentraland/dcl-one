import { createRef } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { FriendCTA } from './FriendCTA'

describe('FriendCTA', () => {
  describe('when rendered for a friendship state', () => {
    it.each([
      ['add', 'Add friend'],
      ['sent', 'Request sent Cancel request'],
      ['friend', 'Friend Unfriend'],
      ['reject', 'Reject'],
      ['blocked', 'Blocked Unblock']
    ] as const)('names the %s button with its state and its action', (status, name) => {
      render(<FriendCTA status={status} />)
      expect(screen.getByRole('button', { name })).toHaveAttribute('type', 'button')
    })

    it('takes the texts of another language', () => {
      render(<FriendCTA status="sent" labels={{ sent: 'Solicitud enviada', cancel: 'Cancelar' }} />)
      expect(screen.getByRole('button', { name: 'Solicitud enviada Cancelar' })).toBeInTheDocument()
    })

    it('forwards its ref and its native attributes to the button', () => {
      const ref = createRef<HTMLButtonElement>()
      render(<FriendCTA status="add" ref={ref} data-testid="cta" />)
      expect(ref.current).toBe(screen.getByTestId('cta'))
    })

    it('renders on the server without a browser', () => {
      expect(renderToString(<FriendCTA status="friend" viewport="mobile" />)).toContain(
        'data-viewport="mobile"'
      )
    })
  })

  describe('when the reader presses it', () => {
    it('runs the action from a mouse click', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="friend" onClick={press} />)
      await user.click(screen.getByRole('button'))
      expect(press).toHaveBeenCalledTimes(1)
    })

    it('runs the action from the keyboard', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="sent" onClick={press} />)
      await user.tab()
      expect(screen.getByRole('button')).toHaveFocus()
      await user.keyboard('{Enter}')
      await user.keyboard(' ')
      expect(press).toHaveBeenCalledTimes(2)
    })

    it('shows the action on the first touch press of a state button, and runs it on the next', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="friend" onClick={press} />)
      const button = screen.getByRole('button')
      await user.pointer({ keys: '[TouchA]', target: button })
      expect(press).not.toHaveBeenCalled()
      expect(button).toHaveAttribute('data-armed')
      await user.pointer({ keys: '[TouchA]', target: button })
      expect(press).toHaveBeenCalledTimes(1)
      expect(button).not.toHaveAttribute('data-armed')
    })

    it('shows the state again when the focus leaves before the second touch press', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(
        <>
          <FriendCTA status="blocked" onClick={press} />
          <button type="button">Other</button>
        </>
      )
      const button = screen.getByRole('button', { name: 'Blocked Unblock' })
      await user.pointer({ keys: '[TouchA]', target: button })
      await user.click(screen.getByRole('button', { name: 'Other' }))
      expect(button).not.toHaveAttribute('data-armed')
      expect(press).not.toHaveBeenCalled()
    })

    it('runs an action button on the first touch press', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="add" onClick={press} />)
      await user.pointer({ keys: '[TouchA]', target: screen.getByRole('button') })
      expect(press).toHaveBeenCalledTimes(1)
    })
  })

  describe('when its write is in progress', () => {
    it('is busy, keeps the focus and its name, and ignores a press', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="sent" loading onClick={press} />)
      const button = screen.getByRole('button', { name: 'Request sent Cancel request' })
      expect(button).toHaveAttribute('aria-busy', 'true')
      expect(button).toHaveAttribute('aria-disabled', 'true')
      await user.click(button)
      expect(button).toHaveFocus()
      expect(press).not.toHaveBeenCalled()
    })
  })

  describe('when disabled', () => {
    it('is a disabled native button that takes no press', async () => {
      const user = userEvent.setup()
      const press = vi.fn()
      render(<FriendCTA status="add" disabled onClick={press} />)
      const button = screen.getByRole('button', { name: 'Add friend' })
      expect(button).toBeDisabled()
      await user.click(button)
      expect(press).not.toHaveBeenCalled()
    })
  })
})

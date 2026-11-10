import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { Modal } from './Modal'

describe('Modal', () => {
  describe('when rendered with a title and content', () => {
    it('should render a modal dialog named by its title', () => {
      render(<Modal title="Buy Item">Body</Modal>)

      const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
      expect(dialog).toHaveAttribute('aria-modal', 'true')
      expect(screen.getByRole('heading', { level: 2, name: 'Buy Item' })).toBeInTheDocument()
      expect(dialog).toHaveTextContent('Body')
    })

    it('should move focus into the dialog', () => {
      render(<Modal title="Buy Item">Body</Modal>)

      expect(screen.getByRole('dialog', { name: 'Buy Item' })).toHaveFocus()
    })
  })

  describe('when no onClose is given', () => {
    it('should render no close button and ignore Escape', async () => {
      const user = userEvent.setup()
      render(<Modal title="Buy Item" />)

      expect(screen.queryByRole('button')).not.toBeInTheDocument()
      await user.keyboard('{Escape}')
      expect(screen.getByRole('dialog', { name: 'Buy Item' })).toBeInTheDocument()
    })
  })

  describe('when onClose is given', () => {
    it('should render a close button labelled with closeLabel that calls onClose', async () => {
      const onClose = vi.fn()
      const user = userEvent.setup()
      render(<Modal title="Buy Item" onClose={onClose} closeLabel="Cerrar" />)

      await user.click(screen.getByRole('button', { name: 'Cerrar' }))
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('should call onClose when Escape is pressed inside the dialog', async () => {
      const onClose = vi.fn()
      const user = userEvent.setup()
      render(<Modal title="Buy Item" onClose={onClose} />)

      await user.keyboard('{Escape}')
      expect(onClose).toHaveBeenCalledTimes(1)
    })

    it('should put initial focus on the close button, its first focusable element', () => {
      render(<Modal title="Buy Item" onClose={() => undefined} />)

      expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
    })
  })

  describe('when it has several focusable elements', () => {
    function Dialog() {
      return (
        <>
          <button type="button">Outside</button>
          <Modal
            title="Buy Item"
            onClose={() => undefined}
            actions={
              <>
                <button type="button">Cancel</button>
                <button type="button">Buy</button>
              </>
            }
          >
            <a href="/terms">Terms</a>
          </Modal>
        </>
      )
    }

    it('should keep Tab inside the dialog, wrapping from the last element to the first', async () => {
      const user = userEvent.setup()
      render(<Dialog />)

      expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('link', { name: 'Terms' })).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('button', { name: 'Buy' })).toHaveFocus()
      await user.tab()
      expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
    })

    it('should wrap Shift+Tab from the first element to the last', async () => {
      const user = userEvent.setup()
      render(<Dialog />)

      await user.tab({ shift: true })
      expect(screen.getByRole('button', { name: 'Buy' })).toHaveFocus()
    })

    it('should pull focus back when something outside the dialog takes it', () => {
      render(<Dialog />)

      screen.getByRole('button', { name: 'Outside' }).focus()
      expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
    })
  })

  describe('when a sibling outside the dialog is focusable', () => {
    it('should never Tab out of the dialog', async () => {
      const user = userEvent.setup()
      render(
        <>
          <Modal title="Buy Item" onClose={() => undefined}>
            <button type="button">Inside</button>
          </Modal>
          <button type="button">Outside</button>
        </>
      )

      for (let press = 0; press < 3; press += 1) {
        await user.tab()
        expect(screen.getByRole('button', { name: 'Outside' })).not.toHaveFocus()
      }
      expect(screen.getByRole('button', { name: 'Inside' })).toHaveFocus()
    })
  })

  describe('when it has nothing focusable', () => {
    it('should keep focus on the dialog when Tab is pressed', async () => {
      const user = userEvent.setup()
      render(
        <>
          <button type="button">Outside</button>
          <Modal title="Notice">Body</Modal>
        </>
      )

      await user.tab()
      expect(screen.getByRole('dialog', { name: 'Notice' })).toHaveFocus()
    })
  })

  describe('when it closes', () => {
    it('should return focus to the element that opened it', async () => {
      const user = userEvent.setup()
      function Opener() {
        const [open, setOpen] = useState(false)
        return (
          <>
            <button type="button" onClick={() => setOpen(true)}>
              Open
            </button>
            {open && <Modal title="Buy Item" onClose={() => setOpen(false)} />}
          </>
        )
      }
      render(<Opener />)

      await user.click(screen.getByRole('button', { name: 'Open' }))
      expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus()
      await user.keyboard('{Escape}')
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Open' })).toHaveFocus()
    })
  })

  describe('when no subtitle or actions are given', () => {
    it('should render neither the subtitle line nor the actions footer', () => {
      render(<Modal title="Buy Item" />)

      const dialog = screen.getByRole('dialog', { name: 'Buy Item' })
      // Only the title reads in the dialog: no subtitle text, and no actions footer.
      expect(dialog).toHaveTextContent(/^Buy Item$/)
      expect(
        within(dialog)
          .queryAllByRole('button')
          .filter((button) => button.closest('footer'))
      ).toEqual([])
    })
  })

  describe('when a subtitle and actions are given', () => {
    it('should render both', () => {
      render(
        <Modal
          title="Buy Item"
          subtitle="My Credits Balance: 200"
          actions={<button type="button">Buy</button>}
        />
      )

      expect(screen.getByText('My Credits Balance: 200')).toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'Buy' }).closest('footer')).not.toBeNull()
    })
  })
  describe('when rendered inside a clipping container', () => {
    it('should render on document.body over a backdrop, outside the container', () => {
      const { container } = render(
        <div style={{ overflow: 'hidden', position: 'relative' }}>
          <Modal title="Download">QR</Modal>
        </div>
      )

      const dialog = screen.getByRole('dialog', { name: 'Download' })
      expect(container.contains(dialog)).toBe(false)
      expect(dialog.parentElement?.parentElement).toBe(document.body)
    })
  })
})

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { CheckboxLabel } from './CheckboxLabel'

describe('CheckboxLabel', () => {
  describe('when rendered with a label', () => {
    it('names the checkbox with its visible text', () => {
      render(<CheckboxLabel label="Open Debug Console" />)
      expect(screen.getByRole('checkbox', { name: 'Open Debug Console' })).not.toBeChecked()
      expect(screen.getByText('Open Debug Console')).toBeVisible()
    })

    it('puts the box before the text by default and after it at the start position', () => {
      const { container, rerender } = render(<CheckboxLabel label="Order" />)
      const order = () =>
        Array.from(container.firstElementChild!.children).map((child) =>
          child.querySelector('input') ? 'box' : 'text'
        )
      expect(container.firstElementChild).toHaveAttribute('data-position', 'end')
      expect(order()).toEqual(['box', 'text'])
      rerender(<CheckboxLabel label="Order" labelPosition="start" />)
      expect(container.firstElementChild).toHaveAttribute('data-position', 'start')
      expect(order()).toEqual(['text', 'box'])
    })

    it('passes size, variant and the mixed state to the box', () => {
      const { container } = render(
        <CheckboxLabel label="All" size="lg" variant="filled" indeterminate />
      )
      const box = container.querySelector('[data-size]')!
      expect(box).toHaveAttribute('data-size', 'lg')
      expect(box).toHaveAttribute('data-variant', 'filled')
      expect(screen.getByRole('checkbox')).toBePartiallyChecked()
    })

    it('keeps a caller id for the input and its label', () => {
      render(<CheckboxLabel id="debug" label="Debug" />)
      expect(screen.getByLabelText('Debug')).toHaveAttribute('id', 'debug')
    })

    it('gives two instances distinct ids', () => {
      render(
        <>
          <CheckboxLabel label="First" />
          <CheckboxLabel label="Second" />
        </>
      )
      const [first, second] = screen.getAllByRole('checkbox')
      expect(first!.id).not.toBe(second!.id)
      expect(screen.getByRole('checkbox', { name: 'Second' })).toBe(second)
    })

    it('renders on the server without a browser', () => {
      const html = renderToString(<CheckboxLabel label="Server" help="Explained" />)
      expect(html).toContain('type="checkbox"')
      expect(html).toContain('Server')
    })
  })

  describe('when it has help', () => {
    it('describes the checkbox without changing its name', () => {
      render(<CheckboxLabel label="Open Debug Console" help="Shows logs in a panel." />)
      const box = screen.getByRole('checkbox', { name: 'Open Debug Console' })
      expect(box).toHaveAccessibleDescription('Shows logs in a panel.')
    })

    it('keeps a caller description before the help', () => {
      render(
        <>
          <p id="extra">Restart required.</p>
          <CheckboxLabel label="Beta" help="Unstable." aria-describedby="extra" />
        </>
      )
      expect(screen.getByRole('checkbox')).toHaveAccessibleDescription(
        'Restart required. Unstable.'
      )
    })

    it('shows no help icon or description without help', () => {
      render(<CheckboxLabel label="Plain" />)
      expect(screen.queryByRole('button')).toBeNull()
      expect(screen.getByRole('checkbox')).not.toHaveAttribute('aria-describedby')
    })

    it('does not toggle the box when the help button is clicked', async () => {
      const user = userEvent.setup()
      render(<CheckboxLabel label="Help" help="Info" />)
      await user.click(screen.getByRole('button', { name: 'More information' }))
      expect(screen.getByRole('checkbox')).not.toBeChecked()
    })

    it('names the help button with helpLabel', () => {
      render(<CheckboxLabel label="Help" help="Info" helpLabel="Más información" />)
      expect(screen.getByRole('button', { name: 'Más información' })).toBeInTheDocument()
    })
  })

  describe('when the reader opens the help', () => {
    const setup = () => {
      const user = userEvent.setup()
      render(
        <>
          <CheckboxLabel label="Open Debug Console" help="Shows engine logs in a panel." />
          <button type="button">Elsewhere</button>
        </>
      )
      const button = screen.getByRole('button', { name: 'More information' })
      return { user, button }
    }
    const isOpen = (button: HTMLElement) => button.getAttribute('aria-expanded') === 'true'
    const visibleText = () =>
      document.querySelector<HTMLElement>('[id$="-bubble"]')!

    it('starts closed with the text hidden', () => {
      const { button } = setup()
      expect(isOpen(button)).toBe(false)
      expect(visibleText()).not.toBeVisible()
      expect(button).toHaveAttribute('aria-controls', visibleText().id)
    })

    it.each(['{Enter}', ' '])('opens and closes from the keyboard with %s', async (key) => {
      const { user, button } = setup()
      button.focus()
      await user.keyboard(key)
      expect(isOpen(button)).toBe(true)
      expect(visibleText()).toBeVisible()
      await user.keyboard(key)
      expect(isOpen(button)).toBe(false)
      expect(visibleText()).not.toBeVisible()
    })

    it('closes on Escape and keeps focus on the button', async () => {
      const { user, button } = setup()
      await user.click(button)
      await user.keyboard('{Escape}')
      expect(isOpen(button)).toBe(false)
      expect(button).toHaveFocus()
    })

    it('keeps the Escape that closes it from reaching the page, and passes it on when closed', async () => {
      const user = userEvent.setup()
      const page = vi.fn()
      render(
        <div onKeyDown={(event) => page(event.key, event.defaultPrevented)}>
          <CheckboxLabel label="Nested" help="Nested help." />
        </div>
      )
      const button = screen.getByRole('button', { name: 'More information' })
      await user.click(button)
      await user.keyboard('{Escape}')
      expect(page).not.toHaveBeenCalled()
      expect(button).toHaveAttribute('aria-expanded', 'false')
      await user.keyboard('{Escape}')
      expect(page).toHaveBeenCalledWith('Escape', false)
    })

    it('closes when focus leaves it', async () => {
      const { user, button } = setup()
      await user.click(button)
      await user.tab()
      expect(screen.getByRole('button', { name: 'Elsewhere' })).toHaveFocus()
      expect(isOpen(button)).toBe(false)
    })

    it('opens on a tap and closes on a tap outside', async () => {
      const { user, button } = setup()
      await user.pointer({ keys: '[TouchA]', target: button })
      expect(isOpen(button)).toBe(true)
      expect(visibleText()).toBeVisible()
      await user.pointer({ keys: '[TouchA]', target: document.body })
      expect(isOpen(button)).toBe(false)
    })

    it('stays open when its own text is tapped', async () => {
      const { user, button } = setup()
      await user.click(button)
      await user.pointer({ keys: '[TouchA]', target: visibleText() })
      expect(isOpen(button)).toBe(true)
    })

    it('keeps the checkbox name and description while open', async () => {
      const { user, button } = setup()
      await user.click(button)
      const box = screen.getByRole('checkbox', { name: 'Open Debug Console' })
      expect(box).toHaveAccessibleDescription('Shows engine logs in a panel.')
      expect(box).not.toBeChecked()
    })
  })

  describe('when the reader toggles it', () => {
    it('toggles from its visible text and from Space', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<CheckboxLabel label="Sounds" onCheckedChange={checked} />)
      await user.click(screen.getByText('Sounds'))
      expect(checked).toHaveBeenLastCalledWith(true)
      expect(screen.getByRole('checkbox')).toHaveFocus()
      await user.keyboard(' ')
      expect(checked).toHaveBeenLastCalledWith(false)
    })
  })

  describe('when disabled', () => {
    it('ignores clicks on the text and marks the row disabled', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      const { container } = render(<CheckboxLabel label="Locked" disabled onCheckedChange={checked} />)
      await user.click(screen.getByText('Locked'))
      expect(checked).not.toHaveBeenCalled()
      expect(screen.getByRole('checkbox', { name: 'Locked' })).toBeDisabled()
      expect(container.firstElementChild).toHaveAttribute('data-disabled', 'true')
    })
  })

  describe('when inside a form', () => {
    it('submits its name and value only while checked', async () => {
      const user = userEvent.setup()
      render(
        <form aria-label="settings">
          <CheckboxLabel label="Debug" name="debug" />
        </form>
      )
      const form = screen.getByRole<HTMLFormElement>('form')
      expect(new FormData(form).has('debug')).toBe(false)
      await user.click(screen.getByText('Debug'))
      expect(new FormData(form).get('debug')).toBe('on')
    })
  })
})

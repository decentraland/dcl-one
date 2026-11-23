import { render, type RenderResult, screen } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { CheckboxLabel } from './CheckboxLabel'

describe('CheckboxLabel', () => {
  let user: UserEvent

  beforeEach(() => {
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when rendered with a label', () => {
    describe('and no other props', () => {
      beforeEach(() => {
        render(<CheckboxLabel label="Open Debug Console" />)
      })

      it('should name the checkbox with its visible text', () => {
        expect(screen.getByRole('checkbox', { name: 'Open Debug Console' })).not.toBeChecked()
        expect(screen.getByText('Open Debug Console')).toBeVisible()
      })
    })

    describe('and the label position changes', () => {
      let rendered: RenderResult
      let order: () => string[]

      beforeEach(() => {
        rendered = render(<CheckboxLabel label="Order" />)
        order = () =>
          Array.from(rendered.container.firstElementChild!.children).map((child) =>
            child.querySelector('input') ? 'box' : 'text'
          )
      })

      it('should put the box before the text by default and after it at the start position', () => {
        const { container, rerender } = rendered
        expect(container.firstElementChild).toHaveAttribute('data-position', 'end')
        expect(order()).toEqual(['box', 'text'])
        rerender(<CheckboxLabel label="Order" labelPosition="start" />)
        expect(container.firstElementChild).toHaveAttribute('data-position', 'start')
        expect(order()).toEqual(['text', 'box'])
      })
    })

    describe('and it has a size, a variant and the mixed state', () => {
      let container: HTMLElement

      beforeEach(() => {
        container = render(
          <CheckboxLabel label="All" size="lg" variant="filled" indeterminate />
        ).container
      })

      it('should pass size, variant and the mixed state to the box', () => {
        const box = container.querySelector('[data-size]')!
        expect(box).toHaveAttribute('data-size', 'lg')
        expect(box).toHaveAttribute('data-variant', 'filled')
        expect(screen.getByRole('checkbox')).toBePartiallyChecked()
      })
    })

    describe('and the caller sets an id', () => {
      beforeEach(() => {
        render(<CheckboxLabel id="debug" label="Debug" />)
      })

      it('should keep the caller id for the input and its label', () => {
        expect(screen.getByLabelText('Debug')).toHaveAttribute('id', 'debug')
      })
    })

    describe('and two instances render together', () => {
      beforeEach(() => {
        render(
          <>
            <CheckboxLabel label="First" />
            <CheckboxLabel label="Second" />
          </>
        )
      })

      it('should give each instance a distinct id', () => {
        const [first, second] = screen.getAllByRole('checkbox')
        expect(first!.id).not.toBe(second!.id)
        expect(screen.getByRole('checkbox', { name: 'Second' })).toBe(second)
      })
    })

    describe('and it renders on the server', () => {
      let html: string

      beforeEach(() => {
        html = renderToString(<CheckboxLabel label="Server" help="Explained" />)
      })

      it('should render without a browser', () => {
        expect(html).toContain('type="checkbox"')
        expect(html).toContain('Server')
      })
    })
  })

  describe('when it has help', () => {
    describe('and no caller description', () => {
      beforeEach(() => {
        render(<CheckboxLabel label="Open Debug Console" help="Shows logs in a panel." />)
      })

      it('should describe the checkbox without changing its name', () => {
        const box = screen.getByRole('checkbox', { name: 'Open Debug Console' })
        expect(box).toHaveAccessibleDescription('Shows logs in a panel.')
      })
    })

    describe('and a caller description', () => {
      beforeEach(() => {
        render(
          <>
            <p id="extra">Restart required.</p>
            <CheckboxLabel label="Beta" help="Unstable." aria-describedby="extra" />
          </>
        )
      })

      it('should keep the caller description before the help', () => {
        expect(screen.getByRole('checkbox')).toHaveAccessibleDescription(
          'Restart required. Unstable.'
        )
      })
    })

    describe('and the help button is clicked', () => {
      beforeEach(() => {
        render(<CheckboxLabel label="Help" help="Info" />)
      })

      it('should not toggle the box', async () => {
        await user.click(screen.getByRole('button', { name: 'More information' }))
        expect(screen.getByRole('checkbox')).not.toBeChecked()
      })
    })

    describe('and a helpLabel', () => {
      beforeEach(() => {
        render(<CheckboxLabel label="Help" help="Info" helpLabel="Más información" />)
      })

      it('should name the help button with helpLabel', () => {
        expect(screen.getByRole('button', { name: 'Más información' })).toBeInTheDocument()
      })
    })
  })

  describe('when it has no help', () => {
    beforeEach(() => {
      render(<CheckboxLabel label="Plain" />)
    })

    it('should show no help icon or description', () => {
      expect(screen.queryByRole('button')).toBeNull()
      expect(screen.getByRole('checkbox')).not.toHaveAttribute('aria-describedby')
    })
  })

  describe('when the reader opens the help', () => {
    const isOpen = (button: HTMLElement) => button.getAttribute('aria-expanded') === 'true'
    const visibleText = () => document.querySelector<HTMLElement>('[id$="-bubble"]')!

    describe('and the page has another focusable control', () => {
      let button: HTMLElement

      beforeEach(() => {
        render(
          <>
            <CheckboxLabel label="Open Debug Console" help="Shows engine logs in a panel." />
            <button type="button">Elsewhere</button>
          </>
        )
        button = screen.getByRole('button', { name: 'More information' })
      })

      it('should start closed with the text hidden', () => {
        expect(isOpen(button)).toBe(false)
        expect(visibleText()).not.toBeVisible()
        expect(button).toHaveAttribute('aria-controls', visibleText().id)
      })

      it.each(['{Enter}', ' '])('should open and close from the keyboard with %s', async (key) => {
        button.focus()
        await user.keyboard(key)
        expect(isOpen(button)).toBe(true)
        expect(visibleText()).toBeVisible()
        await user.keyboard(key)
        expect(isOpen(button)).toBe(false)
        expect(visibleText()).not.toBeVisible()
      })

      it('should close on Escape and keep focus on the button', async () => {
        await user.click(button)
        await user.keyboard('{Escape}')
        expect(isOpen(button)).toBe(false)
        expect(button).toHaveFocus()
      })

      it('should close as focus leaves it', async () => {
        await user.click(button)
        await user.tab()
        expect(screen.getByRole('button', { name: 'Elsewhere' })).toHaveFocus()
        expect(isOpen(button)).toBe(false)
      })

      it('should open on a tap and close on a tap outside', async () => {
        await user.pointer({ keys: '[TouchA]', target: button })
        expect(isOpen(button)).toBe(true)
        expect(visibleText()).toBeVisible()
        await user.pointer({ keys: '[TouchA]', target: document.body })
        expect(isOpen(button)).toBe(false)
      })

      it('should stay open as its own text is tapped', async () => {
        await user.click(button)
        await user.pointer({ keys: '[TouchA]', target: visibleText() })
        expect(isOpen(button)).toBe(true)
      })

      it('should keep the checkbox name and description while open', async () => {
        await user.click(button)
        const box = screen.getByRole('checkbox', { name: 'Open Debug Console' })
        expect(box).toHaveAccessibleDescription('Shows engine logs in a panel.')
        expect(box).not.toBeChecked()
      })
    })

    describe('and the page listens for keys', () => {
      let page: Mock
      let button: HTMLElement

      beforeEach(() => {
        page = vi.fn()
        render(
          <div onKeyDown={(event) => page(event.key, event.defaultPrevented)}>
            <CheckboxLabel label="Nested" help="Nested help." />
          </div>
        )
        button = screen.getByRole('button', { name: 'More information' })
      })

      it('should keep the Escape that closes it from reaching the page, and pass it on once closed', async () => {
        await user.click(button)
        await user.keyboard('{Escape}')
        expect(page).not.toHaveBeenCalled()
        expect(button).toHaveAttribute('aria-expanded', 'false')
        await user.keyboard('{Escape}')
        expect(page).toHaveBeenCalledWith('Escape', false)
      })
    })
  })

  describe('when the reader toggles it', () => {
    let checked: Mock

    beforeEach(() => {
      checked = vi.fn()
      render(<CheckboxLabel label="Sounds" onCheckedChange={checked} />)
    })

    it('should toggle from its visible text and from Space', async () => {
      await user.click(screen.getByText('Sounds'))
      expect(checked).toHaveBeenLastCalledWith(true)
      expect(screen.getByRole('checkbox')).toHaveFocus()
      await user.keyboard(' ')
      expect(checked).toHaveBeenLastCalledWith(false)
    })
  })

  describe('when disabled', () => {
    let checked: Mock
    let container: HTMLElement

    beforeEach(() => {
      checked = vi.fn()
      container = render(
        <CheckboxLabel label="Locked" disabled onCheckedChange={checked} />
      ).container
    })

    it('should ignore clicks on the text and mark the row disabled', async () => {
      await user.click(screen.getByText('Locked'))
      expect(checked).not.toHaveBeenCalled()
      expect(screen.getByRole('checkbox', { name: 'Locked' })).toBeDisabled()
      expect(container.firstElementChild).toHaveAttribute('data-disabled', 'true')
    })
  })

  describe('when inside a form', () => {
    beforeEach(() => {
      render(
        <form aria-label="settings">
          <CheckboxLabel label="Debug" name="debug" />
        </form>
      )
    })

    it('should submit its name and value only while checked', async () => {
      const form = screen.getByRole<HTMLFormElement>('form')
      expect(new FormData(form).has('debug')).toBe(false)
      await user.click(screen.getByText('Debug'))
      expect(new FormData(form).get('debug')).toBe('on')
    })
  })
})

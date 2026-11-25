import { createRef, type RefObject } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from 'vitest'
import { Switch } from './Switch'

describe('Switch', () => {
  let user: UserEvent

  beforeEach(() => {
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when rendered with a visible label', () => {
    beforeEach(() => {
      render(<Switch label="Item Sold" checked onChange={() => {}} />)
    })

    it('should expose a named switch with its state', () => {
      const control = screen.getByRole('switch', { name: 'Item Sold' })
      expect(control).toBeChecked()
      expect(screen.getByText('Item Sold')).toBeVisible()
    })
  })

  describe('when the label is visually hidden', () => {
    beforeEach(() => {
      render(<Switch label="All email notifications" labelHidden />)
    })

    it('should keep the accessible name', () => {
      expect(screen.getByRole('switch', { name: 'All email notifications' })).not.toBeChecked()
    })
  })

  describe('when given a ref', () => {
    let ref: RefObject<HTMLInputElement | null>

    beforeEach(() => {
      ref = createRef<HTMLInputElement>()
      render(<Switch label="Tips" ref={ref} />)
    })

    it('should forward its ref to the native input', () => {
      expect(ref.current).toBe(screen.getByRole('switch'))
    })
  })

  describe('when rendered on the server', () => {
    it('should render without a browser', () => {
      expect(renderToString(<Switch label="Events" defaultChecked />)).toContain('role="switch"')
    })
  })

  describe('when the reader toggles it', () => {
    let change: Mock
    let checked: Mock

    beforeEach(() => {
      change = vi.fn()
      checked = vi.fn()
    })

    describe('and toggles by click and by Space', () => {
      beforeEach(() => {
        render(<Switch label="Bid Received" onChange={change} onCheckedChange={checked} />)
      })

      it('should report the next state, keeping the native change event', async () => {
        const control = screen.getByRole('switch', { name: 'Bid Received' })
        await user.click(control)
        expect(checked).toHaveBeenLastCalledWith(true)
        await user.keyboard(' ')
        expect(checked).toHaveBeenLastCalledWith(false)
        expect(change).toHaveBeenCalledTimes(2)
      })
    })

    describe('and clicks its visible label', () => {
      beforeEach(() => {
        render(<Switch label="Royalties Earned" onCheckedChange={checked} />)
      })

      it('should toggle the switch on', async () => {
        await user.click(screen.getByText('Royalties Earned'))
        expect(checked).toHaveBeenCalledWith(true)
      })
    })

    describe('and the switch is controlled', () => {
      beforeEach(() => {
        render(<Switch label="Rental" checked={false} onCheckedChange={() => {}} />)
      })

      it('should stay as the caller set it', async () => {
        await user.click(screen.getByRole('switch'))
        expect(screen.getByRole('switch')).not.toBeChecked()
      })
    })
  })

  describe('when disabled', () => {
    let checked: Mock

    beforeEach(() => {
      checked = vi.fn()
      render(<Switch label="Tips" disabled onCheckedChange={checked} />)
    })

    it('should be disabled and ignore clicks and keys', async () => {
      const control = screen.getByRole('switch', { name: 'Tips' })
      expect(control).toBeDisabled()
      await user.click(control)
      control.focus()
      await user.keyboard(' ')
      expect(checked).not.toHaveBeenCalled()
      expect(control).not.toBeChecked()
    })
  })

  describe('when inside a form', () => {
    beforeEach(() => {
      render(
        <form aria-label="settings">
          <Switch label="Events" name="events" defaultChecked />
          <Switch label="Tips" name="tips" />
        </form>
      )
    })

    it('should submit its name and value only while on', () => {
      const data = new FormData(screen.getByRole('form'))
      expect(data.get('events')).toBe('on')
      expect(data.has('tips')).toBe(false)
    })
  })
  describe('when it takes the hud appearance', () => {
    it('should keep the named native switch and add the appearance, size and surface classes', () => {
      render(<Switch appearance="hud" size="sm" surface="light" label="Hear others" labelHidden className="row" />)
      const control = screen.getByRole('switch', { name: 'Hear others' })
      const root = control.closest('label') as HTMLElement
      expect(root.className).toMatch(/hud/)
      expect(root.className).toMatch(/sm/)
      expect(root.className).toMatch(/onLight/)
      expect(root.className).toMatch(/row/)
    })

    it('should report the next state to the caller', async () => {
      const onCheckedChange = vi.fn()
      render(<Switch appearance="hud" label="Set as Home" checked={false} onCheckedChange={onCheckedChange} />)
      await userEvent.click(screen.getByRole('switch', { name: 'Set as Home' }))
      expect(onCheckedChange).toHaveBeenCalledWith(true)
    })

    it('should toggle on Enter, as the button it replaces did', async () => {
      const onCheckedChange = vi.fn()
      render(<Switch appearance="hud" label="Auto" checked={false} onCheckedChange={onCheckedChange} />)
      screen.getByRole('switch', { name: 'Auto' }).focus()
      await userEvent.keyboard('{Enter}')
      expect(onCheckedChange).toHaveBeenCalledWith(true)
    })

    it('should leave Enter alone in the default appearance', async () => {
      const onCheckedChange = vi.fn()
      render(<Switch label="Notifications" checked={false} onCheckedChange={onCheckedChange} />)
      screen.getByRole('switch', { name: 'Notifications' }).focus()
      await userEvent.keyboard('{Enter}')
      expect(onCheckedChange).not.toHaveBeenCalled()
    })

    it('should not add the size or surface classes to the default appearance', () => {
      render(<Switch size="sm" surface="light" label="Notifications" />)
      const root = screen.getByRole('switch', { name: 'Notifications' }).closest('label') as HTMLElement
      expect(root.className).not.toMatch(/hud|sm|onLight/)
    })
  })
})

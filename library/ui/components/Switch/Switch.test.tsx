import { createRef } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { Switch } from './Switch'

describe('Switch', () => {
  describe('when rendered with a label', () => {
    it('exposes a named switch with its state', () => {
      render(<Switch label="Item Sold" checked onChange={() => {}} />)
      const control = screen.getByRole('switch', { name: 'Item Sold' })
      expect(control).toBeChecked()
      expect(screen.getByText('Item Sold')).toBeVisible()
    })

    it('keeps the name when the label is visually hidden', () => {
      render(<Switch label="All email notifications" labelHidden />)
      expect(screen.getByRole('switch', { name: 'All email notifications' })).not.toBeChecked()
    })

    it('forwards its ref to the native input', () => {
      const ref = createRef<HTMLInputElement>()
      render(<Switch label="Tips" ref={ref} />)
      expect(ref.current).toBe(screen.getByRole('switch'))
    })

    it('renders on the server without a browser', () => {
      expect(renderToString(<Switch label="Events" defaultChecked />)).toContain('role="switch"')
    })
  })

  describe('when the reader toggles it', () => {
    it('reports the next state by click and by Space, keeping the native change event', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      const checked = vi.fn()
      render(<Switch label="Bid Received" onChange={change} onCheckedChange={checked} />)
      const control = screen.getByRole('switch', { name: 'Bid Received' })
      await user.click(control)
      expect(checked).toHaveBeenLastCalledWith(true)
      await user.keyboard(' ')
      expect(checked).toHaveBeenLastCalledWith(false)
      expect(change).toHaveBeenCalledTimes(2)
    })

    it('toggles from its visible label', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<Switch label="Royalties Earned" onCheckedChange={checked} />)
      await user.click(screen.getByText('Royalties Earned'))
      expect(checked).toHaveBeenCalledWith(true)
    })

    it('stays as the caller set it when controlled', async () => {
      const user = userEvent.setup()
      render(<Switch label="Rental" checked={false} onCheckedChange={() => {}} />)
      await user.click(screen.getByRole('switch'))
      expect(screen.getByRole('switch')).not.toBeChecked()
    })
  })

  describe('when disabled', () => {
    it('is disabled and ignores clicks and keys', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<Switch label="Tips" disabled onCheckedChange={checked} />)
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
    it('submits its name and value only while on', () => {
      render(
        <form aria-label="settings">
          <Switch label="Events" name="events" defaultChecked />
          <Switch label="Tips" name="tips" />
        </form>
      )
      const data = new FormData(screen.getByRole('form'))
      expect(data.get('events')).toBe('on')
      expect(data.has('tips')).toBe(false)
    })
  })
})

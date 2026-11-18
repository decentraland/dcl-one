import { createRef, useState } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { flushSync } from 'react-dom'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { Checkbox } from './Checkbox'

describe('Checkbox', () => {
  describe('when rendered with a name', () => {
    it('exposes a named, unchecked checkbox with outline and md as defaults', () => {
      const { container } = render(<Checkbox aria-label="Accept terms" />)
      const box = screen.getByRole('checkbox', { name: 'Accept terms' })
      expect(box).not.toBeChecked()
      const root = container.firstElementChild!
      expect(root).toHaveAttribute('data-size', 'md')
      expect(root).toHaveAttribute('data-variant', 'outline')
    })

    it('takes its name from visible text through aria-labelledby', () => {
      render(
        <>
          <span id="caption">Remember me</span>
          <Checkbox aria-labelledby="caption" />
        </>
      )
      expect(screen.getByRole('checkbox', { name: 'Remember me' })).toBeInTheDocument()
    })

    it('refuses a checkbox without a name at the type level', () => {
      // @ts-expect-error: a checkbox needs aria-label or aria-labelledby
      render(<Checkbox />)
      expect(screen.getByRole('checkbox')).not.toHaveAccessibleName()
    })

    it.each(['sm', 'md', 'lg'] as const)('renders the %s size', (size) => {
      const { container } = render(<Checkbox aria-label="Size" size={size} />)
      expect(container.firstElementChild).toHaveAttribute('data-size', size)
    })

    it('renders the filled variant', () => {
      const { container } = render(<Checkbox aria-label="Filled" variant="filled" />)
      expect(container.firstElementChild).toHaveAttribute('data-variant', 'filled')
    })

    it('hides the box and glyphs from assistive technology', () => {
      const { container } = render(<Checkbox aria-label="Hidden glyphs" defaultChecked />)
      for (const svg of container.querySelectorAll('svg')) {
        expect(svg.closest('[aria-hidden="true"]')).not.toBeNull()
      }
    })

    it('forwards its ref to the native input', () => {
      const ref = createRef<HTMLInputElement>()
      render(<Checkbox aria-label="Ref" ref={ref} />)
      expect(ref.current).toBe(screen.getByRole('checkbox'))
    })

    it('renders on the server without a browser', () => {
      const html = renderToString(<Checkbox aria-label="Server" defaultChecked />)
      expect(html).toContain('type="checkbox"')
      expect(html).toContain('aria-label="Server"')
    })
  })

  describe('when the reader toggles it', () => {
    it('reports the next state by click and by Space, keeping the native change event', async () => {
      const user = userEvent.setup()
      const change = vi.fn()
      const checked = vi.fn()
      render(<Checkbox aria-label="Newsletter" onChange={change} onCheckedChange={checked} />)
      const box = screen.getByRole('checkbox')
      await user.click(box)
      expect(checked).toHaveBeenLastCalledWith(true)
      expect(box).toBeChecked()
      await user.keyboard(' ')
      expect(checked).toHaveBeenLastCalledWith(false)
      expect(change).toHaveBeenCalledTimes(2)
    })

    it('is reached by Tab and shows focus on the native input', async () => {
      const user = userEvent.setup()
      render(
        <>
          <button type="button">Before</button>
          <Checkbox aria-label="Focusable" />
        </>
      )
      await user.tab()
      await user.tab()
      expect(screen.getByRole('checkbox')).toHaveFocus()
    })

    it('ignores Enter, as a native checkbox does', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<Checkbox aria-label="Enter" onCheckedChange={checked} />)
      screen.getByRole('checkbox').focus()
      await user.keyboard('{Enter}')
      expect(checked).not.toHaveBeenCalled()
    })

    it('lets React warn about a checked box with no way to change it', () => {
      const error = vi.spyOn(console, 'error').mockImplementation(() => {})
      try {
        render(<Checkbox aria-label="Frozen" checked />)
        expect(error.mock.calls.flat().join(' ')).toMatch(/without an `onChange` handler/)
      } finally {
        error.mockRestore()
      }
    })

    it('does not warn when the caller handles the change', () => {
      const error = vi.spyOn(console, 'error').mockImplementation(() => {})
      try {
        render(<Checkbox aria-label="Handled" checked onCheckedChange={() => {}} />)
        expect(error).not.toHaveBeenCalled()
      } finally {
        error.mockRestore()
      }
    })

    it('stays as the caller set it when controlled', async () => {
      const user = userEvent.setup()
      render(<Checkbox aria-label="Controlled" checked={false} onCheckedChange={() => {}} />)
      await user.click(screen.getByRole('checkbox'))
      expect(screen.getByRole('checkbox')).not.toBeChecked()
    })
  })

  describe('when indeterminate', () => {
    it('reports the mixed state on the native input', () => {
      render(<Checkbox aria-label="Select all" indeterminate />)
      const box = screen.getByRole<HTMLInputElement>('checkbox')
      expect(box.indeterminate).toBe(true)
      expect(box).toBePartiallyChecked()
    })

    it('hands the click to a controlled caller, which settles the next state', async () => {
      const user = userEvent.setup()
      function SelectAll() {
        const [state, setState] = useState<'mixed' | boolean>('mixed')
        return (
          <Checkbox
            aria-label="Select all"
            checked={state === true}
            indeterminate={state === 'mixed'}
            onCheckedChange={setState}
          />
        )
      }
      render(<SelectAll />)
      const box = screen.getByRole<HTMLInputElement>('checkbox')
      await user.click(box)
      expect(box.indeterminate).toBe(false)
      expect(box).toBeChecked()
    })

    it('keeps the caller’s mixed state after a click it did not accept', async () => {
      const user = userEvent.setup()
      function Stubborn() {
        const [, force] = useState(0)
        return (
          <Checkbox
            aria-label="Stubborn"
            checked={false}
            indeterminate
            onCheckedChange={() => force((n) => n + 1)}
          />
        )
      }
      render(<Stubborn />)
      const box = screen.getByRole<HTMLInputElement>('checkbox')
      await user.click(box)
      expect(box.indeterminate).toBe(true)
      expect(box).not.toBeChecked()
    })

    it('takes a state the caller commits synchronously from its handler', async () => {
      const user = userEvent.setup()
      function SyncSelectAll() {
        const [state, setState] = useState<'mixed' | boolean>('mixed')
        return (
          <Checkbox
            aria-label="Sync select all"
            checked={state === true}
            indeterminate={state === 'mixed'}
            onCheckedChange={(next) => flushSync(() => setState(next))}
          />
        )
      }
      render(<SyncSelectAll />)
      const box = screen.getByRole<HTMLInputElement>('checkbox')
      await user.click(box)
      expect(box.indeterminate).toBe(false)
      expect(box).toBeChecked()
    })

    it('stays mixed when the caller keeps it mixed without rendering', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<Checkbox aria-label="Kept" indeterminate onCheckedChange={checked} />)
      const box = screen.getByRole<HTMLInputElement>('checkbox')
      await user.click(box)
      expect(checked).toHaveBeenCalledWith(true)
      expect(box.indeterminate).toBe(true)
      expect(box).toBePartiallyChecked()
    })

    it('drops the mixed state when the caller clears it', () => {
      const { rerender } = render(<Checkbox aria-label="Clear" indeterminate />)
      rerender(<Checkbox aria-label="Clear" indeterminate={false} />)
      expect(screen.getByRole<HTMLInputElement>('checkbox').indeterminate).toBe(false)
    })
  })

  describe('when disabled', () => {
    it('is disabled and ignores clicks and keys', async () => {
      const user = userEvent.setup()
      const checked = vi.fn()
      render(<Checkbox aria-label="Locked" disabled onCheckedChange={checked} />)
      const box = screen.getByRole('checkbox', { name: 'Locked' })
      expect(box).toBeDisabled()
      await user.click(box)
      box.focus()
      await user.keyboard(' ')
      expect(checked).not.toHaveBeenCalled()
      expect(box).not.toBeChecked()
      expect(box).not.toHaveFocus()
    })
  })

  describe('when inside a form', () => {
    it('submits its name and value only while checked', () => {
      render(
        <form aria-label="preferences">
          <Checkbox aria-label="Events" name="events" defaultChecked />
          <Checkbox aria-label="Tips" name="tips" />
          <Checkbox aria-label="Mixed" name="mixed" indeterminate />
        </form>
      )
      const data = new FormData(screen.getByRole('form'))
      expect(data.get('events')).toBe('on')
      expect(data.has('tips')).toBe(false)
      expect(data.has('mixed')).toBe(false)
    })

    it('returns to its default state on form reset', async () => {
      const user = userEvent.setup()
      render(
        <form aria-label="reset">
          <Checkbox aria-label="Reset me" name="reset" />
          <button type="reset">Reset</button>
        </form>
      )
      const box = screen.getByRole('checkbox')
      await user.click(box)
      expect(box).toBeChecked()
      await user.click(screen.getByRole('button', { name: 'Reset' }))
      expect(box).not.toBeChecked()
    })
  })
})

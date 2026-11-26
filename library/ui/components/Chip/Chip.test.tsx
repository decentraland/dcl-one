import '@testing-library/jest-dom/vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { Chip } from './Chip'

describe('Chip', () => {
  describe('when it has no clear action', () => {
    it('is one toggle named by its label, not pressed, in the light variant', () => {
      render(<Chip label="All" />)
      const buttons = screen.getAllByRole('button')
      expect(buttons).toHaveLength(1)
      expect(buttons[0]).toHaveAccessibleName('All')
      expect(buttons[0]).toHaveAttribute('type', 'button')
      expect(buttons[0]).toHaveAttribute('aria-pressed', 'false')
      expect(buttons[0]!.parentElement!.className).not.toMatch(/dark|selected/)
    })

    it('reports a selected chip as pressed and takes the dark variant', () => {
      render(<Chip label="Wearables" variant="dark" selected />)
      const toggle = screen.getByRole('button', { name: 'Wearables' })
      expect(toggle).toHaveAttribute('aria-pressed', 'true')
      expect(toggle.parentElement!.className).toMatch(/dark/)
      expect(toggle.parentElement!.className).toMatch(/selected/)
    })

    it('draws its icon inside the toggle and calls onClick', () => {
      const onClick = vi.fn()
      render(<Chip label="Hats" icon={<i data-testid="icon" />} onClick={onClick} />)
      const toggle = screen.getByRole('button', { name: 'Hats' })
      expect(toggle).toContainElement(screen.getByTestId('icon'))
      fireEvent.click(toggle)
      expect(onClick).toHaveBeenCalledTimes(1)
    })

    it('does nothing on a click without onClick', () => {
      render(<Chip label="Hats" />)
      expect(() => fireEvent.click(screen.getByRole('button', { name: 'Hats' }))).not.toThrow()
    })
  })

  describe('when it has a clear action', () => {
    it('adds a second button with the default name and a cross', () => {
      render(<Chip label="Hats" selected onClear={() => {}} />)
      const clear = screen.getByRole('button', { name: 'Clear' })
      expect(clear).toHaveAttribute('type', 'button')
      expect(clear.querySelector('svg')).not.toBeNull()
    })

    it('clears without toggling', () => {
      const onClick = vi.fn()
      const onClear = vi.fn()
      render(<Chip label="Hats" selected onClick={onClick} onClear={onClear} clearLabel="Clear Hats" />)
      fireEvent.click(screen.getByRole('button', { name: 'Clear Hats' }))
      expect(onClear).toHaveBeenCalledTimes(1)
      expect(onClick).not.toHaveBeenCalled()
    })

    it("draws the caller's glyph in place of the cross", () => {
      render(<Chip label="Hats" onClear={() => {}} clearIcon={<i data-testid="glyph" />} />)
      const clear = screen.getByRole('button', { name: 'Clear' })
      expect(clear).toContainElement(screen.getByTestId('glyph'))
      expect(clear.querySelector('svg')).toBeNull()
    })
  })
})

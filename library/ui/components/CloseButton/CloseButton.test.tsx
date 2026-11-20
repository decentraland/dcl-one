import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderToString } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { CloseButton } from './CloseButton'

describe('CloseButton', () => {
  describe('when rendered without a name', () => {
    it('is a button named Close that does not submit a form', () => {
      render(<CloseButton />)
      const button = screen.getByRole('button', { name: 'Close' })
      expect(button).toHaveAttribute('type', 'button')
    })

    it('hides its glyph from assistive technology', () => {
      render(<CloseButton />)
      const glyph = screen.getByRole('button', { name: 'Close' }).querySelector('svg')
      expect(glyph).toHaveAttribute('aria-hidden', 'true')
    })
  })

  describe('when the caller names it', () => {
    it('uses the given name', () => {
      render(<CloseButton aria-label="Cerrar" />)
      expect(screen.getByRole('button', { name: 'Cerrar' })).toBeInTheDocument()
    })
  })

  describe.each(['solid', 'filled', 'plain'] as const)('when the variant is %s', (variant) => {
    it('draws one glyph and records the variant', () => {
      render(<CloseButton variant={variant} />)
      const button = screen.getByRole('button', { name: 'Close' })
      expect(button).toHaveAttribute('data-variant', variant)
      expect(button.querySelectorAll('svg')).toHaveLength(1)
    })
  })

  describe('when pressed', () => {
    it('calls onClick for a click, Enter and Space', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(<CloseButton onClick={onClick} />)
      await user.click(screen.getByRole('button', { name: 'Close' }))
      await user.keyboard('{Enter}')
      await user.keyboard(' ')
      expect(onClick).toHaveBeenCalledTimes(3)
    })
  })

  describe('when disabled', () => {
    it('is really disabled and ignores a click', async () => {
      const onClick = vi.fn()
      const user = userEvent.setup()
      render(<CloseButton disabled onClick={onClick} />)
      const button = screen.getByRole('button', { name: 'Close' })
      expect(button).toBeDisabled()
      await user.click(button)
      expect(onClick).not.toHaveBeenCalled()
    })
  })

  describe('when the caller passes native attributes', () => {
    it('keeps them and adds its class to the caller’s', () => {
      render(<CloseButton className="mine" data-testid="close" aria-controls="panel" />)
      const button = screen.getByTestId('close')
      expect(button).toHaveClass('mine')
      expect(button).toHaveAttribute('aria-controls', 'panel')
    })
  })

  it('renders on the server without a browser', () => {
    expect(renderToString(<CloseButton variant="plain" />)).toContain('aria-label="Close"')
  })
})

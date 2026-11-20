import { render, screen } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { ItemLabel } from './ItemLabel'

describe('ItemLabel', () => {
  describe('when the variant has a default text', () => {
    it.each([
      ['new', 'New'],
      ['smart', 'Smart']
    ] as const)('shows the %s label as "%s"', (variant, text) => {
      render(<ItemLabel variant={variant} data-testid="label" />)
      expect(screen.getByTestId('label')).toHaveTextContent(text)
      expect(screen.getByTestId('label')).toHaveAttribute('data-variant', variant)
    })

    it('shows the caller’s word in place of the default', () => {
      render(<ItemLabel variant="new">Nuevo</ItemLabel>)
      expect(screen.getByText('Nuevo')).toBeVisible()
      expect(screen.queryByText('New')).toBeNull()
    })
  })

  describe('when it is an amount', () => {
    it('shows what the caller gives', () => {
      render(<ItemLabel variant="amount">x2</ItemLabel>)
      expect(screen.getByText('x2')).toBeVisible()
    })

    it('reads the caller’s name for it', () => {
      render(
        <ItemLabel variant="amount" role="img" aria-label="2 owned">
          x2
        </ItemLabel>
      )
      expect(screen.getByRole('img', { name: '2 owned' })).toBeInTheDocument()
    })

    it('draws nothing inside without a text', () => {
      render(<ItemLabel variant="amount" data-testid="label" />)
      expect(screen.getByTestId('label')).toBeEmptyDOMElement()
    })
  })

  describe('when it is a smart label', () => {
    it('hides its mark from assistive technology', () => {
      render(<ItemLabel variant="smart" data-testid="label" />)
      const mark = screen.getByTestId('label').querySelector('svg')
      expect(mark).toHaveAttribute('aria-hidden', 'true')
    })

    it.each(['new', 'amount'] as const)('draws no mark for the %s label', (variant) => {
      render(<ItemLabel variant={variant} data-testid="label" />)
      expect(screen.getByTestId('label').querySelector('svg')).toBeNull()
    })
  })

  describe('when the caller passes native attributes', () => {
    it('keeps them and adds its class to the caller’s', () => {
      render(<ItemLabel variant="new" className="mine" title="Listed this week" data-testid="label" />)
      expect(screen.getByTestId('label')).toHaveClass('mine')
      expect(screen.getByTestId('label')).toHaveAttribute('title', 'Listed this week')
    })
  })

  it('renders on the server without a browser', () => {
    expect(renderToString(<ItemLabel variant="amount">x3</ItemLabel>)).toContain('x3')
  })
})

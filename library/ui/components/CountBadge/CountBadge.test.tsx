import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CountBadge } from './CountBadge'

describe('CountBadge', () => {
  describe('when the count is positive', () => {
    it('shows the count in the md size, hidden from assistive technology', () => {
      render(<CountBadge count={3} />)
      const badge = screen.getByText('3')
      expect(badge).toHaveAttribute('aria-hidden', 'true')
      expect(badge.className).toMatch(/md/)
    })

    it('takes the size it is given', () => {
      render(<CountBadge count={3} size="xs" />)
      expect(screen.getByText('3').className).toMatch(/xs/)
    })

    it('is read when it is announced', () => {
      render(<CountBadge count={3} announce />)
      expect(screen.getByText('3')).not.toHaveAttribute('aria-hidden')
    })
  })

  describe('when the count has a maximum', () => {
    it('shows the count up to the maximum', () => {
      render(<CountBadge count={99} max={99} />)
      expect(screen.getByText('99')).toBeInTheDocument()
    })

    it('shows +max above it', () => {
      render(<CountBadge count={100} max={99} />)
      expect(screen.getByText('+99')).toBeInTheDocument()
    })
  })

  describe('when there is nothing to count', () => {
    it.each([0, -1, Number.NaN])('renders nothing for %s', (count) => {
      const { container } = render(<CountBadge count={count} />)
      expect(container).toBeEmptyDOMElement()
    })
  })
})

import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Spinner } from './Spinner'

describe('Spinner', () => {
  describe('when rendered without a label', () => {
    it('is a progress bar named Loading', () => {
      render(<Spinner />)
      expect(screen.getByRole('progressbar', { name: 'Loading' })).toBeInTheDocument()
    })
  })

  describe('when an inline size is requested', () => {
    it('should keep a named 16px progress indicator', () => {
      render(<Spinner size={16} label="Loading balance" />)
      expect(screen.getByRole('progressbar', { name: 'Loading balance' })).toHaveAttribute('data-size', '16')
    })
  })

  describe('when rendered with a label and the light variant', () => {
    it('names the progress bar and keeps the consumer class', () => {
      render(<Spinner label="Loading places" variant="light" className="extra" />)
      const bar = screen.getByRole('progressbar', { name: 'Loading places' })
      expect(bar.className).toMatch(/light/)
      expect(bar.className).toMatch(/extra/)
    })
  })
})

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

  describe('when given a size', () => {
    it('sizes the ring and keeps the track in proportion', () => {
      render(<Spinner size={60} style={{ margin: 4 }} />)
      const bar = screen.getByRole('progressbar', { name: 'Loading' })
      expect(bar.style.width).toBe('60px')
      expect(bar.style.height).toBe('60px')
      expect(bar.style.borderWidth).toBe('8px')
      expect(bar.style.margin).toBe('4px')
    })

    it('leaves the stylesheet size when none is given', () => {
      render(<Spinner />)
      expect(screen.getByRole('progressbar').style.width).toBe('')
    })
  })

  describe('when the variant is thin', () => {
    it('is a named progress bar that draws its ring as a hidden image, at the given size', () => {
      render(<Spinner variant="thin" size={26} label="Checking the place" className="extra" />)
      const bar = screen.getByRole('progressbar', { name: 'Checking the place' })
      expect(bar.className).toMatch(/thin/)
      expect(bar.className).toMatch(/extra/)
      expect(bar.style.width).toBe('26px')
      expect(bar.style.borderWidth).toBe('')
      expect(bar.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
      expect(bar.querySelectorAll('circle')).toHaveLength(2)
    })

    it('takes the stylesheet size when none is given', () => {
      render(<Spinner variant="thin" />)
      expect(screen.getByRole('progressbar').style.width).toBe('')
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

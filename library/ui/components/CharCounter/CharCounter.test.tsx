import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CharCounter } from './CharCounter'

describe('CharCounter', () => {
  describe('when it shows a count', () => {
    it('writes current/max by default', () => {
      render(<CharCounter current={3} max={10} />)
      expect(screen.getByText('3/10')).not.toHaveAttribute('data-over')
    })

    it('writes the spaced and the worded formats', () => {
      render(
        <>
          <CharCounter current={3} max={10} format="spaced" />
          <CharCounter current={3} max={10} format="words" />
        </>
      )
      expect(screen.getByText('3 / 10')).toBeInTheDocument()
      expect(screen.getByText('(3 out of 10 characters)')).toBeInTheDocument()
    })

    it('starts at zero and keeps the caller class and attributes', () => {
      render(<CharCounter max={10} className="mine" id="count" />)
      const counter = screen.getByText('0/10')
      expect(counter).toHaveClass('mine')
      expect(counter).toHaveAttribute('id', 'count')
    })
  })

  describe('when the count passes the maximum', () => {
    it('marks itself over', () => {
      render(<CharCounter current={11} max={10} />)
      expect(screen.getByText('11/10')).toHaveAttribute('data-over')
    })

    it('is not over at the maximum', () => {
      render(<CharCounter current={10} max={10} />)
      expect(screen.getByText('10/10')).not.toHaveAttribute('data-over')
    })

    it('is not marked when over is off', () => {
      render(<CharCounter current={11} max={10} over={false} />)
      expect(screen.getByText('11/10')).not.toHaveAttribute('data-over')
    })

    it('is not marked without a maximum', () => {
      render(<CharCounter current={11} />)
      expect(screen.getByText('11/undefined')).not.toHaveAttribute('data-over')
    })
  })
})

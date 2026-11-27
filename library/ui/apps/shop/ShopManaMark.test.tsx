import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ShopManaMark } from './ShopManaMark'

describe('ShopManaMark', () => {
  describe('when no network is provided', () => {
    it('should render the default inline size as a decorative mark', () => {
      const { container } = render(<ShopManaMark />)
      const mark = container.querySelector('svg')
      expect(mark).toHaveAttribute('width', '16')
      expect(mark).toHaveAttribute('height', '16')
      expect(mark).toHaveAttribute('aria-hidden', 'true')
    })
  })

  describe('when an inline price selects a network', () => {
    it('should use distinct shapes for Ethereum and Polygon and honor the requested size', () => {
      const view = render(<ShopManaMark network="ethereum" />)
      const ethereum = view.container.querySelector('svg')!.innerHTML
      view.rerender(<ShopManaMark network="polygon" size={24} className="price" />)
      const polygon = view.container.querySelector('svg')!
      expect(polygon.innerHTML).not.toBe(ethereum)
      expect(polygon).toHaveAttribute('width', '24')
      expect(polygon).toHaveAttribute('height', '24')
      expect(polygon).toHaveClass('price')
    })
    it('should treat Matic as the Polygon network', () => {
      const view = render(<ShopManaMark network="polygon" />)
      const polygon = view.container.innerHTML
      view.rerender(<ShopManaMark network="matic" />)
      expect(view.container.innerHTML).toBe(polygon)
    })
  })

  describe('when a runtime caller supplies an unknown network', () => {
    it('should fall back to the Ethereum mark', () => {
      const view = render(<ShopManaMark network="ethereum" />)
      const ethereum = view.container.innerHTML
      // @ts-expect-error Exercise an unsupported network supplied by an untyped caller.
      view.rerender(<ShopManaMark network="unknown" />)
      expect(view.container.innerHTML).toBe(ethereum)
    })
  })
})

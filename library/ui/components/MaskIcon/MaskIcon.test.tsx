import '@testing-library/jest-dom/vitest'
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { MaskIcon } from './MaskIcon'

const icon = (container: HTMLElement) => container.firstElementChild as HTMLElement

describe('MaskIcon', () => {
  it('is a hidden 24px box masked by the image', () => {
    const { container } = render(<MaskIcon src="/close.webp" />)
    const element = icon(container)
    expect(element).toHaveAttribute('aria-hidden', 'true')
    expect(element).toHaveStyle({ width: '24px', height: '24px' })
    expect(element.style.maskImage).toBe('url("/close.webp")')
  })

  it('takes a size, and a width and height that override it', () => {
    const { container, rerender } = render(<MaskIcon src="/a.webp" size={12} />)
    expect(icon(container)).toHaveStyle({ width: '12px', height: '12px' })
    rerender(<MaskIcon src="/a.webp" size={12} width={14} height={22} />)
    expect(icon(container)).toHaveStyle({ width: '14px', height: '22px' })
  })

  it("keeps the caller's class", () => {
    const { container } = render(<MaskIcon src="/a.webp" className="mine" />)
    expect(icon(container)).toHaveClass('mine')
  })

  it('quotes a data URI, which holds spaces and quotes', () => {
    const src = "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'/>"
    const { container } = render(<MaskIcon src={src} />)
    expect(icon(container).style.maskImage).toContain('viewBox=')
  })
})

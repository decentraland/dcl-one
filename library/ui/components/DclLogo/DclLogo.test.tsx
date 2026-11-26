import '@testing-library/jest-dom/vitest'
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DclLogo } from './DclLogo'

describe('DclLogo', () => {
  it('is a hidden 24px mark', () => {
    const { container } = render(<DclLogo />)
    const svg = container.querySelector('svg')!
    expect(svg).toHaveAttribute('aria-hidden', 'true')
    expect(svg).toHaveAttribute('width', '24')
    expect(svg).toHaveAttribute('height', '24')
  })

  it("takes a size and keeps the caller's class", () => {
    const { container } = render(<DclLogo size={72} className="mine" />)
    const svg = container.querySelector('svg')!
    expect(svg).toHaveAttribute('width', '72')
    expect(svg).toHaveClass('mine')
  })

  it('gives each instance its own gradient and clip ids, and points at them', () => {
    const { container } = render(
      <>
        <DclLogo />
        <DclLogo />
      </>
    )
    const ids = Array.from(container.querySelectorAll('[id]')).map((node) => node.id)
    expect(ids).toHaveLength(8)
    expect(new Set(ids).size).toBe(8)
    const [first] = Array.from(container.querySelectorAll('svg'))
    const clip = first!.querySelector('clipPath')!.id
    expect(first!.querySelector('g')).toHaveAttribute('clip-path', `url(#${clip})`)
  })
})

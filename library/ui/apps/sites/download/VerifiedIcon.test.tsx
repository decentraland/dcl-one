import { createHash } from 'node:crypto'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DownloadActions } from './DownloadActions'

describe('when the hero and closing download CTA show verification badges', () => {
  it('should share the exact solid-white product graphic with distinct decorative clip IDs', () => {
    render(
      <>
        <DownloadActions downloads="Hero count" />
        <DownloadActions downloads="Closing count" />
      </>
    )
    const icons = ['Hero count', 'Closing count'].map((label) =>
      screen.getByText(label).querySelector('svg')!
    )
    const ids = icons.map((icon) => icon.querySelector('clipPath')!.id)
    expect(new Set(ids).size).toBe(2)
    for (const [index, icon] of icons.entries()) {
      expect(icon).toHaveAttribute('viewBox', '0 0 18 18')
      expect(icon).toHaveAttribute('aria-hidden', 'true')
      expect(icon).toHaveAttribute('focusable', 'false')
      expect(icon.querySelector('linearGradient')).toBeNull()
      expect(icon.querySelector('path')).toHaveAttribute('fill', 'white')
      expect(icon.querySelector('g')).toHaveAttribute('clip-path', `url(#${ids[index]})`)
      expect(
        createHash('sha256').update(icon.querySelector('path')!.getAttribute('d')!).digest('hex')
      ).toBe('12d18bef06ad2540c70451e9ef0bcb144d3504069fa34df4ac8b5eae6cdf9511')
      expect(icon.querySelector('rect')).toHaveAttribute('width', '17.1016')
    }
  })
})

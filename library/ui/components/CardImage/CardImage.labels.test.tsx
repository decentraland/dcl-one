import { render, screen, fireEvent } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { CardImage } from './CardImage'

describe('when card image names are missing or malformed', () => {
  it.each([' ', '\t', '\n', '\u00a0', undefined, null, 0, {}, []])(
    'should keep a missing poster decorative for alt %j',
    (alt) => {
      const { container } = render(<CardImage alt={alt as string} />)
      expect(screen.queryByRole('img')).toBeNull()
      expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
      expect(container.firstElementChild).not.toHaveAttribute('aria-label')
    }
  )
  it('should retain the decorative name after an image fails', () => {
    const { container } = render(<CardImage src="/poster.webp" alt={' \t '} />)
    expect(container.querySelector('img')).toHaveAttribute('alt', '')
    fireEvent.error(container.querySelector('img')!)
    expect(screen.queryByRole('img')).toBeNull()
    expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
  })
  it('should update a surviving fallback with the current meaningful name', () => {
    const { rerender } = render(<CardImage alt={' \t '} />)
    rerender(<CardImage alt="  Poster artwork  " />)
    expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
      'aria-label',
      'Poster artwork'
    )
  })
  it.each([' ', '\t', '\u00a0'])('should server-render decorative fallback for %j', (alt) => {
    const html = renderToString(<CardImage alt={alt} />)
    expect(html).toContain('aria-hidden="true"')
    expect(html).not.toContain('role="img"')
    expect(html).not.toContain('aria-label=')
  })
})

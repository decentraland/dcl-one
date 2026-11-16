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
  it('should keep caller fallback content exposed when the name is blank', () => {
    const { container } = render(<CardImage alt=" " fallback={<span>No artwork</span>} />)
    expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
    expect(container.firstElementChild).not.toHaveAttribute('role')
    expect(screen.getByText('No artwork').closest('[aria-hidden="true"]')).toBeNull()
  })
  it('should expose caller fallback content after an unnamed image fails', () => {
    const { container } = render(
      <CardImage src="/poster.webp" alt="" fallback={<span>No artwork</span>} />
    )
    fireEvent.error(container.querySelector('img')!)
    expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
    expect(screen.getByText('No artwork').closest('[aria-hidden="true"]')).toBeNull()
  })
  it.each([null, false, true, '', [null, false]])(
    'should keep the default icon decorative when the caller fallback %j renders nothing',
    (fallback) => {
      const { container } = render(<CardImage alt="" fallback={fallback} />)
      expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
      expect(container.querySelector('svg')).not.toBeNull()
    }
  )
  it('should expose a numeric caller fallback under a blank name', () => {
    const { container } = render(<CardImage alt="" fallback={0} />)
    expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
    expect(container.firstElementChild).toHaveTextContent('0')
  })
  it('should let a meaningful name own caller fallback content', () => {
    render(<CardImage alt="Artwork" fallback={<span>No artwork</span>} />)
    expect(screen.getByRole('img', { name: 'Artwork' })).not.toHaveAttribute('aria-hidden')
  })
  it.each([' ', '\t', '\u00a0'])('should server-render decorative fallback for %j', (alt) => {
    const html = renderToString(<CardImage alt={alt} />)
    expect(html).toContain('aria-hidden="true"')
    expect(html).not.toContain('role="img"')
    expect(html).not.toContain('aria-label=')
  })
})

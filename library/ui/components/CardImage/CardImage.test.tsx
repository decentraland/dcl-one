import { render, screen, fireEvent } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CardImage } from './CardImage'

describe('when card images resolve or fail', () => {
  it('should keep decorative posters out of the accessible name and honor priority loading', () => {
    const { container, rerender } = render(<CardImage src="/poster.webp" alt="" />)
    expect(screen.queryByRole('img')).toBeNull()
    expect(container.querySelector('img')).toHaveAttribute('loading', 'lazy')
    rerender(<CardImage src="/poster.webp" alt="Poster artwork" priority />)
    expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute('loading', 'eager')
    expect(screen.getByRole('img')).toHaveAttribute('fetchpriority', 'high')
  })
  it('should render a meaningful fallback when no image exists', () => {
    render(<CardImage src={null} alt="Poster artwork" />)
    expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
      'data-state',
      'empty'
    )
    expect(document.querySelector('img')).toBeNull()
  })
  it('should try a distinct fallback source once and then show the fallback', () => {
    const { container } = render(
      <CardImage src="/thumbnail.webp" fallbackSrc="/original.jpg" alt="Poster artwork" />
    )
    fireEvent.error(container.querySelector('img')!)
    expect(container.querySelector('img')).toHaveAttribute('src', '/original.jpg')
    fireEvent.error(container.querySelector('img')!)
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
      'data-state',
      'failed'
    )
  })
  it('should never loop when both source URLs are the same', () => {
    const { container } = render(<CardImage src="/same.jpg" fallbackSrc="/same.jpg" alt="" />)
    fireEvent.error(container.querySelector('img')!)
    expect(container.querySelector('img')).toBeNull()
  })
  it('should recover for a changed source after a failure', () => {
    const { container, rerender } = render(<CardImage src="/old.jpg" alt="" />)
    fireEvent.error(container.querySelector('img')!)
    expect(container.querySelector('img')).toBeNull()
    rerender(<CardImage src="/new.jpg" alt="" />)
    expect(container.querySelector('img')).toHaveAttribute('src', '/new.jpg')
    fireEvent.load(container.querySelector('img')!)
    expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
  })
})

describe('when SSR images settle before hydration handlers attach', () => {
  afterEach(() => vi.restoreAllMocks())
  it('should recognize a cached successful image', () => {
    vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
    vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(640)
    const { container } = render(<CardImage src="/cached.jpg" alt="" />)
    expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
  })
  it('should recover from cached failures without waiting for a lost error event', () => {
    vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
    vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(0)
    const { container } = render(
      <CardImage src="/cached-broken.jpg" fallbackSrc="/also-broken.jpg" alt="Artwork" />
    )
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByRole('img', { name: 'Artwork' })).toHaveAttribute('data-state', 'failed')
  })
})

describe('when public card images enforce the shared URL policy', () => {
  it.each([
    'javascript:alert(1)',
    'data:image/png,none',
    '//example.com/image.png',
    'http://example.com/image.png',
    'poster.jpg',
    '/\\example.com/image.png',
    'https://user:pass@example.com/image.png',
    'https://example.com/\nimage.png'
  ])('should omit unsafe source %j and allow a safe fallback instead', (src) => {
    const { container, rerender } = render(<CardImage src={src} alt="" />)
    expect(container.querySelector('img')).toBeNull()
    rerender(<CardImage src={src} fallbackSrc="/safe-fallback.jpg" alt="" />)
    expect(container.querySelector('img')).toHaveAttribute('src', '/safe-fallback.jpg')
  })
  it.each(['/poster.webp', 'https://example.com/poster.webp'])(
    'should retain an allowed source %s',
    (src) => {
      const { container } = render(<CardImage src={src} alt="" />)
      expect(container.querySelector('img')).toHaveAttribute('src', src)
    }
  )
  it('should apply the same policy to fallback sources', () => {
    const { container } = render(
      <CardImage src="/broken.jpg" fallbackSrc="javascript:alert(1)" alt="Artwork" />
    )
    fireEvent.error(container.querySelector('img')!)
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByRole('img', { name: 'Artwork' })).toBeInTheDocument()
  })
})

describe('when an image attempt is replaced or incomplete', () => {
  afterEach(() => vi.restoreAllMocks())
  it('should ignore late errors from the detached previous image', () => {
    const { container, rerender } = render(<CardImage src="/old.webp" alt="Artwork" />)
    const oldImage = container.querySelector('img')!
    rerender(<CardImage src="/new.webp" alt="Artwork" />)
    const newImage = container.querySelector('img')!
    expect(newImage).not.toBe(oldImage)
    fireEvent.error(oldImage)
    expect(container.querySelector('img')).toBe(newImage)
    expect(newImage).toHaveAttribute('src', '/new.webp')
    fireEvent.load(newImage)
    expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
  })
  it('should wait for an incomplete request instead of marking it failed', () => {
    vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(false)
    const { container } = render(<CardImage src="/pending.webp" alt="Artwork" />)
    expect(container.querySelector('img')).toHaveAttribute('src', '/pending.webp')
    expect(container.firstElementChild).toHaveAttribute('data-state', 'loading')
  })
  it('should render caller-provided fallback content with the owned accessible name', () => {
    const { container } = render(<CardImage alt="Artwork" fallback={<span>No artwork</span>} />)
    expect(screen.getByRole('img', { name: 'Artwork' })).toHaveTextContent('No artwork')
    expect(container.querySelector('img, svg')).toBeNull()
  })
})

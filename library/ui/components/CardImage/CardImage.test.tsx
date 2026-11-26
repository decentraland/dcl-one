import { render, screen, fireEvent, type RenderResult } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { CardImage } from './CardImage'

describe('CardImage', () => {
  let container: HTMLElement
  let rerender: RenderResult['rerender']

  afterEach(() => {
    vi.restoreAllMocks()
  })

  describe('when card images resolve or fail', () => {
    describe('and the poster is decorative', () => {
      beforeEach(() => {
        ;({ container, rerender } = render(<CardImage src="/poster.webp" alt="" />))
      })

      it('should keep the poster out of the accessible name', () => {
        expect(screen.queryByRole('img')).toBeNull()
      })

      it('should load the poster lazily', () => {
        expect(container.querySelector('img')).toHaveAttribute('loading', 'lazy')
      })

      describe('and it is rerendered with a name and priority', () => {
        beforeEach(() => {
          rerender(<CardImage src="/poster.webp" alt="Poster artwork" priority />)
        })

        it('should load the named poster eagerly', () => {
          expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
            'loading',
            'eager'
          )
        })

        it('should fetch the poster with high priority', () => {
          expect(screen.getByRole('img')).toHaveAttribute('fetchpriority', 'high')
        })
      })
    })

    describe('and no image exists', () => {
      beforeEach(() => {
        render(<CardImage src={null} alt="Poster artwork" />)
      })

      it('should render a meaningful empty fallback', () => {
        expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
          'data-state',
          'empty'
        )
      })

      it('should render no image element', () => {
        expect(document.querySelector('img')).toBeNull()
      })
    })

    describe('and the source fails with a distinct fallback source', () => {
      beforeEach(() => {
        ;({ container } = render(
          <CardImage src="/thumbnail.webp" fallbackSrc="/original.jpg" alt="Poster artwork" />
        ))
        fireEvent.error(container.querySelector('img')!)
      })

      it('should try the fallback source', () => {
        expect(container.querySelector('img')).toHaveAttribute('src', '/original.jpg')
      })

      describe('and the fallback source fails too', () => {
        beforeEach(() => {
          fireEvent.error(container.querySelector('img')!)
        })

        it('should render no image element', () => {
          expect(container.querySelector('img')).toBeNull()
        })

        it('should show the failed fallback', () => {
          expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
            'data-state',
            'failed'
          )
        })
      })
    })

    describe('and both source URLs are the same', () => {
      beforeEach(() => {
        ;({ container } = render(<CardImage src="/same.jpg" fallbackSrc="/same.jpg" alt="" />))
        fireEvent.error(container.querySelector('img')!)
      })

      it('should never loop on the failed source', () => {
        expect(container.querySelector('img')).toBeNull()
      })
    })

    describe('and the source fails', () => {
      beforeEach(() => {
        ;({ container, rerender } = render(<CardImage src="/old.jpg" alt="" />))
        fireEvent.error(container.querySelector('img')!)
      })

      it('should render no image element', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      describe('and the source changes', () => {
        beforeEach(() => {
          rerender(<CardImage src="/new.jpg" alt="" />)
        })

        it('should recover with the changed source', () => {
          expect(container.querySelector('img')).toHaveAttribute('src', '/new.jpg')
        })

        describe('and the changed source loads', () => {
          beforeEach(() => {
            fireEvent.load(container.querySelector('img')!)
          })

          it('should mark the image ready', () => {
            expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
          })
        })
      })
    })
  })

  describe('when SSR images settle before hydration handlers attach', () => {
    beforeEach(() => {
      vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
    })

    describe('and the cached image succeeded', () => {
      beforeEach(() => {
        vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(640)
        ;({ container } = render(<CardImage src="/cached.jpg" alt="" />))
      })

      it('should recognize the cached successful image', () => {
        expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
      })
    })

    describe('and the cached images failed', () => {
      beforeEach(() => {
        vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(0)
        ;({ container } = render(
          <CardImage src="/cached-broken.jpg" fallbackSrc="/also-broken.jpg" alt="Artwork" />
        ))
      })

      it('should drop the image without waiting for a lost error event', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      it('should show the failed fallback', () => {
        expect(screen.getByRole('img', { name: 'Artwork' })).toHaveAttribute(
          'data-state',
          'failed'
        )
      })
    })
  })

  describe('when public card images enforce the shared URL policy', () => {
    describe.each([
      'javascript:alert(1)',
      'data:image/png,none',
      '//example.com/image.png',
      'http://example.com/image.png',
      'poster.jpg',
      '/\\example.com/image.png',
      'https://user:pass@example.com/image.png',
      'https://example.com/\nimage.png'
    ])('and the source is the unsafe %j', (src) => {
      beforeEach(() => {
        ;({ container, rerender } = render(<CardImage src={src} alt="" />))
      })

      it('should omit the unsafe source', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      describe('and a safe fallback is given', () => {
        beforeEach(() => {
          rerender(<CardImage src={src} fallbackSrc="/safe-fallback.jpg" alt="" />)
        })

        it('should use the safe fallback instead', () => {
          expect(container.querySelector('img')).toHaveAttribute('src', '/safe-fallback.jpg')
        })
      })
    })

    describe('and the source is allowed', () => {
      it.each(['/poster.webp', 'https://example.com/poster.webp'])(
        'should retain the allowed source %s',
        (src) => {
          ;({ container } = render(<CardImage src={src} alt="" />))
          expect(container.querySelector('img')).toHaveAttribute('src', src)
        }
      )
    })

    describe('and the fallback source is unsafe', () => {
      beforeEach(() => {
        ;({ container } = render(
          <CardImage src="/broken.jpg" fallbackSrc="javascript:alert(1)" alt="Artwork" />
        ))
        fireEvent.error(container.querySelector('img')!)
      })

      it('should not load the unsafe fallback source', () => {
        expect(container.querySelector('img')).toBeNull()
      })

      it('should show the named fallback', () => {
        expect(screen.getByRole('img', { name: 'Artwork' })).toBeInTheDocument()
      })
    })
  })

  describe('when an image attempt is replaced or incomplete', () => {
    describe('and the source is replaced', () => {
      let oldImage: HTMLImageElement
      let newImage: HTMLImageElement

      beforeEach(() => {
        ;({ container, rerender } = render(<CardImage src="/old.webp" alt="Artwork" />))
        oldImage = container.querySelector('img')!
        rerender(<CardImage src="/new.webp" alt="Artwork" />)
        newImage = container.querySelector('img')!
      })

      it('should render a new image element', () => {
        expect(newImage).not.toBe(oldImage)
      })

      describe('and the detached previous image errors late', () => {
        beforeEach(() => {
          fireEvent.error(oldImage)
        })

        it('should keep the new image', () => {
          expect(container.querySelector('img')).toBe(newImage)
          expect(newImage).toHaveAttribute('src', '/new.webp')
        })

        describe('and the new image loads', () => {
          beforeEach(() => {
            fireEvent.load(newImage)
          })

          it('should mark the image ready', () => {
            expect(container.firstElementChild).toHaveAttribute('data-state', 'ready')
          })
        })
      })
    })

    describe('and the request is incomplete', () => {
      beforeEach(() => {
        vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(false)
        ;({ container } = render(<CardImage src="/pending.webp" alt="Artwork" />))
      })

      it('should keep requesting the source', () => {
        expect(container.querySelector('img')).toHaveAttribute('src', '/pending.webp')
      })

      it('should wait instead of marking it failed', () => {
        expect(container.firstElementChild).toHaveAttribute('data-state', 'loading')
      })
    })

    describe('and the caller provides fallback content', () => {
      beforeEach(() => {
        ;({ container } = render(<CardImage alt="Artwork" fallback={<span>No artwork</span>} />))
      })

      it('should render the fallback content with the owned accessible name', () => {
        expect(screen.getByRole('img', { name: 'Artwork' })).toHaveTextContent('No artwork')
      })

      it('should render no image or icon', () => {
        expect(container.querySelector('img, svg')).toBeNull()
      })
    })
  })
})

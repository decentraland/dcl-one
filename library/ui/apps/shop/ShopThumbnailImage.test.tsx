import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ShopThumbnailImage } from './ShopThumbnailImage'

describe('ShopThumbnailImage', () => {
  function renderThumbnail() {
    render(
      <ShopThumbnailImage
        alt="Jacket"
        src="/small.webp"
        srcSet="/large.webp 640w"
        sizes="50vw"
        fallbackSrc="/original.png"
      />
    )
    return screen.getByRole('img', { name: 'Jacket' })
  }

  describe('when a responsive thumbnail loads normally', () => {
    it('should expose the responsive candidates and requested size', () => {
      const image = renderThumbnail()
      expect(image).toHaveAttribute('srcset', '/large.webp 640w')
      expect(image).toHaveAttribute('sizes', '50vw')
    })
  })

  describe('when a thumbnail request fails', () => {
    it('should remove the responsive candidates and request the original', () => {
      const image = renderThumbnail()
      fireEvent.error(image)
      expect(image).not.toHaveAttribute('srcset')
      expect(image).toHaveAttribute('src', '/original.png')
    })

    it('should avoid requesting a failed original again', () => {
      const image = renderThumbnail()
      fireEvent.error(image)
      const observer = new MutationObserver(() => {})
      observer.observe(image, { attributes: true })
      fireEvent.error(image)
      expect(observer.takeRecords()).toHaveLength(0)
      observer.disconnect()
    })
  })

  describe('when an image failed before hydration', () => {
    beforeEach(() => {
      vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true)
      vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(0)
    })
    afterEach(() => vi.restoreAllMocks())

    it('should restore the original without another error event', () => {
      const image = renderThumbnail()
      expect(image).not.toHaveAttribute('srcset')
      expect(image).toHaveAttribute('src', '/original.png')
    })
  })
})

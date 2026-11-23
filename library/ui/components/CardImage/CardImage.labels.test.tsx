import { render, screen, fireEvent, type RenderResult } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { beforeEach, describe, expect, it } from 'vitest'
import { CardImage } from './CardImage'

describe('when card image names are missing or malformed', () => {
  let container: HTMLElement
  let rerender: RenderResult['rerender']

  describe.each([' ', '\t', '\n', ' ', undefined, null, 0, {}, []])(
    'and the poster is missing with alt %j',
    (alt) => {
      beforeEach(() => {
        ;({ container } = render(<CardImage alt={alt as string} />))
      })

      it('should keep the poster decorative', () => {
        expect(screen.queryByRole('img')).toBeNull()
        expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
        expect(container.firstElementChild).not.toHaveAttribute('aria-label')
      })
    }
  )

  describe('and an image with a blank name is rendered', () => {
    beforeEach(() => {
      ;({ container } = render(<CardImage src="/poster.webp" alt={' \t '} />))
    })

    it('should render the image with an empty alt', () => {
      expect(container.querySelector('img')).toHaveAttribute('alt', '')
    })

    describe('and the image fails', () => {
      beforeEach(() => {
        fireEvent.error(container.querySelector('img')!)
      })

      it('should retain the decorative name', () => {
        expect(screen.queryByRole('img')).toBeNull()
        expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
      })
    })
  })

  describe('and a surviving fallback later receives a meaningful name', () => {
    beforeEach(() => {
      ;({ rerender } = render(<CardImage alt={' \t '} />))
      rerender(<CardImage alt="  Poster artwork  " />)
    })

    it('should update the fallback with the current trimmed name', () => {
      expect(screen.getByRole('img', { name: 'Poster artwork' })).toHaveAttribute(
        'aria-label',
        'Poster artwork'
      )
    })
  })

  describe('and the caller provides fallback content under a blank name', () => {
    beforeEach(() => {
      ;({ container } = render(<CardImage alt=" " fallback={<span>No artwork</span>} />))
    })

    it('should keep the caller fallback content exposed', () => {
      expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
      expect(container.firstElementChild).not.toHaveAttribute('role')
      expect(screen.getByText('No artwork').closest('[aria-hidden="true"]')).toBeNull()
    })
  })

  describe('and an unnamed image with caller fallback content fails', () => {
    beforeEach(() => {
      ;({ container } = render(
        <CardImage src="/poster.webp" alt="" fallback={<span>No artwork</span>} />
      ))
      fireEvent.error(container.querySelector('img')!)
    })

    it('should expose the caller fallback content', () => {
      expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
      expect(screen.getByText('No artwork').closest('[aria-hidden="true"]')).toBeNull()
    })
  })

  describe.each([null, false, true, '', [null, false]])(
    'and the caller fallback %j renders nothing',
    (fallback) => {
      beforeEach(() => {
        ;({ container } = render(<CardImage alt="" fallback={fallback} />))
      })

      it('should keep the default icon decorative', () => {
        expect(container.firstElementChild).toHaveAttribute('aria-hidden', 'true')
        expect(container.querySelector('svg')).not.toBeNull()
      })
    }
  )

  describe('and the caller fallback is numeric under a blank name', () => {
    beforeEach(() => {
      ;({ container } = render(<CardImage alt="" fallback={0} />))
    })

    it('should expose the numeric fallback', () => {
      expect(container.firstElementChild).not.toHaveAttribute('aria-hidden')
      expect(container.firstElementChild).toHaveTextContent('0')
    })
  })

  describe('and a meaningful name has caller fallback content', () => {
    beforeEach(() => {
      render(<CardImage alt="Artwork" fallback={<span>No artwork</span>} />)
    })

    it('should let the name own the fallback content', () => {
      expect(screen.getByRole('img', { name: 'Artwork' })).not.toHaveAttribute('aria-hidden')
    })
  })

  describe.each([' ', '\t', ' '])('and the blank name %j is server-rendered', (alt) => {
    let html: string

    beforeEach(() => {
      html = renderToString(<CardImage alt={alt} />)
    })

    it('should server-render a decorative fallback', () => {
      expect(html).toContain('aria-hidden="true"')
      expect(html).not.toContain('role="img"')
      expect(html).not.toContain('aria-label=')
    })
  })
})

import { describe, expect, it } from 'vitest'
import { render as rtlRender, screen } from '@testing-library/react'
import type { ReactElement, ReactNode } from 'react'
import { TranslationProvider } from '~/intl'
import { ThumbnailMosaic } from './ThumbnailMosaic'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
const render = (ui: ReactElement) => rtlRender(ui, { wrapper })

describe('ThumbnailMosaic', () => {
  it('renders one cell per thumbnail, capped at four, and exposes the count for the layout', () => {
    render(<ThumbnailMosaic thumbnails={['a.png', 'b.png', 'c.png', 'd.png', 'e.png'].map(url => ({ url }))} />)
    expect(screen.getAllByTestId('thumbnail-mosaic-cell')).toHaveLength(4)
    expect(screen.getByTestId('thumbnail-mosaic')).toHaveAttribute('data-count', '4')
  })

  it('tints each cell with its item rarity', () => {
    render(<ThumbnailMosaic thumbnails={[{ url: 'a.png', rarity: 'mythic' }, { url: 'b.png' }]} />)
    const [mythic, plain] = screen.getAllByTestId('thumbnail-mosaic-thumbnail')
    expect(mythic.style.backgroundImage).toContain('radial-gradient')
    expect(plain.style.backgroundImage).toBe('')
  })

  it('labels an empty collection, but not one that is still loading or just has no thumbnails', () => {
    const { rerender } = render(<ThumbnailMosaic thumbnails={[]} empty />)
    expect(screen.getByTestId('thumbnail-mosaic')).toHaveAttribute('data-empty')
    expect(screen.getByTestId('thumbnail-mosaic-empty')).toHaveTextContent('No items')

    rerender(<ThumbnailMosaic thumbnails={[]} empty loading />)
    expect(screen.getByTestId('thumbnail-mosaic')).not.toHaveAttribute('data-empty')
    expect(screen.queryByTestId('thumbnail-mosaic-empty')).not.toBeInTheDocument()

    rerender(<ThumbnailMosaic thumbnails={[]} />)
    expect(screen.getByTestId('thumbnail-mosaic')).not.toHaveAttribute('data-empty')
    expect(screen.queryByTestId('thumbnail-mosaic-empty')).not.toBeInTheDocument()
  })

  it('keeps the empty glyph but drops the label where the cover is too small for it', () => {
    render(<ThumbnailMosaic thumbnails={[]} empty showEmptyLabel={false} />)
    expect(screen.getByTestId('thumbnail-mosaic-empty')).toBeInTheDocument()
    expect(screen.getByTestId('thumbnail-mosaic-empty')).not.toHaveTextContent('No items')
  })

  it('shows a skeleton instead of cells while loading', () => {
    render(<ThumbnailMosaic thumbnails={[]} loading />)
    expect(screen.getByTestId('thumbnail-mosaic-loading')).toBeInTheDocument()
    expect(screen.queryByTestId('thumbnail-mosaic-cell')).not.toBeInTheDocument()
  })
})

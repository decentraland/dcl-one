import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ItemThumbnail } from './ItemThumbnail'

describe('ItemThumbnail', () => {
  it('shows the artwork over a wash in the rarity color', () => {
    render(<ItemThumbnail src="https://cdn.example/hat.png" rarity="epic" />)
    expect(screen.getByTestId('item-thumbnail-img')).toHaveAttribute('src', 'https://cdn.example/hat.png')
    const frame = screen.getByTestId('item-thumbnail')
    expect(frame.style.backgroundImage).toContain('rgba(40, 156, 255')
  })

  it('keeps the neutral field, and no artwork, when rarity and image are missing', () => {
    render(<ItemThumbnail />)
    const frame = screen.getByTestId('item-thumbnail')
    expect(frame.style.backgroundImage).toBe('')
    expect(screen.queryByTestId('item-thumbnail-img')).not.toBeInTheDocument()
  })
})

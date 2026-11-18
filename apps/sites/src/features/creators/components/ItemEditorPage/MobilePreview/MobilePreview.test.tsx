import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BodyShape } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { ItemType, type Item } from '~/lib/items'
import { MobilePreview } from './MobilePreview'

const make = (id: string, bodyShapes: BodyShape[]): Item => ({
  id,
  name: id,
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: { representations: [{ bodyShapes, mainFile: 'm.glb', contents: [] }] },
  contents: {},
  createdAt: 1,
  updatedAt: 1
})
const hat = make('hat', [BodyShape.MALE, BodyShape.FEMALE])
const dress = make('dress', [BodyShape.FEMALE])

describe('MobilePreview', () => {
  it('shows the preview with a thumbnail strip that switches the selection and a dismissible hint', async () => {
    const onTap = vi.fn()
    render(
      <MobilePreview
        items={[hat, dress]}
        selectedId="hat"
        dressedIds={['hat']}
        bodyShape={BodyShape.MALE}
        onTap={onTap}
      >
        <div data-testid="preview" />
      </MobilePreview>,
      { wrapper: TranslationProvider }
    )
    expect(screen.getByTestId('preview')).toBeInTheDocument()
    expect(screen.getByTestId('mobile-strip-item-hat')).toHaveAttribute('data-selected')
    expect(screen.getByTestId('mobile-strip-item-dress')).toHaveAttribute('data-unavailable')
    await userEvent.click(screen.getByTestId('mobile-strip-item-dress'))
    expect(onTap).toHaveBeenCalledWith(dress)
    await userEvent.click(screen.getByTestId('mobile-hint-dismiss'))
    expect(screen.queryByTestId('mobile-hint')).not.toBeInTheDocument()
  })

  it('offers each thumbnail as a toggle, telling apart what is on the avatar from what is not', async () => {
    render(
      <MobilePreview
        items={[hat, dress]}
        selectedId="hat"
        dressedIds={['hat']}
        bodyShape={BodyShape.MALE}
        onTap={vi.fn()}
      >
        <div data-testid="preview" />
      </MobilePreview>,
      { wrapper: TranslationProvider }
    )
    expect(screen.getByRole('button', { name: 'Remove hat from the avatar' })).toHaveAttribute('data-dressed')
    const off = screen.getByRole('button', { name: 'Show dress on the avatar' })
    expect(off).not.toHaveAttribute('data-dressed')
    expect(off).toHaveAttribute('aria-pressed', 'false')
  })
})

import { describe, it, expect, vi } from 'vitest'
import { type ComponentProps, type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { ItemType, type Item } from '~/lib/items'
import { type ItemListing } from '~/lib/listings'
import { ItemRowStatus } from '~/lib/itemSync'
import { ValidationSeverity } from '~/lib/validation'
import { ItemListRow } from './ItemListRow'

const item: Item = {
  id: 'i1',
  name: 'Pirate Hat',
  description: 'Yarr',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  collectionId: 'c1',
  rarity: 'legendary',
  isPublished: true,
  isApproved: true,
  inCatalyst: true,
  type: ItemType.WEARABLE,
  data: {
    category: 'upper_body',
    representations: [
      {
        bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseFemale'],
        mainFile: 'hat.glb',
        contents: ['hat.glb']
      }
    ]
  },
  contents: { 'thumbnail.png': 'Qmthumb' },
  createdAt: 1000,
  updatedAt: 1000
}

type RowProps = Omit<ComponentProps<typeof ItemListRow>, 'item'>

function renderRow(overrides: Partial<Item> = {}, props: RowProps | boolean = {}) {
  const rowProps = typeof props === 'boolean' ? { withPlayMode: props } : props
  const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
  return render(<ItemListRow item={{ ...item, ...overrides }} {...rowProps} />, { wrapper })
}

const creditsListing: ItemListing = { itemId: '3', currency: 'credits', credits: 500 }
const manaListing: ItemListing = { itemId: '3', currency: 'mana', manaWei: 5000000000000000000n }

const emote: Partial<Item> = {
  type: ItemType.EMOTE,
  data: { category: 'dance', representations: [], loop: true }
}

describe('ItemListRow', () => {
  it('has no Status column until the page asks for it', () => {
    renderRow({}, { syncStatus: ItemRowStatus.MODIFIED })
    expect(screen.queryByTestId('item-row-status')).not.toBeInTheDocument()
  })

  it('explains a modified item in an (i) tooltip inside the pill', async () => {
    renderRow({}, { withStatus: true, syncStatus: ItemRowStatus.MODIFIED })
    const pill = screen.getByTestId('item-sync-status')
    expect(pill).toHaveAttribute('data-status', 'modified')
    expect(pill).toHaveTextContent('Modified')

    await userEvent.hover(screen.getByTestId('item-sync-status-hint-trigger'))
    expect(await screen.findByTestId('item-sync-status-hint')).toHaveTextContent(/send them for review/i)
  })

  it('flags an item whose files are not live, with the in-world warning in its tooltip', async () => {
    renderRow({}, { withStatus: true, syncStatus: ItemRowStatus.MISSING })
    const pill = screen.getByTestId('item-sync-status')
    expect(pill).toHaveAttribute('data-status', 'missing')
    expect(pill).toHaveTextContent('Incomplete')
    await userEvent.hover(screen.getByTestId('item-sync-status-hint-trigger'))
    expect(await screen.findByTestId('item-sync-status-hint')).toHaveTextContent(/may not work in-world/)
  })

  it('shows a plain published pill with nothing to explain, and leaves the cell blank while the sync is unknown', () => {
    const { unmount } = renderRow({}, { withStatus: true, syncStatus: ItemRowStatus.PUBLISHED })
    expect(screen.getByTestId('item-sync-status')).toHaveTextContent('Published')
    expect(screen.queryByTestId('item-sync-status-hint-trigger')).not.toBeInTheDocument()
    unmount()

    renderRow({}, { withStatus: true, syncStatus: null })
    expect(screen.getByTestId('item-row-status')).toHaveAttribute('data-empty', 'true')
    expect(screen.queryByTestId('item-sync-status')).not.toBeInTheDocument()
  })

  it('shows name, body shape, category and rarity with its supply', () => {
    renderRow()
    expect(screen.getByText('Pirate Hat')).toBeInTheDocument()
    expect(screen.getByTestId('item-row-body-shape')).toHaveTextContent('Female')
    expect(screen.getByRole('img', { name: 'Female' })).toBeInTheDocument()
    expect(screen.getByTestId('item-row-category')).toHaveTextContent('Upper Body')
    expect(screen.getByRole('img', { name: 'Upper Body' })).toBeInTheDocument()
    expect(screen.getByTestId('item-row-rarity')).toHaveTextContent(/legendary \(100\)/i)
    expect(screen.queryByTestId('item-row-status')).not.toBeInTheDocument()
  })

  it('resolves the thumbnail through storage', () => {
    renderRow()
    expect(screen.queryByRole('link')).not.toBeInTheDocument()
    const img = screen.getByTestId('item-row').querySelector('img')
    expect(img?.src).toContain('/storage/contents/Qmthumb')
  })

  it('shows placeholders for missing fields', () => {
    renderRow({
      rarity: undefined,
      data: { representations: [] }
    })
    expect(screen.getByTestId('item-row-body-shape')).toHaveTextContent('—')
    expect(screen.getByTestId('item-row-category')).toHaveTextContent('—')
    expect(screen.getByTestId('item-row-rarity')).toBeEmptyDOMElement()
  })

  it('omits the play mode cell when the list has no emotes', () => {
    renderRow()
    expect(screen.queryByTestId('item-row-play-mode')).not.toBeInTheDocument()
  })

  it('shows the emote play mode between category and rarity when the list has emotes', () => {
    renderRow(emote, true)
    const cell = screen.getByTestId('item-row-play-mode')
    expect(cell).toHaveTextContent('Loop')
    expect(cell.nextElementSibling).toBe(screen.getByTestId('item-row-rarity'))
    expect(cell.previousElementSibling).toBe(screen.getByTestId('item-row-category'))
  })

  it('shows play once for a non-looping emote', () => {
    renderRow({ ...emote, data: { ...emote.data!, loop: false } }, true)
    expect(screen.getByTestId('item-row-play-mode')).toHaveTextContent('Play Once')
  })

  it('shows a dash for a wearable in a list with emotes', () => {
    renderRow({}, true)
    expect(screen.getByTestId('item-row-play-mode')).toHaveTextContent('—')
  })

  it('has no price or sales cells while the collection has not been published', () => {
    renderRow()
    expect(screen.queryByTestId('item-row-price')).not.toBeInTheDocument()
    expect(screen.queryByTestId('item-row-sales')).not.toBeInTheDocument()
  })

  it('shows a credits price and the minted count over the max supply after rarity', () => {
    renderRow({ tokenId: '3', totalSupply: 15 }, { withMarket: true, listing: creditsListing })
    const price = screen.getByTestId('item-row-price')
    expect(price).toHaveTextContent('500')
    expect(price).toHaveAttribute('data-currency', 'credits')
    expect(price.previousElementSibling).toBe(screen.getByTestId('item-row-rarity'))
    expect(screen.getByTestId('item-row-sales')).toHaveTextContent('15/100')
    expect(screen.getByTestId('item-row-sale-status')).toHaveTextContent(/on sale/i)
  })

  it('shows the discounted price, the percent off and the struck-through original while a discount reaches it', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: creditsListing, discountPpm: 300_000 })
    const price = screen.getByTestId('item-row-discounted-price')
    expect(price).toHaveTextContent('-30%')
    expect(price).toHaveTextContent('350')
    expect(price.querySelector('s')).toHaveTextContent('500')
  })

  it('shows a MANA price discounted the way the contract charges it', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: manaListing, discountPpm: 125_000 })
    const price = screen.getByTestId('item-row-discounted-price')
    expect(price).toHaveTextContent('-12.5%')
    expect(price).toHaveTextContent('4.38')
  })

  // A cheap item can round the discount away: a badge over an unchanged price would read as a broken sale.
  it('shows no discount when it does not change the price', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: { ...creditsListing, credits: 1 }, discountPpm: 300_000 })
    expect(screen.queryByTestId('item-row-discounted-price')).not.toBeInTheDocument()
  })

  it('shows the plain price when no running discount reaches the listing', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: creditsListing })
    expect(screen.queryByTestId('item-row-discounted-price')).not.toBeInTheDocument()
  })

  it('shows a MANA price for a listing made from the legacy marketplace', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: manaListing })
    const price = screen.getByTestId('item-row-price')
    expect(price).toHaveTextContent('5')
    expect(price).toHaveAttribute('data-currency', 'mana')
    expect(screen.getByTestId('item-row-sales')).toHaveTextContent('0/100')
  })

  it('shows Free for a listing priced at zero', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: { ...creditsListing, credits: 0 } })
    expect(screen.getByTestId('item-row-price')).toHaveTextContent('Free')
  })

  it('shows a dash and offers to put on sale an item that is not on sale', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: null, canSell: true, onPutOnSale: vi.fn() })
    expect(screen.getByTestId('item-row-price')).toHaveTextContent('—')
    expect(screen.getByTestId('item-row-sales')).toHaveTextContent('0/100')
    expect(screen.getByRole('button', { name: /put on sale/i })).toBeEnabled()
  })

  it('shows no put on sale action to a viewer who may not sell', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: null, canSell: true })
    expect(screen.queryByRole('button', { name: /put on sale/i })).not.toBeInTheDocument()
    expect(screen.getByTestId('item-row-sale-status')).toHaveTextContent('—')
    expect(screen.getByTestId('item-row-sale-status')).toHaveAttribute('data-empty')
  })

  it('keeps the put on sale action disabled while the collection awaits its first approval', () => {
    renderRow({ tokenId: '3' }, { withMarket: true, listing: null, onPutOnSale: vi.fn() })
    expect(screen.getByRole('button', { name: /put on sale/i })).toBeDisabled()
  })

  it('shows every unit minted and a sold-out status for a sold-out item', () => {
    renderRow({ tokenId: '3', totalSupply: 100 }, { withMarket: true, listing: null })
    expect(screen.getByTestId('item-row-price')).toHaveTextContent('—')
    expect(screen.getByTestId('item-row-sales')).toHaveTextContent('100/100')
    expect(screen.getByTestId('item-row-sale-status')).toHaveTextContent(/sold out/i)
    expect(screen.getByTestId('item-row-sale-status')).not.toHaveAttribute('data-empty')
    expect(screen.queryByRole('button', { name: /put on sale/i })).not.toBeInTheDocument()
  })

  it('leaves the price and sale status blank while listings are loading', () => {
    renderRow({ tokenId: '3' }, { withMarket: true })
    expect(screen.getByTestId('item-row-price')).toBeEmptyDOMElement()
    expect(screen.getByTestId('item-row-sale-status')).toBeEmptyDOMElement()
  })

  it('offers no edit shortcuts to a viewer who cannot edit the item', () => {
    renderRow(
      { tokenId: '3' },
      { withMarket: true, listing: creditsListing, onRename: vi.fn(), onEditThumbnail: vi.fn() }
    )
    expect(screen.queryByTestId('item-row-edit-name')).not.toBeInTheDocument()
    expect(screen.queryByTestId('item-row-edit-thumbnail')).not.toBeInTheDocument()
    expect(screen.queryByTestId('item-row-edit-price')).not.toBeInTheDocument()
    expect(screen.getByTestId('item-row-price')).toHaveTextContent('500')
  })

  it('opens the price editor from the price itself when the listing can be re-priced', async () => {
    const onEditPrice = vi.fn()
    renderRow({ tokenId: '3' }, { withMarket: true, listing: creditsListing, onEditPrice })
    const price = screen.getByTestId('item-row-edit-price')
    expect(price).toHaveTextContent('500')
    await userEvent.click(price)
    expect(onEditPrice).toHaveBeenCalledWith(expect.objectContaining({ id: 'i1' }))
  })

  it('opens the thumbnail picker from the thumbnail of an editable item', async () => {
    const onEditThumbnail = vi.fn()
    renderRow({}, { editable: true, onEditThumbnail })
    await userEvent.click(screen.getByTestId('item-row-edit-thumbnail'))
    expect(onEditThumbnail).toHaveBeenCalledWith(expect.objectContaining({ id: 'i1' }))
  })

  it('renames the item in place and shows the name again once saved', async () => {
    const onRename = vi.fn().mockResolvedValue(undefined)
    renderRow({}, { editable: true, onRename })
    await userEvent.click(screen.getByTestId('item-row-edit-name'))
    const input = screen.getByTestId('item-row-name-input')
    expect(input).toHaveValue('Pirate Hat')
    expect(input).toHaveFocus()
    await userEvent.clear(input)
    await userEvent.type(input, '  Captain Hat {Enter}')
    expect(onRename).toHaveBeenCalledWith(expect.objectContaining({ id: 'i1' }), 'Captain Hat')
    await waitFor(() => expect(screen.queryByTestId('item-row-name-input')).not.toBeInTheDocument())
    expect(screen.getByTestId('item-row-name')).toBeInTheDocument()
  })

  it('discards the draft on Escape or cancel', async () => {
    const onRename = vi.fn()
    renderRow({}, { editable: true, onRename })
    await userEvent.click(screen.getByTestId('item-row-edit-name'))
    await userEvent.type(screen.getByTestId('item-row-name-input'), ' II{Escape}')
    expect(screen.queryByTestId('item-row-name-input')).not.toBeInTheDocument()
    expect(screen.getByTestId('item-row-name')).toHaveTextContent('Pirate Hat')

    await userEvent.click(screen.getByTestId('item-row-edit-name'))
    await userEvent.type(screen.getByTestId('item-row-name-input'), ' II')
    await userEvent.click(screen.getByTestId('item-row-name-cancel'))
    expect(screen.queryByTestId('item-row-name-input')).not.toBeInTheDocument()
    expect(onRename).not.toHaveBeenCalled()
  })

  it('saves a changed name from the confirm button', async () => {
    const onRename = vi.fn().mockResolvedValue(undefined)
    renderRow({}, { editable: true, onRename })
    await userEvent.click(screen.getByTestId('item-row-edit-name'))
    await userEvent.type(screen.getByTestId('item-row-name-input'), ' II')
    await userEvent.click(screen.getByTestId('item-row-name-save'))
    expect(onRename).toHaveBeenCalledWith(expect.anything(), 'Pirate Hat II')
    await waitFor(() => expect(screen.queryByTestId('item-row-name-input')).not.toBeInTheDocument())
  })

  it('refuses to save an invalid name and keeps editing when saving fails', async () => {
    const onRename = vi.fn().mockRejectedValue(new Error('boom'))
    renderRow({}, { editable: true, onRename })
    await userEvent.click(screen.getByTestId('item-row-edit-name'))
    const input = screen.getByTestId('item-row-name-input')
    await userEvent.type(input, ': the sequel{Enter}')
    expect(input).toHaveAttribute('data-invalid')
    expect(screen.getByTestId('item-row-name-save')).toBeDisabled()
    expect(onRename).not.toHaveBeenCalled()

    await userEvent.clear(input)
    await userEvent.type(input, 'Captain Hat{Enter}')
    expect(onRename).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(screen.getByTestId('item-row-name-input')).toBeEnabled())
    expect(screen.getByTestId('item-row-name-input')).toHaveValue('Captain Hat')
  })

  describe('validation indicator', () => {
    const error = { code: 'skeleton', severity: ValidationSeverity.ERROR, message: 'Wrong skeleton' }
    const warning = { code: 'textures', severity: ValidationSeverity.WARNING, message: 'Large texture' }

    it('flags an item with errors and opens its details on click', async () => {
      const onShowValidation = vi.fn()
      renderRow({}, { validation: { status: 'errors', issues: [error, warning] }, onShowValidation })
      expect(screen.getByTestId('item-row')).toHaveAttribute('data-validation', 'errors')
      expect(screen.getByTestId('item-row-validation')).toHaveAttribute('data-status', 'errors')
      expect(screen.getByTestId('item-row-validation')).toHaveAccessibleName(
        /This item has some issues worth fixing before publishing\.\s+Click to see details\./
      )
      await userEvent.click(screen.getByTestId('item-row-validation'))
      expect(onShowValidation).toHaveBeenCalledWith(expect.objectContaining({ id: 'i1' }))
    })

    it('flags an item with only warnings', () => {
      renderRow({}, { validation: { status: 'warnings', issues: [warning] } })
      expect(screen.getByTestId('item-row')).toHaveAttribute('data-validation', 'warnings')
      expect(screen.getByTestId('item-row-validation')).toHaveAttribute('data-status', 'warnings')
      expect(screen.getByTestId('item-row-validation')).toHaveAccessibleName(
        /This item can be improved\.\s+Click to see details\./
      )
    })

    it('says the fixes are required when errors block publishing', () => {
      renderRow({}, { validation: { status: 'errors', issues: [error] }, validationBlocks: true })
      expect(screen.getByTestId('item-row-validation')).toHaveAccessibleName(
        /This item needs some fixes before publishing\.\s+Click to see details\./
      )
    })

    it.each(['pass', 'loading', 'idle'] as const)('shows nothing while the item is %s', status => {
      renderRow({}, { validation: { status, issues: [] } })
      expect(screen.getByTestId('item-row')).not.toHaveAttribute('data-validation')
      expect(screen.queryByTestId('item-row-validation')).not.toBeInTheDocument()
    })
  })
})

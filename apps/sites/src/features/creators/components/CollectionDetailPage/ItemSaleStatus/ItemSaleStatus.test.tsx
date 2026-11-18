import { describe, it, expect, vi } from 'vitest'
import userEvent from '@testing-library/user-event'

vi.mock('~/lib/navigation', () => ({ openExternal: vi.fn() }))
import { openExternal } from '~/lib/navigation'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { type ItemListing } from '~/lib/listings'
import { ItemSaleStatus } from './ItemSaleStatus'

const listing: ItemListing = { itemId: '3', currency: 'mana', manaWei: 10n }

function renderStatus(props: Parameters<typeof ItemSaleStatus>[0]) {
  const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
  return render(<ItemSaleStatus {...props} />, { wrapper })
}

describe('ItemSaleStatus', () => {
  it('offers to put an item on sale when it has no listing', () => {
    renderStatus({ sales: { minted: 3, maxSupply: 100 }, listing: null, canSell: true, onPutOnSale: vi.fn() })
    expect(screen.getByRole('button', { name: /put on sale/i })).toBeEnabled()
  })

  it('shows a dash instead of the CTA to a viewer who may not sell', () => {
    renderStatus({ sales: { minted: 3, maxSupply: 100 }, listing: null, canSell: true })
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.getByTestId('item-sale-status')).toHaveTextContent('—')
  })

  it('disables putting an item on sale until the collection is approved for the first time', () => {
    renderStatus({ sales: { minted: 0, maxSupply: 100 }, listing: null, canSell: false, onPutOnSale: vi.fn() })
    const button = screen.getByRole('button', { name: /put on sale/i })
    expect(button).toBeDisabled()
    expect(button).toHaveAccessibleDescription(/approved/i)
  })

  it('shows the item is on sale when it has a listing', () => {
    renderStatus({ sales: { minted: 3, maxSupply: 100 }, listing })
    expect(screen.getByTestId('item-sale-status')).toHaveTextContent(/on sale/i)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('links a listed item to its Shop page', async () => {
    renderStatus({ sales: { minted: 3, maxSupply: 100 }, listing, shopUrl: 'https://shop.example/item/0xc/3' })
    const pill = screen.getByTestId('item-sale-status')
    expect(pill).toHaveAttribute('href', 'https://shop.example/item/0xc/3')
    await userEvent.click(pill)
    expect(openExternal).toHaveBeenCalledWith('https://shop.example/item/0xc/3')
  })

  it('shows sold out once the whole supply is minted, even without a listing', () => {
    renderStatus({ sales: { minted: 100, maxSupply: 100 }, listing: null })
    expect(screen.getByTestId('item-sale-status')).toHaveTextContent(/sold out/i)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('renders nothing while listings are still loading', () => {
    renderStatus({ sales: { minted: 3, maxSupply: 100 }, listing: undefined })
    expect(screen.queryByTestId('item-sale-status')).not.toBeInTheDocument()
  })
})

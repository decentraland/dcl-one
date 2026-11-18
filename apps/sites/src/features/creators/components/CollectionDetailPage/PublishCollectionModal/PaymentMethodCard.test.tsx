import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { openExternal } from '~/lib/navigation'
import { PaymentMethodCard } from './PaymentMethodCard'

vi.mock('~/lib/navigation', () => ({ openExternal: vi.fn() }))

beforeEach(() => vi.mocked(openExternal).mockClear())

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderCard(props: Partial<React.ComponentProps<typeof PaymentMethodCard>> = {}) {
  render(
    <PaymentMethodCard
      method="credits"
      price="300"
      balance="60"
      hasEnough={false}
      getMoreUrl="https://example.com/credits"
      selected={false}
      showCheckbox={false}
      compactBuy={false}
      disabled={false}
      onSelect={() => {}}
      {...props}
    />,
    { wrapper }
  )
}

describe('PaymentMethodCard', () => {
  it('offers to buy the currency by name when the balance falls short and it is the only method', async () => {
    renderCard()
    const buy = screen.getByTestId('payment-method-credits-buy')
    expect(buy).toHaveTextContent('Buy Credits')
    expect(buy).toHaveAttribute('href', 'https://example.com/credits')
    expect(screen.getByTestId('payment-method-credits-input')).toBeDisabled()
    await userEvent.click(buy)
    expect(openExternal).toHaveBeenCalledWith('https://example.com/credits')
  })

  it('buys inside the wizard instead of leaving for the shop when given an in-app action', async () => {
    const onGetMore = vi.fn()
    renderCard({ onGetMore })
    const buy = screen.getByTestId('payment-method-credits-buy')
    expect(buy).not.toHaveAttribute('href')
    await userEvent.click(buy)
    expect(onGetMore).toHaveBeenCalledTimes(1)
    expect(openExternal).not.toHaveBeenCalled()
  })

  it('shortens the label to "Buy" when several methods are shown', () => {
    renderCard({ method: 'mana', compactBuy: true, showCheckbox: true })
    expect(screen.getByTestId('payment-method-mana-buy')).toHaveTextContent(/^Buy$/)
  })

  it('shows no buy button when the balance covers the fee', () => {
    renderCard({ hasEnough: true, balance: '500' })
    expect(screen.queryByTestId('payment-method-credits-buy')).not.toBeInTheDocument()
    expect(screen.getByTestId('payment-method-credits-input')).toBeEnabled()
  })
})

describe('PaymentMethodCard selection', () => {
  it('never looks selected while the balance is insufficient', () => {
    renderCard({ selected: true, showCheckbox: true })
    expect(screen.getByTestId('payment-method-credits')).not.toHaveAttribute('data-selected')
    expect(screen.getByTestId('payment-method-credits-input')).not.toBeChecked()
  })
})

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { FeatureFlag } from '~/lib/featureFlags'
import { setFeatureFlags } from '~/test/featureFlags'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { NO_EXPIRATION, formatDateValue, minExpirationDate, parseExpirationDate } from '~/lib/sales'
import { SellItemModal } from './SellItemModal'
import { useLocale } from '~/store/locale'
import { ADDRESS, FRIEND, Providers, item, makeSession } from './testUtils'

vi.mock('~/lib/featureFlags', async () => {
  const actual = await vi.importActual<typeof import('~/lib/featureFlags')>('~/lib/featureFlags')
  const mock = await import('~/test/featureFlags')
  return { ...actual, getIsFeatureEnabled: mock.getIsFeatureEnabled }
})

beforeEach(() => setFeatureFlags(FeatureFlag.CREDITS_PRIMARY_LISTINGS))

const rate = { data: undefined as bigint | undefined }
vi.mock('~/hooks/useSales', () => ({
  useFriends: vi.fn(() => ({ data: [], isLoading: false })),
  useManaUsdRate: vi.fn(() => rate)
}))
vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))

type Props = Partial<Parameters<typeof SellItemModal>[0]>

function renderModal(props: Props = {}) {
  const onSubmit = vi.fn()
  const onClose = vi.fn()
  render(
    <SellItemModal item={item} session={makeSession()} busy={false} onSubmit={onSubmit} onClose={onClose} {...props} />,
    {
      wrapper: Providers
    }
  )
  return { onSubmit, onClose }
}

const submit = () => screen.getByTestId('sell-submit')
const price = () => screen.getByTestId('sell-price-input')

beforeEach(() => {
  vi.clearAllMocks()
  rate.data = undefined
  useLocale.setState({ locale: 'en' })
})

describe('SellItemModal', () => {
  it('shows the item with its badges and only lets a priced item be put on sale', async () => {
    const { onSubmit } = renderModal()
    expect(screen.getByTestId('sell-item-card')).toHaveTextContent('Pirate Hat')
    expect(screen.getByTestId('rarity-pill')).toHaveTextContent(/legendary/i)
    expect(screen.getByTestId('sell-item-badge-category')).toBeInTheDocument()
    expect(screen.getByTestId('sell-item-badge-body-shape')).toBeInTheDocument()
    expect(screen.getByTestId('sell-item-badge-smart')).toBeInTheDocument()
    expect(screen.queryByTestId('sell-item-badge-play-mode')).not.toBeInTheDocument()
    expect(submit()).toBeDisabled()

    await userEvent.type(price(), '5a0')
    expect(price()).toHaveValue('50')
    expect(screen.getByTestId('sell-price-usd')).toHaveTextContent('$5.00')
    expect(submit()).toBeEnabled()

    await userEvent.click(submit())
    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ price: { currency: 'credits', amount: '50' }, selfBeneficiary: true }),
      {
        price: { kind: 'credits', credits: 50 },
        beneficiary: ADDRESS,
        expiresAt: NO_EXPIRATION
      }
    )
  })

  it('lists in MANA once picked from the currency dropdown, at least 1 MANA, with an estimate from the oracle', async () => {
    rate.data = 300_000_000_000_000_000n
    const { onSubmit } = renderModal()
    await userEvent.type(price(), '50')
    await userEvent.click(screen.getByTestId('sell-price-currency'))
    await userEvent.click(screen.getByTestId('sell-price-currency-option-mana'))
    // The typed credits mean nothing in MANA: the amount starts over.
    expect(price()).toHaveValue('')
    expect(price()).toHaveAttribute('data-currency', 'mana')
    expect(screen.getByTestId('sell-note')).toHaveTextContent(/for MANA/)
    expect(screen.getByTestId('sell-price-rate')).toHaveTextContent('1 MANA ≈ $0.30')

    await userEvent.type(price(), '0.5')
    expect(screen.getByTestId('sell-price-error')).toHaveTextContent(/at least 1 MANA/)
    expect(submit()).toBeDisabled()

    await userEvent.clear(price())
    await userEvent.type(price(), '2.5x99')
    expect(price()).toHaveValue('2.59')
    expect(screen.getByTestId('sell-price-usd')).toHaveTextContent('≈ $0.78')
    expect(submit()).toBeEnabled()
    await userEvent.click(submit())
    expect(onSubmit.mock.calls[0][1]).toMatchObject({ price: { kind: 'mana', manaWei: 2_590_000_000_000_000_000n } })
  })

  it('prices in MANA only, with no currency to pick, while credit listings are off', async () => {
    setFeatureFlags()
    rate.data = 300_000_000_000_000_000n
    renderModal()
    await waitFor(() => expect(price()).toHaveAttribute('data-currency', 'mana'))
    expect(screen.getByTestId('sell-price-currency-glyph')).toBeInTheDocument()
    expect(screen.queryByTestId('sell-price-currency')).not.toBeInTheDocument()
    await userEvent.type(price(), '2.5')
    expect(screen.getByTestId('sell-price-usd')).toHaveTextContent('≈ $0.75')
    expect(submit()).toBeEnabled()
  })

  it('hides the USD estimate for MANA when the oracle cannot be read or nothing is typed yet', async () => {
    rate.data = 300_000_000_000_000_000n
    renderModal()
    await userEvent.click(screen.getByTestId('sell-price-currency'))
    await userEvent.click(screen.getByTestId('sell-price-currency-option-mana'))
    expect(screen.queryByTestId('sell-price-usd')).not.toBeInTheDocument()

    rate.data = undefined
    await userEvent.type(price(), '3')
    expect(screen.queryByTestId('sell-price-usd')).not.toBeInTheDocument()
    expect(screen.getByTestId('sell-price-rate')).toHaveTextContent(/minimum 1 MANA/i)
    expect(submit()).toBeEnabled()
  })

  it('stops a MANA price at the catalog ceiling', async () => {
    renderModal()
    await userEvent.click(screen.getByTestId('sell-price-currency'))
    await userEvent.click(screen.getByTestId('sell-price-currency-option-mana'))
    await userEvent.type(price(), '1000000000000.01')
    expect(price()).toHaveValue('1000000000000.0')
    expect(screen.getByTestId('sell-price-error')).toHaveTextContent(/at most 1,000,000,000,000 MANA/)
    expect(submit()).toBeEnabled()
  })

  it('warns when the item has edits the committee has not approved yet', () => {
    renderModal({ hasPendingChanges: true })
    expect(screen.getByTestId('sell-pending-changes')).toHaveTextContent(/last approved version/i)
  })

  it('stops the price at the ceiling the Shop can list', async () => {
    renderModal()
    await userEvent.type(price(), '10000000000009')
    expect(price()).toHaveValue('1000000000000')
    expect(screen.getByTestId('sell-price-error')).toHaveTextContent('1,000,000,000,000')
    expect(submit()).toBeEnabled()
  })

  it('drops a pasted price past the ceiling instead of truncating it', async () => {
    renderModal()
    await userEvent.click(price())
    await userEvent.paste('1000000000000500')
    expect(price()).toHaveValue('')
    expect(screen.getByTestId('sell-price-error')).toHaveTextContent('1,000,000,000,000')
  })

  it('freezes the price at 0 for a giveaway and submits it as free', async () => {
    const { onSubmit } = renderModal()
    await userEvent.click(screen.getByTestId('sell-free'))
    expect(price()).toBeDisabled()
    expect(price()).toHaveValue('0')
    expect(screen.getByTestId('sell-price-currency')).toBeDisabled()
    expect(screen.getByTestId('sell-price-usd')).toHaveTextContent('$0.00')
    await userEvent.click(submit())
    expect(onSubmit.mock.calls[0][1]).toMatchObject({ price: { kind: 'free' } })
  })

  it('asks for another beneficiary when the creator is not the one, and validates the address', async () => {
    const { onSubmit } = renderModal()
    await userEvent.type(price(), '50')
    await userEvent.click(screen.getByTestId('sell-self-beneficiary'))
    expect(submit()).toBeDisabled()

    const input = screen.getByTestId('sell-beneficiary-input')
    await userEvent.type(input, 'not-an-address')
    await userEvent.tab()
    expect(screen.getByTestId('sell-beneficiary-error')).toBeInTheDocument()
    expect(submit()).toBeDisabled()

    await userEvent.clear(input)
    await userEvent.type(input, FRIEND)
    expect(screen.getByTestId('sell-beneficiary-selected')).toHaveTextContent(FRIEND)
    expect(submit()).toBeEnabled()
    await userEvent.click(submit())
    expect(onSubmit.mock.calls[0][1]).toMatchObject({ beneficiary: FRIEND })

    await userEvent.click(screen.getByTestId('sell-beneficiary-clear'))
    expect(screen.getByTestId('sell-beneficiary-input')).toBeInTheDocument()
  })

  it('requires a future date once an expiration is set and expires at the end of that day', async () => {
    const { onSubmit } = renderModal()
    await userEvent.type(price(), '50')
    await userEvent.click(screen.getByTestId('sell-expiration-toggle'))
    const date = screen.getByTestId('sell-expiration-date')
    expect(submit()).toBeDisabled()

    // A past day is not selectable: the picker leaves the field empty.
    fireEvent.change(date, { target: { value: '01/01/2020' } })
    expect(submit()).toBeDisabled()

    const tomorrow = minExpirationDate()
    const mmddyyyy = `${String(tomorrow.getMonth() + 1).padStart(2, '0')}/${String(tomorrow.getDate()).padStart(2, '0')}/${tomorrow.getFullYear()}`
    fireEvent.change(date, { target: { value: mmddyyyy } })
    expect(date).toHaveValue(mmddyyyy)
    await userEvent.click(submit())
    expect(onSubmit.mock.calls[0][1]).toMatchObject({ expiresAt: parseExpirationDate(formatDateValue(tomorrow)) })

    // Turning the toggle off drops the date again.
    await userEvent.click(screen.getByTestId('sell-expiration-toggle'))
    await userEvent.click(submit())
    expect(onSubmit.mock.calls[1][1]).toMatchObject({ expiresAt: NO_EXPIRATION })
  })

  it('formats the expiration date in the active language', async () => {
    useLocale.setState({ locale: 'es' })
    renderModal()
    await userEvent.click(screen.getByTestId('sell-expiration-toggle'))
    const date = screen.getByTestId('sell-expiration-date')
    expect(date).toHaveAttribute('placeholder', 'DD/MM/AAAA')

    const tomorrow = minExpirationDate()
    const ddmmyyyy = `${String(tomorrow.getDate()).padStart(2, '0')}/${String(tomorrow.getMonth() + 1).padStart(2, '0')}/${tomorrow.getFullYear()}`
    fireEvent.change(date, { target: { value: ddmmyyyy } })
    expect(date).toHaveValue(ddmmyyyy)
  })

  it('restores a previous attempt and locks the form while a submit is in flight', () => {
    renderModal({
      busy: true,
      initialValues: {
        selfBeneficiary: true,
        beneficiary: '',
        price: { currency: 'credits', amount: '25' },
        free: false,
        withExpiration: false,
        expirationDate: ''
      }
    })
    expect(price()).toHaveValue('25')
    expect(screen.getByTestId('sell-item-form').querySelector('[data-busy]')).not.toBeNull()
    expect(screen.getByTestId('sell-cancel')).toBeDisabled()
    expect(submit()).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByTestId('sell-item-modal-close')).toBeDisabled()
  })
})

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { pollCreditsOrder, type CreditsOrderOutcome } from '~/lib/credits'
import { ADDRESS } from '../SellItemFlow/testUtils'
import { TopUpOutcome } from './TopUpOutcome'

vi.mock('~/lib/credits', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/credits')>()),
  pollCreditsOrder: vi.fn()
}))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
// lottie-web draws on a canvas, which jsdom lacks.
vi.mock('~/components/Confetti/LottieBurst', () => ({ default: () => null }))

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)
beforeEach(() => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ packs: [] }), { status: 200 }))
  vi.mocked(pollCreditsOrder).mockReset()
})

function renderOutcome(outcome: CreditsOrderOutcome) {
  vi.mocked(pollCreditsOrder).mockResolvedValue(outcome)
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  queryClient.setQueryData(['credits-balance', ADDRESS], { credits: 0, balanceCents: 0 })
  const balanceIsStale = () => queryClient.getQueryState(['credits-balance', ADDRESS])?.isInvalidated
  const onDone = vi.fn()
  render(
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>
        <TopUpOutcome address={ADDRESS} orderId="order-1" onDone={onDone} />
      </TranslationProvider>
    </QueryClientProvider>
  )
  return { onDone, balanceIsStale }
}

describe('TopUpOutcome', () => {
  it('confirms the credits granted and re-reads the balance once they land', async () => {
    const { onDone, balanceIsStale } = renderOutcome({ status: 'credited', creditsGranted: 260 })
    expect(screen.getByTestId('top-up-pending')).toBeInTheDocument()
    expect(await screen.findByTestId('top-up-success')).toHaveTextContent('260')
    await waitFor(() => expect(balanceIsStale()).toBe(true))

    await userEvent.click(screen.getByTestId('top-up-success-done'))
    expect(onDone).toHaveBeenCalled()
  })

  it('goes straight back to the wizard when nothing was paid, leaving the balance alone', async () => {
    const { onDone, balanceIsStale } = renderOutcome({ status: 'abandoned' })
    await waitFor(() => expect(onDone).toHaveBeenCalled())
    expect(screen.queryByTestId('top-up-success')).not.toBeInTheDocument()
    expect(screen.queryByTestId('top-up-failed')).not.toBeInTheDocument()
    expect(balanceIsStale()).toBe(false)
  })

  it('reports a failed charge and still re-reads the balance', async () => {
    const { balanceIsStale } = renderOutcome({ status: 'failed' })
    expect(await screen.findByTestId('top-up-failed')).toBeInTheDocument()
    await waitFor(() => expect(balanceIsStale()).toBe(true))
  })

  it('tells the creator the credits are on their way when the order is still settling', async () => {
    renderOutcome({ status: 'pending' })
    expect(await screen.findByTestId('top-up-pending-outcome')).toBeInTheDocument()
  })
})

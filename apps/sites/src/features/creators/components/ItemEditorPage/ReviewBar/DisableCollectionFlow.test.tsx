import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { WrongAccountError } from '~/lib/walletErrors'
import { DisableCollectionFlow } from './DisableCollectionFlow'

type Callbacks = { onSuccess: () => void; onError: (error: unknown) => void }
type Variables = { collection: Collection; onSigned: () => void }

const state = vi.hoisted(() => ({
  mutate: vi.fn(),
  showToast: vi.fn(),
  isError: false,
  isPending: false,
  error: null as Error | null
}))
vi.mock('~/hooks/useCuration', () => ({
  useDisableCollection: () => ({
    mutate: state.mutate,
    reset: vi.fn(),
    isPending: state.isPending,
    isError: state.isError,
    error: state.error
  })
}))
vi.mock('~/lib/notifications', () => ({
  useNotifications: (select: (s: { showToast: typeof state.showToast }) => unknown) =>
    select({ showToast: state.showToast })
}))

const web3 = { address: '0xme', providerType: 'injected' } as Session
const collection = { id: 'c1', name: 'Hats' } as Collection

function renderFlow(session: Session = web3) {
  const onClose = vi.fn()
  render(
    <TranslationProvider>
      <DisableCollectionFlow session={session} collection={collection} onClose={onClose} />
    </TranslationProvider>
  )
  return onClose
}

function lastCall(): [Variables, Callbacks] {
  return state.mutate.mock.lastCall as [Variables, Callbacks]
}

beforeEach(() => {
  vi.clearAllMocks()
  state.isError = false
  state.isPending = false
  state.error = null
})

describe('DisableCollectionFlow', () => {
  it('waits on the wallet, then on the transaction, and closes with a toast', () => {
    const onClose = renderFlow()
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    expect(screen.getByTestId('review-disable-signing')).toBeInTheDocument()

    act(() => lastCall()[0].onSigned())
    expect(screen.getByTestId('review-disable-pending')).toBeInTheDocument()

    act(() => lastCall()[1].onSuccess())
    expect(state.showToast).toHaveBeenCalled()
    expect(onClose).toHaveBeenCalled()
  })

  it('hides the wallet prompt when the curator backs out, and still finishes if they sign it anyway', () => {
    const onClose = renderFlow()
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    fireEvent.click(screen.getByTestId('review-disable-signing-cancel'))
    expect(screen.getByTestId('review-disable')).toBeInTheDocument()

    act(() => lastCall()[0].onSigned())
    expect(screen.getByTestId('review-disable-pending')).toBeInTheDocument()
    act(() => lastCall()[1].onSuccess())
    expect(onClose).toHaveBeenCalled()
    expect(state.mutate).toHaveBeenCalledTimes(1)
  })

  it('goes back to the confirmation when the wallet rejects', () => {
    renderFlow()
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    act(() => lastCall()[1].onError({ code: 4001 }))
    expect(screen.getByTestId('review-disable')).toBeInTheDocument()
  })

  it('asks the curator to switch the wallet back when it sits on another account', () => {
    state.isError = true
    state.error = new WrongAccountError('0xme', '0xother')
    renderFlow()
    expect(state.showToast).toHaveBeenLastCalledWith(expect.stringMatching(/switch it back to the account/i), {
      type: 'error'
    })

    state.error = new Error('execution reverted')
    renderFlow()
    expect(state.showToast).toHaveBeenLastCalledWith(expect.not.stringMatching(/switch it back/i), { type: 'error' })
  })

  it('skips the wallet prompt screen for social logins', () => {
    renderFlow({ address: '0xme', providerType: 'magic' } as Session)
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    expect(screen.queryByTestId('review-disable-signing')).not.toBeInTheDocument()
    expect(screen.getByTestId('review-disable')).toBeInTheDocument()
  })

  it('keeps the request open after backing out: no closing, and Disable shows the wallet prompt again', () => {
    const onClose = renderFlow()
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    state.isPending = true
    fireEvent.click(screen.getByTestId('review-disable-signing-cancel'))
    expect(screen.queryByTestId('review-disable-cancel')).not.toBeInTheDocument()

    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    expect(screen.getByTestId('review-disable-signing')).toBeInTheDocument()
    expect(state.mutate).toHaveBeenCalledTimes(1)
    expect(onClose).not.toHaveBeenCalled()
  })
})

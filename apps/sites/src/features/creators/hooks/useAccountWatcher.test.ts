import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook } from '@testing-library/react'
import { ProviderType } from '@dcl/schemas'
import type { Session } from '~/lib/auth'

const A = '0xaaaa000000000000000000000000000000000001'
const B = '0xbbbb000000000000000000000000000000000002'

let listener: ((...args: unknown[]) => void) | undefined
const provider = {
  on: vi.fn((event: string, cb: (...args: unknown[]) => void) => {
    if (event === 'accountsChanged') listener = cb
  }),
  removeListener: vi.fn()
}

const session = (over: Partial<Session> = {}): Session =>
  ({ address: A, providerType: ProviderType.INJECTED, provider, ...over }) as unknown as Session

const walletState: { session: Session | null } = { session: session() }
vi.mock('~/store/wallet', () => ({
  useWallet: (sel?: (s: unknown) => unknown) => (typeof sel === 'function' ? sel(walletState) : walletState)
}))

const { useAccountWatcher } = await import('./useAccountWatcher')

const reload = vi.fn()

beforeEach(() => {
  vi.clearAllMocks()
  listener = undefined
  walletState.session = session()
  // jsdom does not implement location.reload.
  Object.defineProperty(window, 'location', { value: { reload }, writable: true })
})

describe('useAccountWatcher', () => {
  it('reloads the page when the wallet switches to a different account', () => {
    renderHook(() => useAccountWatcher())
    listener?.([B.toUpperCase()])
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('reloads the page when the wallet disconnects every account', () => {
    renderHook(() => useAccountWatcher())
    listener?.([])
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('leaves the page alone when the wallet re-emits the same account', () => {
    renderHook(() => useAccountWatcher())
    listener?.([A.toUpperCase()])
    expect(reload).not.toHaveBeenCalled()
  })

  it('does not subscribe for non-injected sessions', () => {
    walletState.session = session({ providerType: ProviderType.MAGIC })
    renderHook(() => useAccountWatcher())
    expect(provider.on).not.toHaveBeenCalled()
  })

  it('unsubscribes on unmount', () => {
    const { unmount } = renderHook(() => useAccountWatcher())
    unmount()
    expect(provider.removeListener).toHaveBeenCalledWith('accountsChanged', listener)
  })
})

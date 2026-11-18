import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor, act } from '@testing-library/react'
import type { Session } from '~/lib/auth'
import { supportedChains, useWalletChain } from './useWalletChain'

const captureError = vi.fn()
vi.mock('~/lib/monitoring', () => ({ captureError: (...args: unknown[]) => captureError(...args) }))

type Listener = (...args: unknown[]) => void
type SendImpl = (method: string, params?: unknown[]) => Promise<unknown>

/** An injected wallet whose JSON-RPC answers come from `send`, seeded on `chain` at sign-in. */
function makeSession(sendImpl: SendImpl, chain = 80002) {
  const listeners: Record<string, Listener[]> = {}
  const send = vi.fn(sendImpl)
  const session = {
    address: '0xabc0000000000000000000000000000000000abc',
    chainId: chain,
    providerType: 'injected',
    provider: {
      request: ({ method, params }: { method: string; params?: unknown[] }) => send(method, params),
      on: (e: string, cb: Listener) => {
        listeners[e] = [...(listeners[e] ?? []), cb]
      },
      removeListener: (e: string, cb: Listener) => {
        listeners[e] = (listeners[e] ?? []).filter(l => l !== cb)
      }
    }
  } as unknown as Session
  return { session, listeners, send }
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('supportedChains', () => {
  it('pairs the Polygon chain with its Ethereum counterpart, testnets with testnets', () => {
    expect(supportedChains()).toEqual([80002, 11155111])
    expect(supportedChains(137)).toEqual([137, 1])
  })

  it('offers the configured chain alone when it has no counterpart', () => {
    expect(supportedChains(1)).toEqual([1])
  })
})

describe('useWalletChain', () => {
  it('reports the chain the wallet is on and follows its changes', async () => {
    let current = '0xaa36a7'
    const { session, listeners } = makeSession(async method => (method === 'eth_chainId' ? current : null))
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(result.current.chainId).toBe(11155111))

    current = '0x13882'
    act(() => listeners.chainChanged.forEach(cb => cb('0x13882')))
    await waitFor(() => expect(result.current.chainId).toBe(80002))
  })

  it('moves the wallet when the creator picks another chain', async () => {
    let current = '0x13882'
    const { session, send } = makeSession(async (method, params) => {
      if (method === 'eth_chainId') return current
      if (method === 'wallet_switchEthereumChain') current = (params as { chainId: string }[])[0].chainId
      return null
    })
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(result.current.chainId).toBe(80002))

    await act(async () => {
      await result.current.switchTo(11155111)
    })
    expect(send).toHaveBeenCalledWith('wallet_switchEthereumChain', [{ chainId: '0xaa36a7' }])
    expect(result.current.chainId).toBe(11155111)
    expect(result.current.pendingChainId).toBeUndefined()
  })

  it('adds Amoy when the wallet does not know it yet, and no other chain', async () => {
    const { session, send } = makeSession(async method => {
      if (method === 'eth_chainId') return '0xaa36a7'
      if (method === 'wallet_switchEthereumChain') throw Object.assign(new Error('Unrecognized'), { code: 4902 })
      return null
    })
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(result.current.chainId).toBe(11155111))

    await act(async () => {
      await result.current.switchTo(80002)
    })
    expect(send).toHaveBeenCalledWith('wallet_addEthereumChain', [
      expect.objectContaining({ chainId: '0x13882', chainName: 'Polygon Amoy' })
    ])
    expect(captureError).not.toHaveBeenCalled()

    const other = makeSession(async method => {
      if (method === 'eth_chainId') return '0x13882'
      if (method === 'wallet_switchEthereumChain') throw Object.assign(new Error('Unrecognized'), { code: 4902 })
      return null
    })
    const second = renderHook(() => useWalletChain(other.session))
    await waitFor(() => expect(second.result.current.chainId).toBe(80002))
    await act(async () => {
      await second.result.current.switchTo(11155111)
    })
    expect(other.send).not.toHaveBeenCalledWith('wallet_addEthereumChain', expect.anything())
    expect(captureError).toHaveBeenCalled()
    expect(second.result.current.chainId).toBe(80002)
  })

  it('stays on the real chain when the creator declines, without reporting it', async () => {
    const { session } = makeSession(async method => {
      if (method === 'eth_chainId') return '0x13882'
      if (method === 'wallet_switchEthereumChain') throw Object.assign(new Error('User rejected'), { code: 4001 })
      return null
    })
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(result.current.chainId).toBe(80002))

    await act(async () => {
      await result.current.switchTo(11155111)
    })
    expect(result.current.chainId).toBe(80002)
    expect(captureError).not.toHaveBeenCalled()
  })

  it('ignores a second request while the wallet is still answering the first', async () => {
    let release!: () => void
    const held = new Promise<void>(resolve => {
      release = resolve
    })
    const { session, send, listeners } = makeSession(async method => {
      if (method === 'eth_chainId') return '0x13882'
      if (method === 'wallet_switchEthereumChain') await held
      return null
    })
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(result.current.chainId).toBe(80002))

    let first!: Promise<void>
    await act(async () => {
      first = result.current.switchTo(11155111)
    })
    expect(result.current.pendingChainId).toBe(11155111)

    // MetaMask announces the new chain before the switch request resolves: still one request in flight.
    act(() => listeners.chainChanged.forEach(cb => cb('0xaa36a7')))
    expect(result.current.pendingChainId).toBe(11155111)

    await act(async () => {
      await result.current.switchTo(80002)
    })
    expect(send.mock.calls.filter(([m]) => m === 'wallet_switchEthereumChain')).toHaveLength(1)

    await act(async () => {
      release()
      await first
    })
    expect(result.current.pendingChainId).toBeUndefined()
  })

  it('keeps the last known chain when the wallet will not answer', async () => {
    const { session } = makeSession(async () => {
      throw new Error('wallet is asleep')
    })
    const { result } = renderHook(() => useWalletChain(session))
    await waitFor(() => expect(captureError).toHaveBeenCalled())
    expect(result.current.chainId).toBe(80002)
  })

  it('reports no chain and asks nothing without a wallet', () => {
    const { result } = renderHook(() => useWalletChain(null))
    expect(result.current.chainId).toBeUndefined()
    expect(result.current.chains).toEqual([80002, 11155111])
  })
})

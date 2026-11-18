import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ProviderType } from '@dcl/schemas'
import { type Session } from '~/lib/auth'

const analytics = vi.hoisted(() => ({ track: vi.fn(), identify: vi.fn(), reset: vi.fn() }))
vi.mock('~/lib/analytics', () => analytics)

const auth = vi.hoisted(() => ({
  restoreSession: vi.fn(),
  logout: vi.fn().mockResolvedValue(undefined),
  signInRedirect: vi.fn()
}))
vi.mock('~/lib/auth', () => auth)

const monitoring = vi.hoisted(() => ({ setMonitoringUser: vi.fn() }))
vi.mock('~/lib/monitoring', () => monitoring)

const session = {
  address: '0xcreator',
  chainId: 137,
  providerType: ProviderType.INJECTED,
  walletName: 'Trust Wallet'
} as unknown as Session

/** The store dedupes restores in module state, so each case needs its own copy. */
async function loadStore() {
  vi.resetModules()
  return (await import('./wallet')).useWallet
}

beforeEach(() => {
  vi.clearAllMocks()
  auth.restoreSession.mockResolvedValue(session)
})

describe('restore', () => {
  it('identifies the creator and reports the connection the way the legacy builder does', async () => {
    const useWallet = await loadStore()

    await useWallet.getState().restore()

    expect(analytics.identify).toHaveBeenCalledWith('0xcreator', {
      ethAddress: '0xcreator',
      chainId: 137,
      provider_type: ProviderType.INJECTED
    })
    expect(analytics.track).toHaveBeenCalledWith('Connect Wallet', {
      address: '0xcreator',
      chainId: 137,
      providerType: ProviderType.INJECTED,
      walletName: 'Trust Wallet'
    })
    expect(monitoring.setMonitoringUser).toHaveBeenCalledWith('0xcreator')
  })

  it('reports nothing for a visitor with no session to restore', async () => {
    auth.restoreSession.mockResolvedValue(null)
    const useWallet = await loadStore()

    await useWallet.getState().restore()

    expect(analytics.identify).not.toHaveBeenCalled()
    expect(analytics.track).not.toHaveBeenCalled()
  })

  it('bounds and sanitises the name the wallet reports about itself', async () => {
    auth.restoreSession.mockResolvedValue({ ...session, walletName: `Trust\u0000\u202eWallet${'!'.repeat(80)}` })
    const useWallet = await loadStore()

    await useWallet.getState().restore()

    const props = analytics.track.mock.calls.find(([event]) => event === 'Connect Wallet')?.[1] as {
      walletName: string
    }
    expect(props.walletName).toBe(`TrustWallet${'!'.repeat(53)}`)
    expect(props.walletName).toHaveLength(64)
  })

  it('exposes the signed-in address to analytics and Sentry', async () => {
    const useWallet = await loadStore()
    const { currentAddress } = await import('~/lib/currentAddress')

    expect(currentAddress()).toBeUndefined()
    await useWallet.getState().restore()

    expect(currentAddress()).toBe('0xcreator')
  })
})

describe('disconnect', () => {
  it('drops the analytics identity so the next account is not attributed to this one', async () => {
    const useWallet = await loadStore()
    await useWallet.getState().restore()

    await useWallet.getState().disconnect()

    expect(analytics.track).toHaveBeenCalledWith('Logout')
    expect(analytics.reset).toHaveBeenCalled()
    expect(monitoring.setMonitoringUser).toHaveBeenLastCalledWith(null)
  })
})

describe('signIn', () => {
  it('records the attempt before leaving for the auth app', async () => {
    const useWallet = await loadStore()

    useWallet.getState().signIn()

    expect(analytics.track).toHaveBeenCalledWith('Login')
    expect(auth.signInRedirect).toHaveBeenCalled()
  })
})

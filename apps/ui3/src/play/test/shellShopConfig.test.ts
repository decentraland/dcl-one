// Which CreditsManager an in-world purchase may spend through (shell/shop/config): the Shop address book's,
// so the same one the web checkout spends through. The book names the deployment's own when the build names
// it (VITE_SHOP_CREDITS_MANAGER on the chain VITE_SHOP_CHAIN_ID names), else the chain's registry one; none
// on a chain without a manager.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { chainContracts, requireContracts } from '../../shop/checkout/chain/contracts'
import { trustedCreditsManager } from '../shell/shop/config'

const DEDICATED = '0x' + 'De'.repeat(20)
const registry = (chainId: number): string => chainContracts(chainId as never)!.creditsManager!.address.toLowerCase()

afterEach(() => {
  vi.unstubAllEnvs()
})

describe('the trusted CreditsManager', () => {
  it('is the deployment’s on the chain the build names, lowercased, with the manager’s EIP-712 domain', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', ` ${DEDICATED} `)
    const book = requireContracts(137 as never).creditsManager!
    expect(trustedCreditsManager(137)).toEqual({ chainId: 137, address: DEDICATED.toLowerCase(), name: book.name, version: book.version })
  })

  it('is the manager the web checkout spends through, so the two never disagree', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '80002')
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', DEDICATED)
    for (const chainId of [137, 80002]) {
      expect(trustedCreditsManager(chainId)?.address).toBe(requireContracts(chainId as never).creditsManager!.address.toLowerCase())
    }
    expect(trustedCreditsManager(80002)?.address).toBe(DEDICATED.toLowerCase())
    expect(trustedCreditsManager(137)?.address).toBe(registry(137))
  })

  it('is the chain’s registry manager when the deployment names none', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', '')
    expect(trustedCreditsManager(137)?.address).toBe(registry(137))
  })

  it('is the registry manager when the build names the manager but not its chain', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '')
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', DEDICATED)
    expect(trustedCreditsManager(137)?.address).toBe(registry(137))
  })

  it('never trusts a malformed address: the registry manager stands', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
    for (const value of ['0x1234', 'not an address', `${DEDICATED}00`]) {
      vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', value)
      expect(trustedCreditsManager(137)?.address).toBe(registry(137))
    }
  })

  it('is none on a chain without a CreditsManager', () => {
    vi.stubEnv('VITE_SHOP_CHAIN_ID', '137')
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', DEDICATED)
    expect(trustedCreditsManager(1)).toBeNull()
    expect(trustedCreditsManager(424242)).toBeNull()
  })
})

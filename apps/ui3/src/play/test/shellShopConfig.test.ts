// Which CreditsManager an in-world purchase may spend through (shell/shop/config): the deployment's,
// named by VITE_SHOP_CREDITS_MANAGER, else the chain's registry one; none on a chain without a manager.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { chainContracts } from '../../shop/checkout/chain/contracts'
import { trustedCreditsManager } from '../shell/shop/config'

const DEDICATED = '0x' + 'De'.repeat(20)
const registry = (chainId: number): string => chainContracts(chainId as never)!.creditsManager!.address.toLowerCase()

afterEach(() => {
  vi.unstubAllEnvs()
})

describe('the trusted CreditsManager', () => {
  it('is the deployment’s, lowercased, with the manager’s EIP-712 domain', () => {
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', ` ${DEDICATED} `)
    const book = chainContracts(137 as never)!.creditsManager!
    expect(trustedCreditsManager(137)).toEqual({ chainId: 137, address: DEDICATED.toLowerCase(), name: book.name, version: book.version })
  })

  it('is the chain’s registry manager when the deployment names none', () => {
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', '')
    expect(trustedCreditsManager(137)?.address).toBe(registry(137))
  })

  it('never trusts a malformed address: the registry manager stands', () => {
    for (const value of ['0x1234', 'not an address', `${DEDICATED}00`]) {
      vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', value)
      expect(trustedCreditsManager(137)?.address).toBe(registry(137))
    }
  })

  it('is none on a chain without a CreditsManager', () => {
    vi.stubEnv('VITE_SHOP_CREDITS_MANAGER', DEDICATED)
    expect(trustedCreditsManager(1)).toBeNull()
    expect(trustedCreditsManager(424242)).toBeNull()
  })
})

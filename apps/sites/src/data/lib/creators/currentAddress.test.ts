import { beforeEach, describe, expect, it } from 'vitest'
import { currentAddress, setCurrentAddressReader } from './currentAddress'

beforeEach(() => setCurrentAddressReader(() => undefined))

describe('currentAddress', () => {
  it('is undefined until a reader is registered', () => {
    expect(currentAddress()).toBeUndefined()
  })

  it('reads the address at call time, not at registration time', () => {
    const wallet: { address?: string } = {}
    setCurrentAddressReader(() => wallet.address)
    expect(currentAddress()).toBeUndefined()

    wallet.address = '0xcreator'
    expect(currentAddress()).toBe('0xcreator')
  })

  it('reports no address rather than throwing when the reader fails', () => {
    setCurrentAddressReader(() => {
      throw new Error('store not ready')
    })
    expect(currentAddress()).toBeUndefined()
  })
})

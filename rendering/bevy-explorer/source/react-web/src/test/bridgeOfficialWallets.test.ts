// The official wallets: the feature flags' `alfa-official-wallets` variant, its first CSV line, read
// once.
import { describe, it, expect, vi, beforeEach } from 'vitest'

const { getJson, serviceBase } = vi.hoisted(() => ({ getJson: vi.fn(), serviceBase: vi.fn(async () => 'https://feature-flags.example') }))
vi.mock('../../bridge-scene/src/http', () => ({ getJson, serviceBase }))

const doc = (variant: { name?: string; payload?: { type?: string; value?: string } }) => ({ flags: { 'explorer-alfa-official-wallets': true }, variants: { 'explorer-alfa-official-wallets': variant } })

beforeEach(() => {
  vi.resetModules()
  getJson.mockReset()
})

describe('official wallets', () => {
  it("reads the first line of the `wallets` csv, lowercased and without empty entries", async () => {
    const { parseOfficialWallets } = await import('../../bridge-scene/src/officialWallets')
    expect(parseOfficialWallets(doc({ name: 'wallets', payload: { type: 'CSV', value: '0xAa,,0xBb\r\n0xCc' } }))).toEqual(new Set(['0xaa', '0xbb']))
  })

  it('marks no one for another variant, another payload type or an empty list', async () => {
    const { parseOfficialWallets } = await import('../../bridge-scene/src/officialWallets')
    expect(parseOfficialWallets(doc({ name: 'other', payload: { type: 'csv', value: '0xaa' } })).size).toBe(0)
    expect(parseOfficialWallets(doc({ name: 'wallets', payload: { type: 'string', value: '0xaa' } })).size).toBe(0)
    expect(parseOfficialWallets(doc({ name: 'wallets', payload: { type: 'csv', value: '' } })).size).toBe(0)
    expect(parseOfficialWallets(undefined).size).toBe(0)
  })

  it("reads the deployment's explorer.json once", async () => {
    getJson.mockResolvedValue(doc({ name: 'wallets', payload: { type: 'csv', value: '0xaa' } }))
    const { officialWallets } = await import('../../bridge-scene/src/officialWallets')
    expect(await officialWallets()).toEqual(new Set(['0xaa']))
    await officialWallets()
    expect(getJson).toHaveBeenCalledTimes(1)
    expect(getJson).toHaveBeenCalledWith('https://feature-flags.example/explorer.json')
  })

  it('a failed read marks no one and is tried again a minute later', async () => {
    getJson.mockRejectedValueOnce(new Error('HTTP 500')).mockResolvedValue(doc({ name: 'wallets', payload: { type: 'csv', value: '0xaa' } }))
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const { officialWallets } = await import('../../bridge-scene/src/officialWallets')
    expect((await officialWallets(1000)).size).toBe(0)
    expect((await officialWallets(30_000)).size).toBe(0)
    expect(getJson).toHaveBeenCalledTimes(1)
    expect(await officialWallets(61_000)).toEqual(new Set(['0xaa']))
  })
})

// What may cross into the shell's Donate (lib/donateProtocol) and what a tip may ask of the wallet
// (shell/donate/guard). The typed data is built by the Shop's own meta-transaction sender, so the guard is
// tested against exactly what Donate will send.
import { describe, expect, it, vi } from 'vitest'
import { encodeFunctionData, parseAbi, parseUnits } from 'viem'
import { MANA_TOKEN } from '@shop/market/lib/contracts'
import { metaTransactionTypedData } from '../../shop/market/lib/metaTx'
import { signTypedData } from '../../shop/market/lib/tx'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { DONATE_PROTOCOL, isDonateEvent, parseDonateCapabilities, parseDonateRequest } from '../lib/donateProtocol'
import { guardedDonationProvider, type DonationIntent } from '../shell/donate/guard'

const FROM = '0x' + 'aa'.repeat(20)
const CREATOR = '0x' + 'bb'.repeat(20)
const OTHER = '0x' + 'cc'.repeat(20)
const CHAIN = 137
const TOKEN = MANA_TOKEN[CHAIN]!
const WEI = parseUnits('166', 18)
const TRANSFER = parseAbi(['function transfer(address to, uint256 amount) returns (bool)'])
const transfer = (to: string, wei: bigint): `0x${string}` => encodeFunctionData({ abi: TRANSFER, functionName: 'transfer', args: [to as `0x${string}`, wei] })

describe('the donate.open request', () => {
  it('admits a parcel or a World, and a display hint', () => {
    expect(parseDonateRequest({ v: 1, place: { parcel: { x: -10, y: 20 } } })).toEqual({ v: 1, place: { parcel: { x: -10, y: 20 } } })
    expect(parseDonateRequest({ v: 1, place: { world: 'Lorux0r.dcl.eth' }, input: 'keyboard' })).toEqual({ v: 1, place: { world: 'lorux0r.dcl.eth' }, input: 'keyboard' })
  })

  it.each([
    ['a recipient', { v: 1, place: { parcel: { x: 0, y: 0 } }, to: CREATOR }],
    ['an amount', { v: 1, place: { parcel: { x: 0, y: 0 } }, amount: 166 }],
    ['another version', { v: 2, place: { parcel: { x: 0, y: 0 } } }],
    ['a parcel and a World', { v: 1, place: { parcel: { x: 0, y: 0 }, world: 'a.dcl.eth' } }],
    ['a parcel off the map', { v: 1, place: { parcel: { x: 1001, y: 0 } } }],
    ['part of a parcel', { v: 1, place: { parcel: { x: 0.5, y: 0 } } }],
    ['a field in a parcel', { v: 1, place: { parcel: { x: 0, y: 0, z: 1 } } }],
    ['a World that is not a name', { v: 1, place: { world: 'https://evil.example' } }],
    ['another input', { v: 1, place: { parcel: { x: 0, y: 0 } }, input: 'touch' }]
  ])('refuses %s', (_, request) => {
    expect(() => parseDonateRequest(request)).toThrow()
  })

  it('reads anything but a valid capabilities answer as unavailable', () => {
    expect(parseDonateCapabilities({ v: DONATE_PROTOCOL, available: true }).available).toBe(true)
    expect(parseDonateCapabilities(undefined).available).toBe(false)
    expect(parseDonateCapabilities({ v: 2, available: true }).available).toBe(false)
    expect(isDonateEvent({ type: 'bevy-shell:donate', v: 1, state: 'sent' })).toBe(true)
    expect(isDonateEvent({ type: 'bevy-shell:donate', v: 1, state: 'other' })).toBe(false)
  })
})

function fakeWallet(account = FROM, chainId = CHAIN) {
  const calls: { method: string; params?: unknown }[] = []
  const provider: Eip1193Provider = {
    request: vi.fn(async ({ method, params }: { method: string; params?: unknown }) => {
      calls.push({ method, params })
      if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [account]
      if (method === 'eth_chainId') return `0x${chainId.toString(16)}`
      if (method === 'eth_signTypedData_v4') return '0x' + '11'.repeat(65)
      if (method === 'eth_sendTransaction') return '0x' + '22'.repeat(32)
      return null
    }) as Eip1193Provider['request']
  }
  return { provider, calls, reached: (method: string) => calls.some((c) => c.method === method) }
}

const intent = (over: Partial<DonationIntent> = {}): DonationIntent => ({
  from: FROM,
  chainId: CHAIN,
  token: { address: TOKEN.address.toLowerCase(), name: TOKEN.name, version: TOKEN.version },
  to: CREATOR,
  wei: WEI,
  current: () => true,
  ...over
})

const typedFor = (functionData: `0x${string}`, contract = TOKEN, from = FROM) => metaTransactionTypedData(contract, 'collection', from, 7n, functionData)

describe('the tip wallet', () => {
  it('signs the token meta-transaction of exactly the confirmed transfer', async () => {
    const w = fakeWallet()
    await expect(signTypedData(guardedDonationProvider(w.provider, intent()), FROM, typedFor(transfer(CREATOR, WEI)))).resolves.toMatch(/^0x/)
    expect(w.reached('eth_signTypedData_v4')).toBe(true)
  })

  it.each([
    ['another recipient', () => typedFor(transfer(OTHER, WEI))],
    ['another amount', () => typedFor(transfer(CREATOR, WEI + 1n))],
    ['another call', () => typedFor(encodeFunctionData({ abi: parseAbi(['function approve(address,uint256)']), functionName: 'approve', args: [CREATOR as `0x${string}`, WEI] }))],
    ['trailing bytes', () => typedFor(`${transfer(CREATOR, WEI)}00` as `0x${string}`)],
    ['another token', () => typedFor(transfer(CREATOR, WEI), { ...TOKEN, address: OTHER as typeof TOKEN.address })],
    ['another chain', () => typedFor(transfer(CREATOR, WEI), { ...TOKEN, chainId: 80002 as typeof TOKEN.chainId })],
    ['another signer', () => typedFor(transfer(CREATOR, WEI), TOKEN, OTHER)]
  ])('refuses to sign %s', async (_, build) => {
    const w = fakeWallet()
    await expect(signTypedData(guardedDonationProvider(w.provider, intent()), FROM, build())).rejects.toThrow()
    expect(w.reached('eth_signTypedData_v4')).toBe(false)
  })

  it('sends the confirmed transfer directly, with no value, on the tip chain only', async () => {
    const tx = { from: FROM, to: TOKEN.address, data: transfer(CREATOR, WEI) }
    const ok = fakeWallet()
    await expect(guardedDonationProvider(ok.provider, intent()).request({ method: 'eth_sendTransaction', params: [tx] })).resolves.toMatch(/^0x/)
    for (const bad of [{ ...tx, value: '0x1' }, { ...tx, to: OTHER }, { ...tx, data: transfer(OTHER, WEI) }, { ...tx, nonce: '0x1' }]) {
      const w = fakeWallet()
      await expect(guardedDonationProvider(w.provider, intent()).request({ method: 'eth_sendTransaction', params: [bad] })).rejects.toThrow()
      expect(w.reached('eth_sendTransaction')).toBe(false)
    }
    const elsewhere = fakeWallet(FROM, 1)
    await expect(guardedDonationProvider(elsewhere.provider, intent()).request({ method: 'eth_sendTransaction', params: [tx] })).rejects.toThrow(/another network/)
    expect(elsewhere.reached('eth_sendTransaction')).toBe(false)
  })

  it('refuses once the sign-in or the wallet account changed', async () => {
    const moved = fakeWallet(OTHER)
    await expect(signTypedData(guardedDonationProvider(moved.provider, intent()), FROM, typedFor(transfer(CREATOR, WEI)))).rejects.toThrow()
    expect(moved.reached('eth_signTypedData_v4')).toBe(false)
    const stale = fakeWallet()
    await expect(signTypedData(guardedDonationProvider(stale.provider, intent({ current: () => false })), FROM, typedFor(transfer(CREATOR, WEI)))).rejects.toThrow(/sign-in changed/)
    expect(stale.reached('eth_signTypedData_v4')).toBe(false)
  })

  it('refuses anything else, and switches only to the tip chain', async () => {
    const w = fakeWallet()
    const p = guardedDonationProvider(w.provider, intent())
    await expect(p.request({ method: 'personal_sign', params: ['0x00', FROM] })).rejects.toThrow()
    await expect(p.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] })).rejects.toThrow()
    await expect(p.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x89' }] })).resolves.toBeNull()
    expect(w.calls.filter((c) => c.method === 'personal_sign')).toHaveLength(0)
  })
})

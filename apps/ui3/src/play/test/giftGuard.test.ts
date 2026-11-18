// What a gift may ask of the wallet (shell/gift/guard). The typed data is built by the Shop's own meta-transaction
// sender, so the guard is tested against exactly what Gift will send.
import { describe, expect, it, vi } from 'vitest'
import { encodeFunctionData, parseAbi } from 'viem'
import { collectionV2, ERC721_ABI } from '@shop/market/lib/contracts'
import { metaTransactionTypedData } from '../../shop/market/lib/metaTx'
import { signTypedData } from '../../shop/market/lib/tx'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { guardedGiftProvider, type GiftIntent } from '../shell/gift/guard'

const FROM = '0x' + 'aa'.repeat(20)
const TO = '0x' + 'bb'.repeat(20)
const OTHER = '0x' + 'cc'.repeat(20)
const CONTRACT = '0x' + 'dd'.repeat(20)
const CHAIN = 137
const COLLECTION = collectionV2(CHAIN, CONTRACT)
const transferFrom = (from: string, to: string, id: bigint): `0x${string}` =>
  encodeFunctionData({ abi: ERC721_ABI, functionName: 'transferFrom', args: [from as `0x${string}`, to as `0x${string}`, id] })

function fakeWallet(account = FROM) {
  const calls: string[] = []
  const provider: Eip1193Provider = {
    request: vi.fn(async ({ method }: { method: string }) => {
      calls.push(method)
      if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [account]
      if (method === 'eth_chainId') return '0x89'
      if (method === 'eth_signTypedData_v4') return '0x' + '11'.repeat(65)
      return null
    }) as Eip1193Provider['request']
  }
  return { provider, reached: (m: string) => calls.includes(m) }
}

const intent = (over: Partial<GiftIntent> = {}): GiftIntent => ({
  from: FROM,
  chainId: CHAIN,
  collection: { address: CONTRACT, name: COLLECTION.name, version: COLLECTION.version },
  to: TO,
  tokenId: 105n,
  current: () => true,
  ...over
})
const typedFor = (data: `0x${string}`, contract = COLLECTION, from = FROM) => metaTransactionTypedData(contract, 'collection', from, 7n, data)

describe('the gift wallet', () => {
  it("signs the collection's meta-transaction of exactly that token to that recipient", async () => {
    const w = fakeWallet()
    await expect(signTypedData(guardedGiftProvider(w.provider, intent()), FROM, typedFor(transferFrom(FROM, TO, 105n)))).resolves.toMatch(/^0x/)
    expect(w.reached('eth_signTypedData_v4')).toBe(true)
  })

  it.each([
    ['another recipient', () => typedFor(transferFrom(FROM, OTHER, 105n))],
    ['another token', () => typedFor(transferFrom(FROM, TO, 106n))],
    ['from another account', () => typedFor(transferFrom(OTHER, TO, 105n))],
    ['another call', () => typedFor(encodeFunctionData({ abi: parseAbi(['function setApprovalForAll(address,bool)']), functionName: 'setApprovalForAll', args: [TO as `0x${string}`, true] }))],
    ['trailing bytes', () => typedFor(`${transferFrom(FROM, TO, 105n)}00` as `0x${string}`)],
    ['another collection', () => typedFor(transferFrom(FROM, TO, 105n), collectionV2(CHAIN, OTHER))],
    ['another chain', () => typedFor(transferFrom(FROM, TO, 105n), collectionV2(80002, CONTRACT))],
    ['another signer', () => typedFor(transferFrom(FROM, TO, 105n), COLLECTION, OTHER)]
  ])('refuses to sign %s', async (_, build) => {
    const w = fakeWallet()
    await expect(signTypedData(guardedGiftProvider(w.provider, intent()), FROM, build())).rejects.toThrow()
    expect(w.reached('eth_signTypedData_v4')).toBe(false)
  })

  it('signs only the pinned nonce on TRY AGAIN', async () => {
    await expect(signTypedData(guardedGiftProvider(fakeWallet().provider, intent({ nonce: 7n })), FROM, typedFor(transferFrom(FROM, TO, 105n)))).resolves.toMatch(/^0x/)
    const w = fakeWallet()
    await expect(signTypedData(guardedGiftProvider(w.provider, intent({ nonce: 6n })), FROM, typedFor(transferFrom(FROM, TO, 105n)))).rejects.toThrow()
    expect(w.reached('eth_signTypedData_v4')).toBe(false)
  })

  it('drops a signature that comes back once the gift is no longer current', async () => {
    let current = true
    const w = fakeWallet()
    const wallet: Eip1193Provider = {
      request: async (a) => {
        const r = await w.provider.request(a)
        if (a.method === 'eth_signTypedData_v4') current = false
        return r
      }
    }
    await expect(signTypedData(guardedGiftProvider(wallet, intent({ current: () => current })), FROM, typedFor(transferFrom(FROM, TO, 105n)))).rejects.toThrow()
  })

  it('never lets the wallet send a transaction itself, or sign once the sign-in changed', async () => {
    const w = fakeWallet()
    await expect(guardedGiftProvider(w.provider, intent()).request({ method: 'eth_sendTransaction', params: [{ from: FROM, to: CONTRACT, data: transferFrom(FROM, TO, 105n) }] })).rejects.toThrow()
    expect(w.reached('eth_sendTransaction')).toBe(false)
    const stale = fakeWallet()
    await expect(signTypedData(guardedGiftProvider(stale.provider, intent({ current: () => false })), FROM, typedFor(transferFrom(FROM, TO, 105n)))).rejects.toThrow(/sign-in changed/)
    expect(stale.reached('eth_signTypedData_v4')).toBe(false)
  })
})

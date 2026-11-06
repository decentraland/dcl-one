// What may cross into the shell's purchase (lib/shopProtocol) and what the purchase may ask of the wallet
// (shell/shop/walletGuard).
import { describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { chainIdSalt } from '../../shop/checkout/chain/metatx'
import { parseBuyRequest, parseOperationEvent, SHOP_EVENT } from '../lib/shopProtocol'
import { guardedProvider } from '../shell/shop/walletGuard'

const BUYER = '0x' + 'b0'.repeat(20)
const OTHER = '0x' + 'cc'.repeat(20)
const MANAGER = '0x' + 'ce'.repeat(20)
const CHAIN = 80002

describe('the shop.buy request', () => {
  const ok = { v: 1, item: { contract: '0x' + 'AB'.repeat(20), itemId: '7' }, method: 'credits' }

  it('admits an item reference, lowercased, and a cleaned display name', () => {
    expect(parseBuyRequest({ ...ok, creatorName: ' Aurora\u0007 ' })).toEqual({ v: 1, item: { contract: '0x' + 'ab'.repeat(20), itemId: '7' }, method: 'credits', creatorName: 'Aurora' })
    expect(parseBuyRequest({ ...ok, creatorName: 'x'.repeat(200) }).creatorName).toHaveLength(64)
  })

  it.each([
    ['another field', { ...ok, buyer: BUYER }],
    ['a field in the item', { ...ok, item: { ...ok.item, tradeId: 'x' } }],
    ['another version', { ...ok, v: 2 }],
    ['a bad contract', { ...ok, item: { contract: '0x12', itemId: '7' } }],
    ['a bad item id', { ...ok, item: { contract: ok.item.contract, itemId: '07' } }],
    ['an item id past uint256', { ...ok, item: { contract: ok.item.contract, itemId: '1'.repeat(79) } }],
    ['MANA', { ...ok, method: 'mana' }],
    ['nothing', null]
  ])('refuses %s', (_, params) => {
    expect(() => parseBuyRequest(params)).toThrow()
  })

  it('reads only well-formed operation events', () => {
    const event = { type: SHOP_EVENT, v: 1, operation: 'a', state: 'succeeded', item: { contract: '0x1', itemId: '7' }, next: 'backpack', txHash: '0xabc' }
    expect(parseOperationEvent(event)).toMatchObject({ state: 'succeeded', next: 'backpack', txHash: '0xabc' })
    expect(parseOperationEvent({ ...event, state: 'exploded' })).toBeNull()
    expect(parseOperationEvent({ ...event, v: 2 })).toBeNull()
  })
})

describe('the purchase wallet', () => {
  function guarded(accounts: string[] = [BUYER]) {
    const request = vi.fn(async ({ method }: { method: string }) => (method === 'eth_accounts' || method === 'eth_requestAccounts' ? accounts : '0xresult'))
    const provider: Eip1193Provider = { request: request as Eip1193Provider['request'] }
    return { request, wallet: guardedProvider(provider, BUYER, { chainId: CHAIN, targets: [MANAGER] }) }
  }
  const typed = (over: Record<string, unknown> = {}, domain: Record<string, unknown> = {}) =>
    JSON.stringify({ primaryType: 'MetaTransaction', domain: { name: 'Decentraland Credits', version: '1.0.0', verifyingContract: MANAGER, salt: chainIdSalt(CHAIN), ...domain }, message: { from: BUYER, nonce: '0' }, ...over })

  it('answers the account only when the wallet is on the login’s', async () => {
    expect(await guarded().wallet.request({ method: 'eth_requestAccounts' })).toEqual([BUYER])
    await expect(guarded([OTHER]).wallet.request({ method: 'eth_accounts' })).rejects.toMatchObject({ code: 4100 })
  })

  it('signs the Shop’s meta-transaction for the login, on its chain, for its contract', async () => {
    const { wallet, request } = guarded()
    await expect(wallet.request({ method: 'eth_signTypedData_v4', params: [BUYER, typed()] })).resolves.toBe('0xresult')
    expect(request).toHaveBeenCalledTimes(1)
  })

  it.each([
    ['for another account', [OTHER, typed()]],
    ['another contract', [BUYER, typed({}, { verifyingContract: OTHER })]],
    ['another chain', [BUYER, typed({}, { salt: chainIdSalt(137) })]],
    ['another kind of message', [BUYER, typed({ primaryType: 'Permit' })]],
    ['a message from someone else', [BUYER, typed({ message: { from: OTHER } })]],
    ['what it can’t read', [BUYER, '{']]
  ])('refuses to sign %s', async (_, params) => {
    const { wallet, request } = guarded()
    await expect(wallet.request({ method: 'eth_signTypedData_v4', params })).rejects.toMatchObject({ code: 4100 })
    expect(request).not.toHaveBeenCalled()
  })

  it('sends only to its contract, from the login, with no value', async () => {
    const { wallet, request } = guarded()
    await expect(wallet.request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANAGER, data: '0x' }] })).resolves.toBe('0xresult')
    for (const tx of [{ from: OTHER, to: MANAGER }, { from: BUYER, to: OTHER }, { from: BUYER, to: MANAGER, value: '0x1' }]) {
      await expect(wallet.request({ method: 'eth_sendTransaction', params: [tx] })).rejects.toMatchObject({ code: 4100 })
    }
    expect(request).toHaveBeenCalledTimes(1)
  })

  it('switches only to its own network, and uses nothing else', async () => {
    const { wallet } = guarded()
    await expect(wallet.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x13882' }] })).resolves.toBe('0xresult')
    await expect(wallet.request({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x1' }] })).rejects.toMatchObject({ code: 4100 })
    for (const method of ['personal_sign', 'eth_sign', 'eth_signTypedData', 'wallet_watchAsset']) {
      await expect(wallet.request({ method, params: [] })).rejects.toMatchObject({ code: 4200 })
    }
  })
})

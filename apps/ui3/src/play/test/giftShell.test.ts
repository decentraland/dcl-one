// The web shell's side of Gift (shell/gift/gifting): who may gift, what is checked before signing, and what a send's
// outcome means. Reads, the relay and the chain are fakes here; the guard is the real one, so a send that reaches the
// fake wallet is one the guard let through.
import { describe, expect, it, vi } from 'vitest'
import type { Eip1193Provider } from '../../data/auth/wallet'
import type { collectionV2 } from '@shop/market/lib/contracts'
import { metaTransactionTypedData, MetaTransactionError } from '../../shop/market/lib/metaTx'
import { signTypedData } from '../../shop/market/lib/tx'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { isShellError } from '@explorer-core/lib/shellReply'
import type { GiftTransferHandlers, GiftTransferView } from '@hud/features/gifting/GiftTransferPanel'
import { GIFT_EVENT, GIFT_UNAVAILABLE } from '../lib/giftProtocol'
import { createShellGift, tokenParts, type ShellGiftServices } from '../shell/gift/gifting'
import { guardedGiftProvider } from '../shell/gift/guard'
import type { ShellShopWallet } from '../shell/shop/purchase'

const SENDER = '0x' + 'aa'.repeat(20)
const RECIPIENT = '0x' + 'bb'.repeat(20)
const CONTRACT = '0x' + 'dd'.repeat(20)
const URN = `urn:decentraland:matic:collections-v2:${CONTRACT}:0:105`
const HASH = '0x' + 'ab'.repeat(32)
const EXPIRES = new Date(Date.now() + 3_600_000).toISOString()
const LOGIN: StoredLogin = {
  address: SENDER,
  identity: {
    ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
    expiration: EXPIRES,
    authChain: [
      { type: 'SIGNER', payload: SENDER, signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0x' }
    ]
  }
}
const OPEN = { v: 1, recipient: RECIPIENT, urn: URN, tokenId: '105', transferredAt: 1700000000 }

function fakeProvider() {
  const calls: string[] = []
  const provider: Eip1193Provider = {
    request: vi.fn(async ({ method }: { method: string }) => {
      calls.push(method)
      if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [SENDER]
      if (method === 'eth_chainId') return '0x89'
      if (method === 'eth_signTypedData_v4') return '0x' + '11'.repeat(65)
      return null
    }) as Eip1193Provider['request']
  }
  return { provider, signed: () => calls.includes('eth_signTypedData_v4') }
}

/** The gasless send as the Shop's sender makes it: the collection's meta-transaction signed through the wallet given. */
const signThen = (after: () => Promise<string>) => async (p: Eip1193Provider, c: ReturnType<typeof collectionV2>, data: `0x${string}`, nonce?: bigint) => {
  await signTypedData(p, SENDER, metaTransactionTypedData(c, 'collection', SENDER, nonce ?? 0n, data))
  return after()
}

function setup(over: Partial<ShellGiftServices> = {}, opts: { login?: StoredLogin | null; kind?: 'web3' | 'managed' | null; confirm?: boolean } = {}) {
  const views: GiftTransferView[] = []
  let handlers: GiftTransferHandlers | null = null
  const posts: unknown[] = []
  const opened: string[] = []
  const store = new Map<string, string>()
  const { provider, signed } = fakeProvider()
  const kind = opts.kind === undefined ? 'web3' : opts.kind
  const wallet: ShellShopWallet = { kind: () => kind, provider: () => (kind ? provider : null) }
  let owner = SENDER
  const services: Partial<ShellGiftServices> = {
    enabled: async () => true,
    chainId: () => 137,
    token: async (contractAddress, tokenId) => ({ contractAddress, tokenId, owner: SENDER, name: 'Cool Hat', image: 'https://img.test/hat.png', rarity: 'Epic', category: 'wearable', chainId: 137 }),
    recipient: async (address) => ({ address, name: 'Bob', nameColor: '#ff0000', face: 'javascript:alert(1)' }),
    ownerOf: async () => owner,
    nonce: async () => 7n,
    sendGasless: signThen(async () => HASH),
    claim: async () => () => undefined,
    locksAvailable: () => true,
    storage: () => ({ getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v) }) as unknown as Storage,
    sleep: async () => undefined,
    now: () => 1_000_000,
    activityUrl: () => 'https://site.test/shop/activity',
    supportUrl: () => 'https://site.test/help',
    ...over
  }
  const gift = createShellGift({
    login: () => (opts.login === undefined ? LOGIN : opts.login),
    wallet,
    ui: {
      show: (v, h) => {
        views.push(v)
        handlers = h
        // The player presses SEND GIFT on the confirm step, unless a test leaves it waiting.
        if (v.step === 'confirm' && opts.confirm !== false) queueMicrotask(h.onConfirm)
      },
      hide: () => views.push({ step: 'hidden' } as unknown as GiftTransferView)
    },
    post: (e) => posts.push(e),
    openExternal: (url) => opened.push(url),
    otherDialogOpen: () => false,
    services
  })
  const settled = async (): Promise<void> => {
    for (let i = 0; i < 20; i++) await Promise.resolve()
    await new Promise((r) => setTimeout(r, 0))
  }
  return { gift, views, posts, opened, signed, settled, handlers: () => handlers!, setOwner: (o: string) => (owner = o) }
}

describe('who may gift', () => {
  it('a signed-in web3 or managed account, with the flag on', async () => {
    expect(await setup().gift.request('gift.capabilities', undefined)).toEqual({ v: 1, available: true })
    expect(await setup({}, { kind: 'managed' }).gift.request('gift.capabilities', undefined)).toMatchObject({ available: true })
  })

  it('not a guest, nobody, a session with no wallet here, or with the flag off', async () => {
    expect(await setup({}, { login: { ...LOGIN, guest: true } }).gift.request('gift.capabilities', undefined)).toMatchObject({ available: false, reason: 'guest' })
    expect(await setup({}, { login: null }).gift.request('gift.capabilities', undefined)).toMatchObject({ available: false, reason: 'signedOut' })
    expect(await setup({}, { kind: null }).gift.request('gift.capabilities', undefined)).toMatchObject({ available: false, reason: 'noWallet' })
    expect(await setup({ enabled: async () => false }).gift.request('gift.capabilities', undefined)).toMatchObject({ available: false, reason: 'disabled' })
    const err = await setup({}, { login: { ...LOGIN, guest: true } }).gift.request('gift.open', OPEN).catch((e: unknown) => e)
    expect(isShellError(err, GIFT_UNAVAILABLE)).toBe(true)
  })
})

describe('a gift', () => {
  it('is checked, signed gaslessly, shown sent with the recipient the shell resolved, and kept as pending', async () => {
    const t = setup()
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.signed()).toBe(true)
    const preparing = t.views.find((v) => v.step === 'preparing')
    expect(preparing).toMatchObject({ item: { name: 'Cool Hat', rarity: 'epic', image: 'https://img.test/hat.png' }, recipient: { address: RECIPIENT, name: 'Bob', nameColor: '#ff0000' }, status: 'A browser window should open for you to confirm the transaction.' })
    expect((preparing as { recipient: { face?: string } }).recipient.face).toBeUndefined()
    expect(t.views.at(-1)).toMatchObject({ step: 'sent', recipient: { address: RECIPIENT } })
    expect(t.posts).toEqual([{ type: GIFT_EVENT, v: 1, state: 'sent', urn: URN, transferredAt: 1700000000 }])
    expect(await t.gift.request('gift.pending', undefined)).toEqual({ v: 1, pending: [{ urn: URN, transferredAt: 1700000000 }] })
  })

  it("is refused before signing when the sender doesn't own the token, it is another chain's, the index doesn't know it, or it goes to the sender", async () => {
    for (const [over, open] of [
      [{ ownerOf: async () => RECIPIENT }, OPEN],
      [{ token: async () => null }, OPEN],
      [{}, { ...OPEN, urn: URN.replace(':matic:', ':amoy:') }],
      [{}, { ...OPEN, recipient: SENDER }]
    ] as const) {
      const t = setup(over as Partial<ShellGiftServices>)
      await t.gift.request('gift.open', open)
      await t.settled()
      expect(t.signed()).toBe(false)
      expect(t.views.at(-1)).toEqual({ step: 'failed' })
    }
  })

  it('a relay refusal fails it while the nonce is unused, and TRY AGAIN signs that nonce again', async () => {
    const nonces: Array<bigint | undefined> = []
    let first = true
    const t = setup({
      sendGasless: async (p, c, d, nonce) => {
        nonces.push(nonce)
        return signThen(async () => {
          if (first) {
            first = false
            throw new MetaTransactionError('refused', 'X', 400, true)
          }
          return HASH
        })(p, c, d, nonce)
      }
    })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.at(-1)).toEqual({ step: 'failed' })
    expect(t.posts).toEqual([])
    t.handlers().onRetry()
    await t.settled()
    expect(nonces).toEqual([7n, 7n])
    expect(t.views.at(-1)).toMatchObject({ step: 'sent' })
  })

  it("a lost relay answer is settled by the token's owner: gone from the account means sent", async () => {
    // eslint-disable-next-line prefer-const -- the send reaches it after setup returns
    let run: ReturnType<typeof setup>
    run = setup({
      sendGasless: signThen(async () => {
        run.setOwner(RECIPIENT)
        throw new Error('network')
      })
    })
    await run.gift.request('gift.open', OPEN)
    await run.settled()
    expect(run.views.at(-1)).toMatchObject({ step: 'sent' })
    expect(run.posts).toHaveLength(1)
  })

  it('a lost answer for a token still the sender\'s is watched, then failed, with TRY AGAIN signing the same nonce', async () => {
    const nonces: Array<bigint | undefined> = []
    const t = setup({
      sendGasless: async (p, c, d, nonce) => {
        nonces.push(nonce)
        return signThen(async () => {
          throw new Error('network')
        })(p, c, d, nonce)
      }
    })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.at(-1)).toEqual({ step: 'failed' })
    t.handlers().onRetry()
    await t.settled()
    expect(nonces).toEqual([7n, 7n])
  })

  it('a declined signature fails it, and the links open the activity and support pages', async () => {
    const t = setup({ sendGasless: async () => Promise.reject(Object.assign(new Error('User rejected the request.'), { code: 4001 })) })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.at(-1)).toEqual({ step: 'failed' })
    t.handlers().onSupport()
    t.handlers().onActivity()
    expect(t.opened).toEqual(['https://site.test/help', 'https://site.test/shop/activity'])
  })
})

describe('the confirm step', () => {
  it('shows the item and recipient the shell resolved, and signs nothing until SEND GIFT', async () => {
    const t = setup({}, { confirm: false })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.at(-1)).toMatchObject({ step: 'confirm', recipient: { address: RECIPIENT, name: 'Bob' }, item: { name: 'Cool Hat' } })
    expect(t.signed()).toBe(false)
    t.handlers().onConfirm()
    await t.settled()
    expect(t.signed()).toBe(true)
    expect(t.views.at(-1)).toMatchObject({ step: 'sent' })
  })

  it('marks a claimed recipient, and draws only an opaque name colour', async () => {
    const t = setup({ recipient: async (address) => ({ address, name: 'Bob', claimed: true, nameColor: '#ff000000' }) }, { confirm: false })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.at(-1)).toMatchObject({ step: 'confirm', recipient: { address: RECIPIENT, name: 'Bob', claimed: true } })
    expect((t.views.at(-1) as { recipient: { nameColor?: string } }).recipient.nameColor).toBeUndefined()
  })

  it('CANCEL on the confirm step sends nothing and closes', async () => {
    const t = setup({}, { confirm: false })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    t.handlers().onClose()
    await t.settled()
    expect(t.signed()).toBe(false)
    expect(t.posts).toEqual([])
    expect(t.gift.dialogOpen()).toBe(false)
  })

  it('tells a managed wallet, which opens no window, that it is processing', async () => {
    const t = setup({}, { kind: 'managed' })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.views.find((v) => v.step === 'preparing')).toMatchObject({ status: 'Processing...' })
  })

  it("refuses a token the index answers for with another contract's or another token's row", async () => {
    const t = setup({ token: async () => ({ contractAddress: '0x' + 'ee'.repeat(20), tokenId: '105', owner: SENDER, name: 'Other', image: '', category: 'wearable', chainId: 137 }) })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(t.signed()).toBe(false)
    expect(t.views.at(-1)).toEqual({ step: 'failed' })
  })

  it('drops a signature that comes back after the gift was closed, so nothing is relayed', async () => {
    const relayed = vi.fn(async () => HASH)
    let t: ReturnType<typeof setup>
    const slowWallet = (p: Eip1193Provider): Eip1193Provider => ({
      request: async (a) => {
        if (a.method === 'eth_signTypedData_v4') t.handlers().onClose()
        return p.request(a)
      }
    })
    t = setup({
      guard: (p, intent) => guardedGiftProvider(slowWallet(p), intent),
      sendGasless: signThen(relayed)
    })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(relayed).not.toHaveBeenCalled()
    expect(t.posts).toEqual([])
  })
})

describe('what never waits, and what cancels', () => {
  it('fails at once, with no watch, when nothing was signed: a wallet on another account, or a failed read', async () => {
    for (const over of [{ nonce: async () => Promise.reject(new Error('rpc down')) }, {}] as Array<Partial<ShellGiftServices>>) {
      const looks = vi.fn(async () => SENDER)
      const t = setup({ ...over, ownerOf: looks, ...(Object.keys(over).length === 0 ? { guard: (p) => ({ request: async (a) => (a.method === 'eth_signTypedData_v4' ? Promise.reject(new Error('wrong account')) : p.request(a)) }) } : {}) })
      await t.gift.request('gift.open', OPEN)
      await t.settled()
      expect(t.signed()).toBe(false)
      expect(t.views.at(-1)).toEqual({ step: 'failed' })
      // the check before signing is the only owner read
      expect(looks).toHaveBeenCalledTimes(1)
      expect(t.posts).toEqual([])
    }
  })

  it('CLOSE before the signature cancels the gift; after it, the gift goes on and is still reported', async () => {
    let close: (() => void) | null = null
    const before = setup({
      nonce: async () => {
        close?.()
        return 7n
      }
    })
    close = () => before.handlers().onClose()
    await before.gift.request('gift.open', OPEN)
    await before.settled()
    expect(before.signed()).toBe(false)
    expect(before.posts).toEqual([])
    expect(before.gift.dialogOpen()).toBe(false)

    // eslint-disable-next-line prefer-const -- the send reaches it after setup returns
    let after: ReturnType<typeof setup>
    after = setup({
      sendGasless: signThen(async () => {
        after.handlers().onClose()
        return HASH
      })
    })
    await after.gift.request('gift.open', OPEN)
    await after.settled()
    expect(after.posts).toHaveLength(1)
    expect(after.views.some((v) => v.step === 'sent')).toBe(false)
  })

  it('refuses a second gift while the dialog is up, and fails when another tab holds the account', async () => {
    const t = setup({ sendGasless: () => new Promise(() => undefined) })
    await t.gift.request('gift.open', OPEN)
    const err = await t.gift.request('gift.open', OPEN).catch((e: unknown) => e)
    expect(isShellError(err, 'gift.busy')).toBe(true)
    const held = setup({ claim: async () => null })
    await held.gift.request('gift.open', OPEN)
    await held.settled()
    expect(held.signed()).toBe(false)
    expect(held.views.at(-1)).toEqual({ step: 'failed' })
  })

  it('forgets a pending gift after a week', async () => {
    let now = 1_000_000
    const t = setup({ now: () => now })
    await t.gift.request('gift.open', OPEN)
    await t.settled()
    expect(await t.gift.request('gift.pending', undefined)).toMatchObject({ pending: [{ urn: URN }] })
    now += 8 * 24 * 60 * 60_000
    expect(await t.gift.request('gift.pending', undefined)).toEqual({ v: 1, pending: [] })
  })
})

describe('tokenParts', () => {
  it('reads a collection-v2 token urn only', () => {
    expect(tokenParts(URN, '105')).toEqual({ network: 'matic', contract: CONTRACT })
    expect(tokenParts(URN, '106')).toBeNull()
    expect(tokenParts('urn:decentraland:ethereum:collections-v1:dcl_launch:hat:7', '7')).toBeNull()
  })
})

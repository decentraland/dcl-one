// The web shell's side of Donate (shell/donate/donation): who may tip, what the panel shows, and what a send's
// outcome means. Reads, the relay and the chain are fakes here; the guard is the real one, so a send that reaches
// the fake wallet is one the guard let through.
import { describe, expect, it, vi } from 'vitest'
import { parseUnits } from 'viem'
import type { Eip1193Provider } from '../../data/auth/wallet'
import { MANA_TOKEN } from '@shop/market/lib/contracts'
import { metaTransactionTypedData, MetaTransactionError } from '../../shop/market/lib/metaTx'
import { signTypedData, TxPendingError, TxRevertedError } from '../../shop/market/lib/tx'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { isShellError } from '@explorer-core/lib/shellReply'
import { DONATE_BUSY, DONATE_EVENT, DONATE_UNAVAILABLE } from '../lib/donateProtocol'
import { createShellDonate, parsePresets, placeTitle, resolvePlace, weiToMana, type DonateHandlers, type DonateView, type ShellDonateServices } from '../shell/donate/donation'
import type { ShellShopWallet } from '../shell/shop/purchase'

const TIPPER = '0x' + 'aa'.repeat(20)
const CREATOR = '0x' + 'bb'.repeat(20)
const TOKEN = MANA_TOKEN[137]!
const HASH = '0x' + 'ab'.repeat(32)
const EXPIRES = new Date(Date.now() + 3_600_000).toISOString()
const LOGIN: StoredLogin = {
  address: TIPPER,
  identity: {
    ephemeralIdentity: { address: '0x' + '1'.repeat(40), publicKey: '0x04', privateKey: '0x' + 'ab'.repeat(32) },
    expiration: EXPIRES,
    authChain: [
      { type: 'SIGNER', payload: TIPPER, signature: '' },
      { type: 'ECDSA_EPHEMERAL', payload: 'Decentraland Login', signature: '0x' }
    ]
  }
}

/** A clock that steps 20 minutes a read, so a 30-minute watch takes exactly one look. */
function clock(): () => number {
  let t = 0
  return () => (t += 20 * 60_000)
}

function fakeProvider() {
  const calls: string[] = []
  const provider: Eip1193Provider = {
    request: vi.fn(async ({ method }: { method: string }) => {
      calls.push(method)
      if (method === 'eth_accounts' || method === 'eth_requestAccounts') return [TIPPER]
      if (method === 'eth_chainId') return '0x89'
      if (method === 'eth_signTypedData_v4') return '0x' + '11'.repeat(65)
      if (method === 'eth_sendTransaction') return HASH
      return null
    }) as Eip1193Provider['request']
  }
  return { provider, calls }
}

function wallet(provider: Eip1193Provider, kind: 'web3' | 'managed' = 'web3', web3 = true): ShellShopWallet {
  return { kind: () => kind, provider: () => provider, marketplace: () => web3 }
}

/** The gasless send as the Shop's sender makes it: the token meta-transaction signed through the wallet given. */
const signThen = (after: () => Promise<string>) => async (p: Eip1193Provider, token: typeof TOKEN, data: `0x${string}`) => {
  await signTypedData(p, TIPPER, metaTransactionTypedData(token, 'collection', TIPPER, 1n, data))
  return after()
}

function setup(over: Partial<ShellDonateServices> = {}, opts: { kind?: 'web3' | 'managed'; web3?: boolean; other?: boolean } = {}) {
  const w = fakeProvider()
  const views: DonateView[] = []
  let handlers: DonateHandlers = { onClose: () => {} }
  let hidden = 0
  const posted: unknown[] = []
  const services: Partial<ShellDonateServices> = {
    enabled: async () => true,
    presets: async () => [166, 333, 500],
    place: async () => ({ creator: CREATOR, title: 'Genesis Plaza' }),
    creator: async (address) => ({ address, name: 'Aurora' }),
    balance: async () => parseUnits('1000', 18),
    usdRate: async () => 0.5,
    tipTarget: async () => ({ chainId: 137, manaToken: TOKEN.address }),
    chainId: () => 137,
    sendGasless: signThen(async () => HASH),
    sendDirect: async () => HASH,
    wait: async (_c, h) => h,
    forward: vi.fn(async () => {}),
    sleep: async () => {},
    now: clock(),
    accountUrl: () => 'https://example.org/account',
    supportUrl: () => 'https://example.org/help',
    ...over
  }
  const donate = createShellDonate({
    login: () => LOGIN,
    wallet: wallet(w.provider, opts.kind, opts.web3 ?? true),
    ui: {
      show: (v, h) => {
        views.push(v)
        handlers = h
      },
      hide: () => hidden++
    },
    post: (e) => posted.push(e),
    openExternal: vi.fn(),
    otherDialogOpen: () => opts.other === true,
    services
  })
  return { donate, views, last: () => views[views.length - 1], handlers: () => handlers, hidden: () => hidden, posted, w, services }
}

const open = (d: ReturnType<typeof setup>['donate']) => d.request('donate.open', { v: 1, place: { parcel: { x: 0, y: 0 } } })
const flush = () => new Promise((r) => setTimeout(r, 0))

describe('who may tip', () => {
  it('offers Donate to a web3 sign-in only, with the feature on', async () => {
    expect(await setup().donate.request('donate.capabilities', undefined)).toEqual({ v: 1, available: true })
    expect(await setup({ enabled: async () => false }).donate.request('donate.capabilities', undefined)).toMatchObject({ available: false, reason: 'disabled' })
    expect(await setup({}, { kind: 'managed', web3: false }).donate.request('donate.capabilities', undefined)).toMatchObject({ available: false, reason: 'notWeb3' })
  })

  it('refuses to open for a managed wallet, whatever the app asks', async () => {
    const s = setup({}, { kind: 'managed', web3: false })
    await expect(open(s.donate)).rejects.toSatisfy((e) => isShellError(e, DONATE_UNAVAILABLE))
    expect(s.views).toHaveLength(0)
  })

  it('waits its turn while another dialog is up', async () => {
    const s = setup({}, { other: true })
    await expect(open(s.donate)).rejects.toSatisfy((e) => isShellError(e, DONATE_BUSY))
  })
})

describe('the panel', () => {
  it('shows the place, the creator the shell resolved, the balance and the suggested amounts', async () => {
    const s = setup()
    await open(s.donate)
    expect(s.views[0]).toEqual({ step: 'loading' })
    expect(s.last()).toEqual({ step: 'form', scene: 'Genesis Plaza', creator: { address: CREATOR, name: 'Aurora' }, balance: 1000, presets: [166, 333, 500], usdRate: 0.5 })
  })

  it('closes when the place has no creator, or the payments config names another network', async () => {
    const none = setup({ place: async () => null })
    await expect(open(none.donate)).rejects.toThrow(/no creator/)
    expect(none.hidden()).toBe(1)
    const elsewhere = setup({ tipTarget: async () => ({ chainId: 80002, manaToken: TOKEN.address }) })
    await expect(open(elsewhere.donate)).rejects.toThrow(/another network/)
    expect(elsewhere.donate.dialogOpen()).toBe(false)
  })

  it('takes the creator only from the row that is the place asked for', () => {
    const plaza = { creator_address: CREATOR, title: 'Plaza', world: false, positions: ['0,0', '0,1'] }
    const world = { creator_address: '0x' + 'dd'.repeat(20), title: 'W', world: true, world_name: 'a.dcl.eth', positions: ['0,0'] }
    expect(resolvePlace({ parcel: { x: 0, y: 1 } }, [world, plaza])).toEqual({ creator: CREATOR, title: 'Plaza', positions: ['0,0', '0,1'] })
    expect(resolvePlace({ parcel: { x: 5, y: 5 } }, [plaza])).toBeNull()
    expect(resolvePlace({ world: 'a.dcl.eth' }, [plaza, world])?.creator).toBe('0x' + 'dd'.repeat(20))
    expect(resolvePlace({ world: 'a.dcl.eth' }, [{ ...world, world_name: 'aa.dcl.eth' }])).toBeNull()
    expect(resolvePlace({ parcel: { x: 0, y: 0 } }, [{ ...plaza, creator_address: 'nobody' }])).toBeNull()
  })

  it('reads the suggested amounts and the place name', () => {
    expect(parsePresets('10, 20,30,40')).toEqual([10, 20, 30])
    expect(parsePresets('')).toEqual([166, 333, 500])
    expect(placeTitle({ world: 'lorux0r.dcl.eth' }, 'x')).toBe('lorux0r')
    expect(placeTitle({ parcel: { x: 0, y: 0 } }, null)).toBe('Unknown place')
    expect(weiToMana(parseUnits('12.3456', 18))).toBe(12.3456)
  })
})

describe('sending a tip', () => {
  it('sends gasless, shows it sent, tells the app and forwards the mined hash', async () => {
    const s = setup()
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(s.views.map((v) => v.step)).toEqual(['loading', 'form', 'sending', 'sent'])
    // The emote plays once the player closes the sent step.
    expect(s.posted).toEqual([])
    s.handlers().onClose()
    expect(s.posted).toEqual([{ type: DONATE_EVENT, v: 1, state: 'sent' }])
    expect(s.donate.dialogOpen()).toBe(false)
    expect(s.services.forward).toHaveBeenCalledWith(HASH, LOGIN)
    expect(s.w.calls).toContain('eth_signTypedData_v4')
  })

  it('refuses, and says why, an amount below 1 MANA, one it can’t read, or above the balance read before signing', async () => {
    const s = setup({ balance: vi.fn().mockResolvedValueOnce(parseUnits('1000', 18)).mockResolvedValue(parseUnits('100', 18)) })
    await open(s.donate)
    for (const [amount, error] of [['0.5', /1 MANA/], ['10.', /1 MANA/], ['166', /Insufficient MANA/]] as const) {
      s.handlers().onSend?.(amount)
      await flush()
      expect(s.last()).toMatchObject({ step: 'form', error: expect.stringMatching(error) })
    }
    expect(s.w.calls).not.toContain('eth_signTypedData_v4')
  })

  it('sends one tip per SEND, however often it is pressed', async () => {
    let release: () => void = () => {}
    const gate = new Promise<void>((r) => (release = r))
    const sendGasless = vi.fn(signThen(async () => (await gate, HASH)))
    const s = setup({ sendGasless })
    await open(s.donate)
    const send = s.handlers().onSend!
    send('166')
    send('166')
    await flush()
    release()
    await flush()
    expect(sendGasless).toHaveBeenCalledOnce()
  })

  it('fails, with TRY AGAIN, when the wallet declines', async () => {
    const s = setup({ sendGasless: async () => Promise.reject(Object.assign(new Error('User rejected the request.'), { code: 4001 })) })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(s.last().step).toBe('failed')
    s.handlers().onRetry?.()
    expect(s.last().step).toBe('form')
  })

  it('sends from the wallet only when the relay clearly refused before broadcasting', async () => {
    const sendDirect = vi.fn(async () => HASH)
    const s = setup({ sendGasless: signThen(() => Promise.reject(new MetaTransactionError('refused', 'INVALID', 400, true))), sendDirect })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(sendDirect).toHaveBeenCalledOnce()
    expect(s.last().step).toBe('sent')
  })

  it('never sends again after an unclear relay answer: it watches the balance and confirms by its effect', async () => {
    const sendDirect = vi.fn(async () => HASH)
    // Read when the panel opens and again before signing; then the tip's effect.
    const balance = vi.fn().mockResolvedValueOnce(parseUnits('1000', 18)).mockResolvedValueOnce(parseUnits('1000', 18)).mockResolvedValue(parseUnits('834', 18))
    const s = setup({ sendGasless: signThen(() => Promise.reject(new MetaTransactionError('The relay answered 502', '502', 502, false))), sendDirect, balance })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(sendDirect).not.toHaveBeenCalled()
    expect(s.views.map((v) => v.step)).toContain('processing')
    expect(s.last().step).toBe('sent')
  })

  it('keeps an unclear send that shows no effect as still processing, with no retry', async () => {
    const sendDirect = vi.fn(async () => HASH)
    const s = setup({ sendGasless: signThen(() => Promise.reject(new MetaTransactionError('lost', 'network', 502, false))), sendDirect })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(sendDirect).not.toHaveBeenCalled()
    expect(s.views.some((v) => v.step === 'failed')).toBe(false)
    expect(s.last()).toMatchObject({ step: 'processing', final: true })
  })

  it('fails at once when a processing tip reverts', async () => {
    const s = setup({ wait: async (_c, h, timeout) => Promise.reject(timeout > 200_000 ? new TxRevertedError(h) : new TxPendingError(h, 'The tip')) })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(s.views.map((v) => v.step)).toEqual(['loading', 'form', 'sending', 'processing', 'failed'])
  })

  it('shows an unsettled tip again instead of a new form, until it settles', async () => {
    const s = setup({ wait: async (_c, h, timeout) => (timeout > 200_000 ? new Promise(() => {}) : Promise.reject(new TxPendingError(h, 'The tip'))), sleep: () => new Promise(() => {}) })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(s.last().step).toBe('processing')
    s.handlers().onClose()
    expect(s.donate.dialogOpen()).toBe(false)
    await open(s.donate)
    expect(s.last()).toMatchObject({ step: 'processing', amount: '166' })
    expect(s.views.filter((v) => v.step === 'form')).toHaveLength(1)
  })

  it('claims the dialog before its first read, so a second open waits', async () => {
    let release: () => void = () => {}
    const s = setup({ enabled: () => new Promise<boolean>((r) => (release = () => r(true))) })
    const first = open(s.donate)
    expect(s.donate.dialogOpen()).toBe(true)
    await expect(open(s.donate)).rejects.toSatisfy((e) => isShellError(e, DONATE_BUSY))
    release()
    await first
  })

  it('fails without watching when the guard refused before the wallet signed', async () => {
    // A sender asking for another recipient: the guard refuses, nothing reaches the wallet.
    const s = setup({
      sendGasless: async (p, token) => {
        await signTypedData(p, TIPPER, metaTransactionTypedData(token, 'collection', TIPPER, 1n, '0xa9059cbb' + '0'.repeat(24) + 'cc'.repeat(20) + '0'.repeat(64) as `0x${string}`))
        return HASH
      }
    })
    await open(s.donate)
    s.handlers().onSend?.('166')
    await flush()
    expect(s.w.calls).not.toContain('eth_signTypedData_v4')
    expect(s.last().step).toBe('failed')
    expect(s.views.some((v) => v.step === 'processing')).toBe(false)
  })

  it('shows a tip not yet mined as processing, and a reverted one as failed', async () => {
    const pending = setup({ wait: async (_c, h, timeout) => (timeout > 200_000 ? new Promise(() => {}) : Promise.reject(new TxPendingError(h, 'The tip'))), sleep: () => new Promise(() => {}) })
    await open(pending.donate)
    pending.handlers().onSend?.('166')
    await flush()
    expect(pending.last().step).toBe('processing')
    const reverted = setup({ wait: async (_c, h) => Promise.reject(new TxRevertedError(h)) })
    await open(reverted.donate)
    reverted.handlers().onSend?.('166')
    await flush()
    expect(reverted.last().step).toBe('failed')
  })
})

// A MANA purchase's attempt (shell/shop/manaPurchase): which ways the sale may go, and when, and what its record keeps.
import { describe, expect, it, vi } from 'vitest'
import type { Hex } from 'viem'
import { createManaAttempt, isManaAttempt, manaDispatched, manaFingerprint, type ManaAttemptRecord } from '../shell/shop/manaPurchase'
import type { ManaSaleTerms } from '../shell/shop/manaGuard'

const STORE = '0x214ffc0f0103735728dc66b61a22e4f163e275ae'
const MANA = '0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4'
const BUYER = '0x' + '0b'.repeat(20)
const TERMS: ManaSaleTerms = { kind: 'mints', venue: STORE, units: [{ contract: '0x' + 'c0'.repeat(20), itemId: '7' }], maxWei: 2_300_000_000_000_000_000n }

/** What `keep` stored, in order; `refuse` makes chosen ones fail. */
function keeper(refuse: (record: ManaAttemptRecord) => boolean = () => false) {
  const kept: ManaAttemptRecord[] = []
  return {
    kept,
    keep: (record: ManaAttemptRecord) => {
      if (refuse(record)) return false
      kept.push(record)
      return true
    }
  }
}

const post = (target: string) => ({ method: 'POST', body: JSON.stringify({ transactionData: { from: BUYER, params: [target, '0x'] } }) })
const answer = (body: unknown, status = 200) => async () => new Response(JSON.stringify(body), { status })

describe('a MANA purchase’s attempt', () => {
  it('keeps the relay’s signature, then its dispatch right before the relay request carries the sale', async () => {
    const k = keeper()
    const a = createManaAttempt(TERMS, k.keep, { mana: MANA })
    expect(a.handOff({ rail: 'relay', nonce: 41n })).toBe(true)
    expect(k.kept.at(-1)?.rails).toEqual([{ rail: 'relay', nonce: '41', stage: 'signing' }])
    expect(a.dispatched()).toBe(false)
    const send = vi.fn(answer({ ok: true, txHash: '0x' + 'f'.repeat(64) }))
    await a.relay(send as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(send).toHaveBeenCalledTimes(1)
    expect(k.kept.at(-1)?.rails).toEqual([{ rail: 'relay', nonce: '41', stage: 'dispatched' }])
    expect(a.dispatched()).toBe(true)
  })

  it('takes one signature for the relay, and no second way after a dispatch', () => {
    const a = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    expect(a.handOff({ rail: 'relay', nonce: 1n })).toBe(true)
    expect(a.handOff({ rail: 'relay', nonce: 2n })).toBe(false)
    a.record.rails[0].stage = 'dispatched'
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('lets the wallet send it once after the relay proved it sent nothing, and the relay is marked refused first', async () => {
    const k = keeper()
    const a = createManaAttempt(TERMS, k.keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    await a.relay(answer({ ok: false, not_sent: true, error: 'invalid_transaction' }, 400) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(k.kept.at(-1)?.rails).toEqual([{ rail: 'relay', nonce: '1', stage: 'refused' }])
    expect(a.dispatched()).toBe(false)
    expect(a.handOff({ rail: 'direct' })).toBe(true)
    expect(k.kept.at(-1)?.rails.map((r) => `${r.rail}:${r.stage}`)).toEqual(['relay:refused', 'direct:dispatched'])
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it.each([
    ['a refusal without its evidence', answer({ ok: false, error: 'invalid_transaction' }, 400)],
    ['an error after the relay took it', answer({ ok: false, error: 'unknown' }, 504)]
  ])('keeps the wallet closed after %s', async (_what, reply) => {
    const a = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    await a.relay(reply as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(a.dispatched()).toBe(true)
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('lets the wallet send it when the relay never got a signature (the wallet couldn’t sign typed data)', () => {
    const a = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    expect(a.handOff({ rail: 'direct' })).toBe(true)
  })

  it('answers the sale’s relay request as never sent, and sends nothing more, when its dispatch can’t be kept', async () => {
    const a = createManaAttempt(TERMS, keeper((r) => r.rails.some((x) => x.stage === 'dispatched')).keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    const send = vi.fn(answer({ ok: true, txHash: '0x' + 'f'.repeat(64) }))
    const res = await a.relay(send as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(send).not.toHaveBeenCalled()
    expect(await res.json()).toMatchObject({ ok: false, not_sent: true })
    expect(a.record.rails).toEqual([{ rail: 'relay', nonce: '1', stage: 'signing' }])
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('refuses the hand-off when its checkpoint can’t be kept, and every one after', () => {
    let fail = true
    const a = createManaAttempt(TERMS, keeper(() => fail).keep, { mana: MANA })
    expect(a.handOff({ rail: 'relay', nonce: 1n })).toBe(false)
    fail = false
    expect(a.handOff({ rail: 'direct' })).toBe(false)
    expect(a.record.rails).toEqual([])
  })

  it('keeps the wallet closed when the relay’s refusal can’t be kept', async () => {
    const a = createManaAttempt(TERMS, keeper((r) => r.rails.some((x) => x.stage === 'refused')).keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    await a.relay(answer({ ok: false, not_sent: true }, 400) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('lets MANA’s approve through the relay only while the buyer authorizes it, and never once closed', async () => {
    const k = keeper()
    const a = createManaAttempt(TERMS, k.keep, { mana: MANA })
    const send = vi.fn(answer({ ok: true, txHash: '0x' + 'f'.repeat(64) }))
    const relay = a.relay(send as unknown as typeof fetch)
    // On the rails' own initiative (no AUTHORIZE running): answered as never sent.
    expect(a.approving()).toBe(false)
    expect(await (await relay('https://relay/transactions', post(MANA))).json()).toMatchObject({ ok: false, not_sent: true })
    expect(send).not.toHaveBeenCalled()
    await a.authorize(async () => {
      expect(a.approving()).toBe(true)
      await relay('https://relay/transactions', post(MANA))
    })
    expect(send).toHaveBeenCalledTimes(1)
    expect(a.approving()).toBe(false)
    expect(k.kept).toHaveLength(0)
    a.close()
    await a.authorize(async () => {
      expect(a.approving()).toBe(false)
      expect(await (await relay('https://relay/transactions', post(MANA))).json()).toMatchObject({ ok: false, not_sent: true })
    })
    expect(send).toHaveBeenCalledTimes(1)
    expect(a.handOff({ rail: 'relay', nonce: 1n })).toBe(false)
  })

  it('answers any other relay request as never sent: another contract, an unreadable body, a Request', async () => {
    const a = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    const send = vi.fn(answer({ ok: true, txHash: '0x' + 'f'.repeat(64) }))
    const relay = a.relay(send as unknown as typeof fetch)
    for (const res of [
      await relay('https://relay/transactions', post('0x' + '99'.repeat(20))),
      await relay('https://relay/transactions', { method: 'POST', body: 'not json' }),
      await relay(new Request('https://relay/transactions', { method: 'POST', body: JSON.stringify({ transactionData: { from: BUYER, params: [STORE, '0x'] } }) }))
    ]) {
      expect(await res.json()).toMatchObject({ ok: false, not_sent: true })
    }
    expect(send).not.toHaveBeenCalled()
    expect(a.dispatched()).toBe(false)
  })

  it('ends the attempt when the buyer declines the sale’s relay signature: nothing more goes', async () => {
    const reject = Object.assign(new Error('User rejected the request.'), { code: 4001 })
    const a = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    a.handOff({ rail: 'relay', nonce: 1n })
    const wallet = { request: vi.fn(async () => Promise.reject(reject)) }
    const typed = JSON.stringify({ domain: { verifyingContract: STORE }, message: {} })
    await expect(a.observe(wallet).request({ method: 'eth_signTypedData_v4', params: [BUYER, typed] })).rejects.toBe(reject)
    expect(a.record.rails).toEqual([{ rail: 'relay', nonce: '1', stage: 'declined' }])
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('ends the wallet’s send as declined when the buyer declines it there, and not for any other error', async () => {
    const reject = Object.assign(new Error('User rejected the request.'), { code: 4001 })
    const declined = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    declined.handOff({ rail: 'direct' })
    const wallet = { request: vi.fn(async () => Promise.reject(reject)) }
    await expect(declined.observe(wallet).request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: STORE, data: '0x' }] })).rejects.toBe(reject)
    expect(declined.record.rails).toEqual([{ rail: 'direct', stage: 'declined' }])
    expect(declined.dispatched()).toBe(false)

    const lost = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    lost.handOff({ rail: 'direct' })
    const broken = { request: vi.fn(async () => Promise.reject(new Error('internal error'))) }
    await expect(lost.observe(broken).request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: STORE, data: '0x' }] })).rejects.toThrow('internal error')
    expect(lost.dispatched()).toBe(true)

    // An approve the buyer declines leaves the sale's rails alone.
    const approve = createManaAttempt(TERMS, keeper().keep, { mana: MANA })
    approve.handOff({ rail: 'direct' })
    await expect(approve.observe(wallet).request({ method: 'eth_sendTransaction', params: [{ from: BUYER, to: MANA, data: '0x' }] })).rejects.toBe(reject)
    expect(approve.record.rails).toEqual([{ rail: 'direct', stage: 'dispatched' }])
  })

  it('reads a stored attempt only in the shape it writes', () => {
    const good: ManaAttemptRecord = { venue: STORE, fingerprint: manaFingerprint(TERMS), rails: [{ rail: 'relay', nonce: '3', stage: 'refused' }, { rail: 'direct', stage: 'dispatched' }] }
    expect(isManaAttempt(good)).toBe(true)
    expect(manaDispatched(good)).toBe(true)
    expect(isManaAttempt({ ...good, venue: 'nope' })).toBe(false)
    expect(isManaAttempt({ ...good, fingerprint: '0x12' })).toBe(false)
    expect(isManaAttempt({ ...good, rails: [{ rail: 'direct', stage: 'refused' }] })).toBe(false)
    expect(isManaAttempt({ ...good, rails: [{ rail: 'relay', stage: 'later' }] })).toBe(false)
    expect(isManaAttempt({ ...good, rails: [...good.rails, { rail: 'direct', stage: 'dispatched' }] })).toBe(false)
  })

  it('tells two sales apart by their exact bytes or units and ceiling', () => {
    const listings = (calldata: Hex): ManaSaleTerms => ({ kind: 'listings', venue: STORE, calldata, units: TERMS.units })
    expect(manaFingerprint(TERMS)).toBe(manaFingerprint(structuredClone(TERMS)))
    expect(manaFingerprint(TERMS)).not.toBe(manaFingerprint({ ...TERMS, maxWei: TERMS.kind === 'mints' ? TERMS.maxWei + 1n : 0n }))
    expect(manaFingerprint(listings('0x1234'))).not.toBe(manaFingerprint(listings('0x1235')))
  })
})

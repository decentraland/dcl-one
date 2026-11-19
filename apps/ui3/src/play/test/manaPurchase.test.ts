// A MANA purchase's attempt (shell/shop/manaPurchase): when each send of the sale may go, and what its record keeps.
import { describe, expect, it, vi } from 'vitest'
import { keccak256, type Hex } from 'viem'
import { compactManaAttempt, createManaAttempt, isManaAttempt, manaDispatched, manaFingerprint, manaLastDispatch, manaHasRoom, MAX_SENDS, type ManaAttemptRecord } from '../shell/shop/manaPurchase'
import type { ManaSaleTerms } from '../shell/shop/manaGuard'

const STORE = '0x214ffc0f0103735728dc66b61a22e4f163e275ae'
const MANA = '0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4'
const BUYER = '0x' + '0b'.repeat(20)
const TERMS: ManaSaleTerms = { kind: 'mints', venue: STORE, units: [{ contract: '0x' + 'c0'.repeat(20), itemId: '7' }], maxWei: 2_300_000_000_000_000_000n }
const CALL = '0xa4fdc78a00' as Hex
const PAYLOAD = '0x0c53c51c' + 'ab'.repeat(40)

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

const attempt = (k = keeper(), opts: Partial<Parameters<typeof createManaAttempt>[2]> = {}) =>
  createManaAttempt(TERMS, k.keep, { mana: MANA, checkpoint: async () => 1000n, now: () => 5_000, ...opts })
const post = (target: string, payload = PAYLOAD) => ({ method: 'POST', body: JSON.stringify({ transactionData: { from: BUYER, params: [target, payload] } }) })
const answer = (body: unknown, status = 200) => async () => new Response(JSON.stringify(body), { status })
const HASH = '0x' + 'f'.repeat(64)
const sent = (body: unknown = { ok: true, txHash: HASH }) => vi.fn(answer(body))

describe('a MANA purchase’s attempt', () => {
  it('keeps the nonce and the call before the signature, then the checkpoint, payload and time before the relay request', async () => {
    const k = keeper()
    const a = attempt(k)
    expect(a.handOff({ rail: 'relay', nonce: 41n, data: CALL })).toBe(true)
    expect(k.kept.at(-1)).toMatchObject({ nonce: '41', data: CALL, sends: [{ stage: 'signing' }] })
    expect(k.kept.at(-1)?.fromBlock).toBeUndefined()
    const send = sent()
    await a.relay(send as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(send).toHaveBeenCalledTimes(1)
    expect(k.kept.at(-2)).toMatchObject({ fromBlock: '1000', sends: [{ stage: 'dispatched', payload: keccak256(PAYLOAD as Hex), at: 5_000 }] })
    // The relay's hash for what went, kept after.
    expect(k.kept.at(-1)?.sends[0]).toMatchObject({ stage: 'dispatched', hash: HASH })
    expect(a.dispatched()).toBe(true)
    expect(manaLastDispatch(a.record)).toBe(5_000)
  })

  it('never sends the sale from the wallet', () => {
    const a = attempt()
    expect(a.handOff({ rail: 'direct' })).toBe(false)
    expect(a.handOff({ rail: 'relay', nonce: 1n, data: CALL })).toBe(true)
    expect(a.handOff({ rail: 'direct' })).toBe(false)
  })

  it('takes one signature, and a second only through RETRY, of the same call at the same nonce', async () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 7n, data: CALL })
    await a.relay(sent({ ok: false, error: 'unknown' }) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(a.handOff({ rail: 'relay', nonce: 7n, data: CALL })).toBe(false)
    a.retry()
    expect(a.handOff({ rail: 'relay', nonce: 8n, data: CALL })).toBe(false)
    expect(a.handOff({ rail: 'relay', nonce: 7n, data: '0xdead' })).toBe(false)
    expect(a.handOff({ rail: 'relay', nonce: 7n, data: CALL })).toBe(true)
    expect(a.record.sends.map((s) => s.stage)).toEqual(['dispatched', 'signing'])
    // One RETRY, one send; and none while one is being signed.
    a.retry()
    expect(a.handOff({ rail: 'relay', nonce: 7n, data: CALL })).toBe(false)
  })

  it('marks a send the relay refused, and keeps the checkpoint the first request took', async () => {
    const checkpoint = vi.fn(async () => 1000n)
    const a = attempt(keeper(), { checkpoint })
    a.handOff({ rail: 'relay', nonce: 1n, data: CALL })
    await a.relay(sent({ ok: false, not_sent: true, error: 'quota_reached' }) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(a.record.sends).toMatchObject([{ stage: 'refused' }])
    expect(a.dispatched()).toBe(false)
    a.retry()
    a.handOff({ rail: 'relay', nonce: 1n, data: CALL })
    await a.relay(sent() as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(checkpoint).toHaveBeenCalledTimes(1)
    expect(a.record.fromBlock).toBe('1000')
  })

  it('answers the sale’s relay request as never sent, and sends nothing more, when a checkpoint can’t be kept or read', async () => {
    for (const make of [
      () => attempt(keeper((r) => r.sends.some((x) => x.stage === 'dispatched'))),
      () =>
        attempt(keeper(), {
          checkpoint: async () => {
            throw new Error('rpc down')
          }
        })
    ]) {
      const a = make()
      a.handOff({ rail: 'relay', nonce: 1n, data: CALL })
      const send = sent()
      const res = await a.relay(send as unknown as typeof fetch)('https://relay/transactions', post(STORE))
      expect(send).not.toHaveBeenCalled()
      expect(await res.json()).toMatchObject({ ok: false, not_sent: true })
      expect(a.dispatched()).toBe(false)
      a.retry()
      expect(a.handOff({ rail: 'relay', nonce: 1n, data: CALL })).toBe(false)
    }
  })

  it('refuses the hand-off when its checkpoint can’t be kept, and every one after', () => {
    let fail = true
    const a = attempt(keeper(() => fail))
    expect(a.handOff({ rail: 'relay', nonce: 1n, data: CALL })).toBe(false)
    fail = false
    expect(a.handOff({ rail: 'relay', nonce: 1n, data: CALL })).toBe(false)
    expect(a.record.sends).toEqual([])
  })

  it('lets MANA’s approve through the relay only while the buyer authorizes it, and never once closed', async () => {
    const k = keeper()
    const a = attempt(k)
    const send = sent()
    const relay = a.relay(send as unknown as typeof fetch)
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
    expect(a.handOff({ rail: 'relay', nonce: 1n, data: CALL })).toBe(false)
  })

  it('answers any other relay request as never sent: another contract, an unreadable body, a Request', async () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 1n, data: CALL })
    const send = sent()
    const relay = a.relay(send as unknown as typeof fetch)
    for (const res of [
      await relay('https://relay/transactions', post('0x' + '99'.repeat(20))),
      await relay('https://relay/transactions', { method: 'POST', body: 'not json' }),
      await relay(new Request('https://relay/transactions', { method: 'POST', body: JSON.stringify({ transactionData: { from: BUYER, params: [STORE, PAYLOAD] } }) }))
    ]) {
      expect(await res.json()).toMatchObject({ ok: false, not_sent: true })
    }
    expect(send).not.toHaveBeenCalled()
    expect(a.dispatched()).toBe(false)
  })

  it('ends a send whose signature the buyer declines in the wallet, and only for the sale’s signature', async () => {
    const reject = Object.assign(new Error('User rejected the request.'), { code: 4001 })
    const wallet = { request: vi.fn(async () => Promise.reject(reject)) }
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 1n, data: CALL })
    const approve = JSON.stringify({ domain: { verifyingContract: MANA }, message: {} })
    await expect(a.observe(wallet).request({ method: 'eth_signTypedData_v4', params: [BUYER, approve] })).rejects.toBe(reject)
    expect(a.record.sends).toMatchObject([{ stage: 'signing' }])
    const sale = JSON.stringify({ domain: { verifyingContract: STORE }, message: {} })
    await expect(a.observe(wallet).request({ method: 'eth_signTypedData_v4', params: [BUYER, sale] })).rejects.toBe(reject)
    expect(a.record.sends).toMatchObject([{ stage: 'declined' }])
    expect(a.dispatched()).toBe(false)
  })

  it('keeps only what may be evidence: the sends that went, each as itself even with the same bytes', () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    const rec: ManaAttemptRecord = {
      ...a.record,
      fromBlock: '1',
      sends: [
        { id: 's7', stage: 'dispatched', payload: keccak256('0x01'), at: 1 },
        { id: 's8', stage: 'refused', payload: keccak256('0x02'), at: 2 },
        { id: 's9', stage: 'dispatched', payload: keccak256('0x01'), at: 3, hash: HASH },
        { id: 's10', stage: 'declined' },
        { id: 's11', stage: 'signing' }
      ]
    }
    expect(compactManaAttempt(rec).sends).toEqual([
      { id: 's7', stage: 'dispatched', payload: keccak256('0x01'), at: 1 },
      { id: 's9', stage: 'dispatched', payload: keccak256('0x01'), at: 3, hash: HASH }
    ])
  })

  it('retires a send left signing, and has no room once as many sends went as a record keeps', async () => {
    const k = keeper()
    const a = attempt(k)
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await a.relay(sent({ ok: false, error: 'unknown' }) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    a.retry()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    expect(manaHasRoom(a.record)).toBe(false)
    expect(a.retire()).toBe(true)
    expect(a.record.sends.map((s) => s.stage)).toEqual(['dispatched'])
    expect(manaHasRoom(a.record)).toBe(true)
    const full: ManaAttemptRecord = { ...a.record, sends: Array.from({ length: MAX_SENDS }, (_, i) => ({ id: `s${i}`, stage: 'dispatched' as const, payload: keccak256(`0x0${i}`), at: i })) }
    expect(manaHasRoom(full)).toBe(false)
    const capped = attempt(keeper(), { from: full })
    capped.retry()
    expect(capped.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(false)
  })

  it('takes a send unanswered past its deadline as a relay that didn’t answer, and keeps its late answer on that send alone', async () => {
    const a = attempt(keeper(), { sendMs: 20 })
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    let answerFirst!: (res: Response) => void
    const first = vi.fn(() => new Promise<Response>((r) => (answerFirst = r)))
    await expect(a.relay(first as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))).rejects.toThrow(TypeError)
    expect(a.dispatched()).toBe(true)
    // A retry (the same signed execution) goes and is answered; then the first one's late refusal comes back.
    a.retry()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await a.relay(sent() as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))
    answerFirst(new Response(JSON.stringify({ ok: false, not_sent: true }), { status: 400 }))
    await vi.waitFor(() => expect(a.record.sends[0]).toMatchObject({ stage: 'refused', payload: keccak256(PAYLOAD as Hex) }))
    expect(a.record.sends[1]).toMatchObject({ stage: 'dispatched', payload: keccak256(PAYLOAD as Hex), hash: HASH })
  })

  it('hands a retry off only once the wait since the latest request passed, a request the relay refused included', async () => {
    const clock = { now: 5_000 }
    const a = attempt(keeper(), { now: () => clock.now, retryAfterMs: 1_000 })
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await expect(a.relay((async () => Promise.reject(new TypeError('Failed to fetch'))) as unknown as typeof fetch)('https://relay/transactions', post(STORE))).rejects.toThrow()
    a.retry()
    expect(a.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(false)
    clock.now += 1_000
    expect(a.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(true)
    // The retry is refused; dropped from the record, its request still counts.
    await a.relay(sent({ ok: false, not_sent: true }) as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(a.retire()).toBe(true)
    expect(a.record.sends.map((x) => x.stage)).toEqual(['dispatched'])
    expect(manaLastDispatch(a.record)).toBe(6_000)
    a.retry()
    expect(a.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(false)
    clock.now += 1_000
    expect(a.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(true)
  })

  it('sends only the signed execution once the sale was signed: other bytes are answered as never sent', async () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await a.relay(sent() as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))
    expect(a.record.signed).toBe(PAYLOAD)
    a.retry()
    expect(a.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(true)
    const relay = sent()
    const res = await a.relay(relay as unknown as typeof fetch)('https://relay/transactions', post(STORE, '0x0c53c51c' + 'ef'.repeat(40)))
    expect(await res.json()).toMatchObject({ ok: false, not_sent: true })
    expect(relay).not.toHaveBeenCalled()
    expect(a.record.sends.map((x) => x.stage)).toEqual(['dispatched', 'signing'])
    expect(a.retire()).toBe(true)
    expect(a.record.sends.map((x) => x.stage)).toEqual(['dispatched'])
  })

  it('keeps a retry and its hash when the first send’s late refusal comes back, even with the same signed bytes, through a reload', async () => {
    // One record for the operation, as the shell keeps it: both attempts read and write it.
    let stored: ManaAttemptRecord | undefined
    const keep = (record: ManaAttemptRecord) => {
      stored = structuredClone(record)
      return true
    }
    const shared = { mana: MANA, checkpoint: async () => 1000n, now: () => 5_000, load: () => stored, sendMs: 20 }
    const a = createManaAttempt(TERMS, keep, shared)
    keep(a.record)
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    let answerA!: (res: Response) => void
    await expect(a.relay((() => new Promise<Response>((r) => (answerA = r))) as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))).rejects.toThrow(TypeError)
    // The retry, as retryMana makes it: a second attempt from what was kept. The wallet signs the same bytes.
    const b = createManaAttempt(TERMS, keep, { ...shared, from: stored })
    b.retry()
    expect(b.handOff({ rail: 'relay', nonce: 3n, data: CALL })).toBe(true)
    await b.relay(sent() as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))
    answerA(new Response(JSON.stringify({ ok: false, not_sent: true }), { status: 400 }))
    await vi.waitFor(() => expect(stored!.sends.map((x) => x.stage)).toEqual(['refused', 'dispatched']))
    expect(stored!.sends[1]).toMatchObject({ stage: 'dispatched', payload: keccak256(PAYLOAD as Hex), hash: HASH })
    // A reload reads it whole: the retry still went, with its hash.
    const reloaded = JSON.parse(JSON.stringify(stored)) as unknown
    expect(isManaAttempt(reloaded)).toBe(true)
    expect(manaDispatched(reloaded as ManaAttemptRecord)).toBe(true)
    expect(compactManaAttempt(reloaded as ManaAttemptRecord).sends).toMatchObject([{ stage: 'dispatched', hash: HASH }])
  })

  it('keeps an unanswered first send dispatched when a retry with the same signed bytes is refused', async () => {
    let stored: ManaAttemptRecord | undefined
    const keep = (record: ManaAttemptRecord) => {
      stored = structuredClone(record)
      return true
    }
    const shared = { mana: MANA, checkpoint: async () => 1000n, now: () => 5_000, load: () => stored, sendMs: 20 }
    const a = createManaAttempt(TERMS, keep, shared)
    keep(a.record)
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await expect(a.relay((() => new Promise<Response>(() => {})) as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))).rejects.toThrow(TypeError)
    const b = createManaAttempt(TERMS, keep, { ...shared, from: stored })
    b.retry()
    b.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    await b.relay(sent({ ok: false, not_sent: true }) as unknown as typeof fetch)('https://relay/transactions', post(STORE, PAYLOAD))
    // The same bytes, two requests: only the refused one is refused; the first may still have gone.
    expect(stored!.sends.map((x) => x.stage)).toEqual(['dispatched', 'refused'])
    expect(manaDispatched(stored!)).toBe(true)
  })

  it('bounds the relay’s whole answer: headers in time and a body that never ends still give way at the deadline', async () => {
    const a = attempt(keeper(), { sendMs: 20 })
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    const open = new ReadableStream<Uint8Array>({ start: () => {} })
    const started = Date.now()
    await expect(a.relay((async () => new Response(open, { status: 200 })) as unknown as typeof fetch)('https://relay/transactions', post(STORE))).rejects.toThrow(TypeError)
    expect(Date.now() - started).toBeLessThan(1000)
    expect(a.dispatched()).toBe(true)
  })

  it('hands the rails a buffered answer: its body read once, whole', async () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    const res = await a.relay(sent() as unknown as typeof fetch)('https://relay/transactions', post(STORE))
    expect(await res.json()).toEqual({ ok: true, txHash: HASH })
  })

  it('takes a kept record up again for a retry, and only for the same sale', () => {
    const a = attempt()
    a.handOff({ rail: 'relay', nonce: 3n, data: CALL })
    const again = attempt(keeper(), { from: a.record })
    expect(again.record).toEqual(a.record)
    expect(() => createManaAttempt({ ...TERMS, maxWei: 1n } as ManaSaleTerms, keeper().keep, { mana: MANA, checkpoint: async () => 1n, now: () => 1, from: a.record })).toThrow()
  })

  it('reads a stored attempt only in the shape it writes', () => {
    const good: ManaAttemptRecord = {
      venue: STORE,
      terms: { kind: 'mints', venue: STORE, units: [{ contract: '0x' + 'c0'.repeat(20), itemId: '7' }], maxWei: '2300000000000000000' },
      fingerprint: manaFingerprint(TERMS),
      nonce: '3',
      data: CALL,
      fromBlock: '99',
      signed: PAYLOAD as Hex,
      sends: [
        { id: 's16', stage: 'refused', payload: keccak256(PAYLOAD as Hex), at: 1 },
        { id: 's17', stage: 'dispatched', payload: keccak256(PAYLOAD as Hex), at: 2, hash: HASH }
      ]
    }
    expect(isManaAttempt(good)).toBe(true)
    // Every send carried the one signed execution the record keeps.
    expect(isManaAttempt({ ...good, signed: undefined })).toBe(false)
    expect(isManaAttempt({ ...good, sends: [good.sends[0], { ...good.sends[1], payload: keccak256('0x01') }] })).toBe(false)
    expect(isManaAttempt({ ...good, signed: '0x' })).toBe(false)
    expect(isManaAttempt({ ...good, lastSent: 2 })).toBe(true)
    expect(isManaAttempt({ ...good, lastSent: -1 })).toBe(false)
    expect(manaDispatched(good)).toBe(true)
    expect(isManaAttempt({ ...good, venue: 'nope' })).toBe(false)
    expect(isManaAttempt({ ...good, fingerprint: '0x12' })).toBe(false)
    expect(isManaAttempt({ ...good, nonce: undefined })).toBe(false)
    expect(isManaAttempt({ ...good, fromBlock: undefined })).toBe(false)
    expect(isManaAttempt({ ...good, sends: [{ id: 's18', stage: 'dispatched' }] })).toBe(false)
    expect(isManaAttempt({ ...good, sends: [{ stage: 'later' }] })).toBe(false)
    expect(isManaAttempt({ ...good, terms: { ...good.terms, venue: MANA } })).toBe(false)
  })

  it('tells two sales apart by their exact bytes or units and ceiling', () => {
    const listings = (calldata: Hex): ManaSaleTerms => ({ kind: 'listings', venue: STORE, calldata, units: TERMS.units })
    expect(manaFingerprint(TERMS)).toBe(manaFingerprint(structuredClone(TERMS)))
    expect(manaFingerprint(TERMS)).not.toBe(manaFingerprint({ ...TERMS, maxWei: TERMS.kind === 'mints' ? TERMS.maxWei + 1n : 0n }))
    expect(manaFingerprint(listings('0x1234'))).not.toBe(manaFingerprint(listings('0x1235')))
  })
})

// What the chain says about a MANA sale sent through the relay (shell/shop/manaEvidence): only an execution attributable
// to a signed send ends it as bought, and only a consumed nonce with no possible match ends it as not bought.
import { describe, expect, it, vi } from 'vitest'
import { encodeAbiParameters, encodeEventTopics, keccak256, parseAbiItem, type Hex } from 'viem'
import { boundedReader, createManaEvidence, decodeExecution, manaIncluded, resolveMana, type ManaEvidence, type ManaExecution } from '../shell/shop/manaEvidence'
import { createRpcReader } from '../../shop/checkout/chain/rpc'
import type { ManaAttemptRecord } from '../shell/shop/manaPurchase'

const STORE = '0x214ffc0f0103735728dc66b61a22e4f163e275ae'
const BUYER = '0x' + '0b'.repeat(20)
const CALL = '0xa4fdc78a00aa' as Hex
const SIGNED = ('0x0c53c51c' + 'ab'.repeat(40)) as Hex
const OTHER_SIGNED = ('0x0c53c51c' + 'cd'.repeat(40)) as Hex
const HASH = ('0x' + 'e1'.repeat(32)) as Hex
const RELAY_HASH = ('0x' + 'f1'.repeat(32)) as Hex
const OTHER_HASH = ('0x' + 'a2'.repeat(32)) as Hex
const F = 500n

const record = (over: Partial<ManaAttemptRecord> = {}): ManaAttemptRecord => ({
  venue: STORE,
  terms: { kind: 'mints', venue: STORE, units: [{ contract: '0x' + 'c0'.repeat(20), itemId: '7' }], maxWei: '1' },
  fingerprint: '0x' + '11'.repeat(32),
  nonce: '7',
  data: CALL,
  fromBlock: '100',
  sends: [{ id: 's1', stage: 'dispatched', payload: keccak256(SIGNED), at: 1 }],
  ...over
})

/**
 * A chain: the buyer's nonce now, the executions a log read returns (only those in the range asked for), and the
 * transactions and receipts it knows. As at a verified venue, each execution moved its signer's nonce once, so the nonce
 * at a block is today's less the executions after it, `unlogged` ones included (on chain, left out of every log read).
 */
function chain(opts: { now?: bigint; executions?: ManaExecution[] | Error; unlogged?: ManaExecution[]; txs?: Record<string, { to: string; input: Hex }>; receipts?: Record<string, { status: 'success' | 'reverted'; blockNumber: bigint }> } = {}) {
  const logged = opts.executions instanceof Error ? [] : (opts.executions ?? [])
  // The chain holds each execution once, however a log read repeats it.
  const all = [...new Map([...logged, ...(opts.unlogged ?? [])].map((e) => [`${e.blockNumber}:${e.logIndex}`, e])).values()]
  const evidence: ManaEvidence = {
    finalized: vi.fn(async () => F),
    nonceAt: vi.fn(async (_c, _v, user, block) => (opts.now ?? 7n) - BigInt(all.filter((e) => e.user === user && e.blockNumber > block).length)),
    executions: vi.fn(async (_c, _v, from, to) => {
      if (opts.executions instanceof Error) throw opts.executions
      return logged.filter((e) => e.blockNumber >= from && e.blockNumber <= to)
    }),
    transaction: vi.fn(async (_c, hash) => opts.txs?.[hash] ?? null),
    receipt: vi.fn(async (_c, hash) => {
      const r = opts.receipts?.[hash]
      return r ? { transactionHash: hash as Hex, status: r.status, blockNumber: r.blockNumber } : null
    })
  }
  return evidence
}

const resolve = (rec: ManaAttemptRecord, evidence: ManaEvidence) => resolveMana(rec, { chainId: 137, buyer: BUYER, venue: { address: STORE, metaTx: 'functionSignature' }, evidence })
const execution = (over: Partial<ManaExecution> = {}): ManaExecution => ({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 200n, logIndex: 0, ...over })

describe('a MANA sale on chain', () => {
  it('ends bought on the relay’s hash only when its transaction is a signed send of ours, finalized and successful', async () => {
    const rec = record({ sends: [{ id: 's2', stage: 'dispatched', payload: keccak256(SIGNED), at: 1, hash: RELAY_HASH }] })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toEqual({ kind: 'succeeded', txHash: RELAY_HASH })
    // Another signed payload, another contract, not finalized yet: not ours, or not final.
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: OTHER_SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: '0x' + '99'.repeat(20), input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: F + 1n } } }))).toMatchObject({ kind: 'waiting' })
  })

  it('takes the relay’s hash only past the checkpoint: the same signed bytes at or before it are another purchase', async () => {
    // A second identical mint signed at a stale nonce has the first one's bytes; the relay answers the first one's hash.
    const rec = record({ sends: [{ id: 's7', stage: 'dispatched', payload: keccak256(SIGNED), at: 1, hash: RELAY_HASH }] })
    for (const [block, out] of [
      [99n, { kind: 'failed' }],
      [100n, { kind: 'failed' }],
      [101n, { kind: 'succeeded', txHash: RELAY_HASH }]
    ] as const) {
      const evidence = chain({ now: 8n, executions: [execution({ blockNumber: block, txHash: RELAY_HASH })], txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: block } } })
      expect(await resolve(rec, evidence)).toEqual(out)
    }
    // At a venue whose evidence isn't verified, a hash at or before the checkpoint ends nothing.
    const other = '0x' + '77'.repeat(20)
    const before = chain({ now: 8n, executions: [execution({ blockNumber: 100n, txHash: RELAY_HASH })], txs: { [RELAY_HASH]: { to: other, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 100n } } })
    expect(await resolveMana(record({ venue: other, sends: rec.sends }), { chainId: 137, buyer: BUYER, venue: { address: other, metaTx: 'functionData' }, evidence: before })).toEqual({ kind: 'waiting', retry: false })
  })

  it('keeps waiting while the nonce isn’t consumed at the finalized block, open to RETRY unless a send is on its way', async () => {
    expect(await resolve(record(), chain({ now: 7n }))).toEqual({ kind: 'waiting', retry: true })
    expect(await resolve(record({ sends: [{ id: 's3', stage: 'dispatched', payload: keccak256(SIGNED), at: 1 }, { id: 's4', stage: 'signing' }] }), chain({ now: 7n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('ends not bought when the execution that consumed the nonce is another call', async () => {
    const evidence = chain({ now: 8n, executions: [execution({ user: '0x' + '22'.repeat(20) }), execution({ call: '0xbeef', logIndex: 1 })] })
    expect(await resolve(record(), evidence)).toEqual({ kind: 'failed' })
    // The nonce read at F and at the checkpoint; the logs of the one block that consumed N, never the range.
    expect(evidence.nonceAt).toHaveBeenCalledWith(137, { address: STORE, metaTx: 'functionSignature' }, BUYER, F)
    expect(evidence.nonceAt).toHaveBeenCalledWith(137, { address: STORE, metaTx: 'functionSignature' }, BUYER, 100n)
    expect(evidence.executions).toHaveBeenCalledWith(137, STORE, 200n, 200n)
    expect(evidence.executions).toHaveBeenCalledTimes(1)
  })

  it('ends bought on the execution that consumed the nonce, with this call, whatever transaction carried it', async () => {
    // No hash from the relay, and the transaction isn't one this page signed (a forwarder's): the execution at N decides.
    const evidence = chain({ now: 9n, executions: [execution({ call: '0xbeef', blockNumber: 150n }), execution({ txHash: RELAY_HASH, logIndex: 4 })] })
    expect(await resolve(record({ nonce: '8' }), evidence)).toEqual({ kind: 'succeeded', txHash: RELAY_HASH })
    // The same executions, the sale at the earlier nonce: there the other call ran, so it never can.
    expect(await resolve(record({ nonce: '7' }), evidence)).toEqual({ kind: 'failed' })
  })

  it('keeps waiting on a block’s log read that doesn’t hold every execution the nonce moved by', async () => {
    // N went to another call at block 200, which the node leaves out: the block's read holds one execution where the
    // nonce moved twice, so the one it shows (this call, at N+1) says nothing about N. And no RETRY: the nonce is gone.
    const unlogged = [execution({ call: '0xbeef', logIndex: 1, txHash: OTHER_HASH })]
    expect(await resolve(record(), chain({ now: 9n, executions: [execution({ logIndex: 2 })], unlogged }))).toEqual({ kind: 'waiting', retry: false })
    // A log the node repeats never stands in for the one it left out.
    expect(await resolve(record(), chain({ now: 9n, executions: [execution({ logIndex: 2 }), execution({ logIndex: 2 })], unlogged }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('ends bought on a forwarder’s transaction, by the execution that consumed N, and only that one', async () => {
    const rec = record({ sends: [{ id: 's6', stage: 'dispatched', payload: keccak256(SIGNED), at: 1, hash: RELAY_HASH }] })
    const forwarded = { [RELAY_HASH]: { to: '0x' + '99'.repeat(20), input: OTHER_SIGNED } }
    const final = { [RELAY_HASH]: { status: 'success' as const, blockNumber: 300n } }
    // Another of the buyer's executions in that block first (N−1), then ours (N). One at 150 the node leaves out is
    // never read: only the block that consumed N is.
    const block = [execution({ call: '0xbeef', blockNumber: 300n, logIndex: 1, txHash: OTHER_HASH }), execution({ blockNumber: 300n, logIndex: 2, txHash: RELAY_HASH })]
    const unlogged = [execution({ call: '0xbeef', blockNumber: 150n })]
    expect(await resolve(rec, chain({ now: 8n, executions: block, unlogged, txs: forwarded, receipts: final }))).toEqual({ kind: 'succeeded', txHash: RELAY_HASH })
    // At N, this call from another transaction (the relay's ran it at N−1): bought, by that other transaction, never
    // the relay's. At N, another call from the relay's: not bought. That block's read missing one: no word.
    const earlier = [execution({ blockNumber: 300n, logIndex: 1, txHash: RELAY_HASH }), execution({ blockNumber: 300n, logIndex: 2, txHash: OTHER_HASH })]
    expect(await resolve(rec, chain({ now: 8n, executions: earlier, unlogged, txs: forwarded, receipts: final }))).toEqual({ kind: 'succeeded', txHash: OTHER_HASH })
    const otherCall = [block[0], execution({ call: '0xbeef', blockNumber: 300n, logIndex: 2, txHash: RELAY_HASH })]
    expect(await resolve(rec, chain({ now: 8n, executions: otherCall, unlogged, txs: forwarded, receipts: final }))).toEqual({ kind: 'failed' })
    expect(await resolve(rec, chain({ now: 8n, executions: [block[1]], unlogged: [...unlogged, block[0]], txs: forwarded, receipts: final }))).toMatchObject({ kind: 'waiting' })
    // At an unverified venue, place proves nothing: only the signed execution itself would.
    const other = '0x' + '77'.repeat(20)
    const out = await resolveMana(record({ venue: other, sends: rec.sends }), { chainId: 137, buyer: BUYER, venue: { address: other, metaTx: 'functionData' }, evidence: chain({ now: 8n, executions: block, unlogged, txs: forwarded, receipts: final }) })
    expect(out).toEqual({ kind: 'waiting', retry: false })
  })

  it('ends not bought when N was consumed by the checkpoint, even by this very call: no send of this sale was out yet', async () => {
    // N went at block 98, before the checkpoint at 100 (another call while the wallet's prompt was open, or the same mint
    // bought before, read from a lagging node).
    for (const call of ['0xbeef', CALL] as const) {
      const evidence = chain({ now: 8n, executions: [execution({ call, blockNumber: 98n })] })
      expect(await resolve(record(), evidence)).toEqual({ kind: 'failed' })
      expect(evidence.nonceAt).toHaveBeenCalledWith(137, { address: STORE, metaTx: 'functionSignature' }, BUYER, 100n)
      expect(evidence.executions).not.toHaveBeenCalled()
    }
    // At the checkpoint block itself too: it was final before the first request left.
    expect(await resolve(record(), chain({ now: 8n, executions: [execution({ blockNumber: 100n })] }))).toEqual({ kind: 'failed' })
    // Just past it, this call is the sale.
    expect(await resolve(record(), chain({ now: 8n, executions: [execution({ blockNumber: 101n })] }))).toEqual({ kind: 'succeeded', txHash: HASH })
  })

  it('keeps waiting, with no RETRY, on a nonce read below the sale’s (a lagging node)', async () => {
    expect(await resolve(record(), chain({ now: 6n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('keeps waiting on a range that would read backwards (the finalized block below the checkpoint)', async () => {
    const evidence = chain({ now: 8n })
    expect(await resolve(record({ fromBlock: String(F + 1n) }), evidence)).toEqual({ kind: 'waiting', retry: false })
    expect(evidence.executions).not.toHaveBeenCalled()
  })

  it('offers no RETRY on a finalized block below the checkpoint, even with the nonce exactly the sale’s', async () => {
    expect(await resolve(record({ fromBlock: String(F + 1n) }), chain({ now: 7n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('gives up on a chain read that stalls, so a later look can try again', async () => {
    const stalled = (_url: string, init?: RequestInit) =>
      new Promise<Response>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('timed out', 'TimeoutError'))))
    const reader = { call: async () => '0x' as Hex, receipt: () => new Promise<null>(() => {}), balance: async () => 0n, code: async () => '0x' as Hex }
    const evidence = createManaEvidence({ url: () => 'https://rpc.test', fetchImpl: stalled as unknown as typeof fetch, reader, readMs: 20 })
    await expect(evidence.finalized(137)).rejects.toThrow()
    await expect(evidence.receipt(137, HASH)).rejects.toThrow(/too long/)
  })

  it('aborts a bounded read past its deadline, so its request doesn’t stay open', async () => {
    const aborted: string[] = []
    const stalled = (_url: string, init?: RequestInit) =>
      new Promise<Response>((_, reject) =>
        init?.signal?.addEventListener('abort', () => {
          aborted.push(JSON.parse(String(init.body)).method)
          reject(new DOMException('aborted', 'AbortError'))
        })
      )
    const rpc = createRpcReader({ url: () => 'https://rpc.test', fetchImpl: stalled as unknown as typeof fetch })
    await expect(boundedReader(rpc, 20).call(137, STORE, '0x')).rejects.toThrow()
    const evidence = createManaEvidence({ url: () => 'https://rpc.test', fetchImpl: stalled as unknown as typeof fetch, reader: rpc, readMs: 20 })
    await expect(evidence.receipt(137, HASH)).rejects.toThrow()
    await vi.waitFor(() => expect(aborted).toEqual(['eth_call', 'eth_getTransactionReceipt']))
  })

  it('never ends a sale as not bought at a venue whose evidence isn’t verified', async () => {
    const other = '0x' + '77'.repeat(20)
    const evidence = chain({ now: 8n, executions: [execution({ call: '0xbeef' })] })
    const out = await resolveMana(record({ venue: other }), { chainId: 137, buyer: BUYER, venue: { address: other, metaTx: 'functionData' }, evidence })
    expect(out).toEqual({ kind: 'waiting', retry: false })
  })

  it('throws on a log range it can’t read, so nothing settles on half a range', async () => {
    await expect(resolve(record(), chain({ now: 8n, executions: new Error('range too large'), unlogged: [execution({ call: '0xbeef' })] }))).rejects.toThrow(/range too large/)
  })

  it('keeps waiting for a record without what this needs, and for one that sent nothing', async () => {
    expect(await resolve(record({ fromBlock: undefined }), chain({ now: 8n }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(record({ sends: [{ id: 's5', stage: 'refused', payload: keccak256(SIGNED), at: 1 }] }), chain({ now: 8n }))).toMatchObject({ kind: 'waiting' })
  })

  it('reads an execution in both layouts the venues log it in', () => {
    const event = parseAbiItem('event MetaTransactionExecuted(address indexed userAddress, address indexed relayerAddress, bytes functionData)')
    const relayer = '0x' + '33'.repeat(20)
    const indexed = decodeExecution({
      topics: encodeEventTopics({ abi: [event], eventName: 'MetaTransactionExecuted', args: { userAddress: BUYER as Hex, relayerAddress: relayer as Hex } }) as Hex[],
      data: encodeAbiParameters([{ type: 'bytes' }], [CALL]),
      transactionHash: HASH,
      blockNumber: '0x10',
      logIndex: '0x2'
    })
    expect(indexed).toEqual({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 16n, logIndex: 2 })
    const flat = decodeExecution({
      topics: ['0x5845892132946850460bff5a0083f71031bc5bf9aadcd40f1de79423eac9b10b'],
      data: encodeAbiParameters([{ type: 'address' }, { type: 'address' }, { type: 'bytes' }], [BUYER as Hex, relayer as Hex, CALL]),
      transactionHash: HASH,
      blockNumber: '0x11',
      logIndex: '0x0'
    })
    expect(flat).toEqual({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 17n, logIndex: 0 })
    expect(() => decodeExecution({ topics: ['0x01', '0x02'], data: '0x', transactionHash: HASH, blockNumber: '0x1', logIndex: '0x0' })).toThrow()
    // Without its place in the block it can't be ordered, so it can't be read.
    expect(() => decodeExecution({ topics: ['0x5845892132946850460bff5a0083f71031bc5bf9aadcd40f1de79423eac9b10b'], data: '0x', transactionHash: HASH, blockNumber: '0x1' } as never)).toThrow(/place/)
  })

  it('lets an error answer’s body go instead of leaving it open', async () => {
    let cancelled = false
    const body = new ReadableStream({ pull: () => new Promise(() => {}), cancel: () => void (cancelled = true) })
    const fetchImpl = async () => new Response(body, { status: 503 })
    const reader = { call: async () => '0x' as Hex, receipt: async () => null, balance: async () => 0n, code: async () => '0x' as Hex }
    const evidence = createManaEvidence({ url: () => 'https://rpc.test', fetchImpl: fetchImpl as unknown as typeof fetch, reader })
    await expect(evidence.finalized(137)).rejects.toThrow(/HTTP 503/)
    await vi.waitFor(() => expect(cancelled).toBe(true))
  })
})

describe('a MANA sale on chain before it is final', () => {
  const included = (rec: ManaAttemptRecord, evidence: ManaEvidence) => manaIncluded(rec, { chainId: 137, buyer: BUYER, venue: { address: STORE, metaTx: 'functionSignature' }, evidence })
  const sent = record({ sends: [{ id: 's2', stage: 'dispatched', payload: keccak256(SIGNED), at: 1, hash: RELAY_HASH }] })

  it('holds once the relay’s hash is this sale’s own signed execution past the checkpoint, final or not', async () => {
    expect(await included(sent, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: F + 50n } } }))).toBe(true)
  })

  it('doesn’t hold on a receipt alone: another transaction, another contract, at or before the checkpoint, reverted, or no hash', async () => {
    const ok = { status: 'success' as const, blockNumber: 300n }
    expect(await included(sent, chain({ txs: { [RELAY_HASH]: { to: STORE, input: OTHER_SIGNED } }, receipts: { [RELAY_HASH]: ok } }))).toBe(false)
    expect(await included(sent, chain({ txs: { [RELAY_HASH]: { to: '0x' + '99'.repeat(20), input: SIGNED } }, receipts: { [RELAY_HASH]: ok } }))).toBe(false)
    expect(await included(sent, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 100n } } }))).toBe(false)
    expect(await included(sent, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'reverted', blockNumber: 300n } } }))).toBe(false)
    expect(await included(record(), chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: ok } }))).toBe(false)
  })

  it('holds for a forwarder’s transaction whose block holds the execution that consumed the nonce with this call, never another call', async () => {
    const forwarder = '0x' + '99'.repeat(20)
    const at = (call: Hex) =>
      chain({ now: 8n, executions: [execution({ blockNumber: 300n, txHash: RELAY_HASH, call })], txs: { [RELAY_HASH]: { to: forwarder, input: OTHER_SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } })
    expect(await included(sent, at(CALL))).toBe(true)
    expect(await included(sent, at('0xbeef'))).toBe(false)
  })
})

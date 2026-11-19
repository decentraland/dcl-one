// What the chain says about a MANA sale sent through the relay (shell/shop/manaEvidence): only an execution attributable
// to a signed send ends it as bought, and only a consumed nonce with no possible match ends it as not bought.
import { describe, expect, it, vi } from 'vitest'
import { encodeAbiParameters, encodeEventTopics, keccak256, parseAbiItem, type Hex } from 'viem'
import { createManaEvidence, decodeExecution, resolveMana, type ManaEvidence, type ManaExecution } from '../shell/shop/manaEvidence'
import type { ManaAttemptRecord } from '../shell/shop/manaPurchase'

const STORE = '0x214ffc0f0103735728dc66b61a22e4f163e275ae'
const BUYER = '0x' + '0b'.repeat(20)
const CALL = '0xa4fdc78a00aa' as Hex
const SIGNED = ('0x0c53c51c' + 'ab'.repeat(40)) as Hex
const OTHER_SIGNED = ('0x0c53c51c' + 'cd'.repeat(40)) as Hex
const HASH = ('0x' + 'e1'.repeat(32)) as Hex
const RELAY_HASH = ('0x' + 'f1'.repeat(32)) as Hex
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

/** A chain: the nonce at F, the executions in range, and the transactions and receipts it knows. */
function chain(opts: { nonce?: bigint; executions?: ManaExecution[] | Error; txs?: Record<string, { to: string; input: Hex }>; receipts?: Record<string, { status: 'success' | 'reverted'; blockNumber: bigint }> } = {}) {
  const evidence: ManaEvidence = {
    finalized: vi.fn(async () => F),
    latest: vi.fn(async () => F + 3n),
    nonceAt: vi.fn(async () => opts.nonce ?? 7n),
    executions: vi.fn(async () => {
      if (opts.executions instanceof Error) throw opts.executions
      return opts.executions ?? []
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
const execution = (over: Partial<ManaExecution> = {}): ManaExecution => ({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 200n, ...over })

describe('a MANA sale on chain', () => {
  it('ends bought on the relay’s hash only when its transaction is a signed send of ours, finalized and successful', async () => {
    const rec = record({ sends: [{ id: 's2', stage: 'dispatched', payload: keccak256(SIGNED), at: 1, hash: RELAY_HASH }] })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toEqual({ kind: 'succeeded', txHash: RELAY_HASH })
    // Another signed payload, another contract, not finalized yet: not ours, or not final.
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: OTHER_SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: '0x' + '99'.repeat(20), input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: 300n } } }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(rec, chain({ txs: { [RELAY_HASH]: { to: STORE, input: SIGNED } }, receipts: { [RELAY_HASH]: { status: 'success', blockNumber: F + 1n } } }))).toMatchObject({ kind: 'waiting' })
  })

  it('keeps waiting while the nonce isn’t consumed at the finalized block, open to RETRY unless a send is on its way', async () => {
    expect(await resolve(record(), chain({ nonce: 7n }))).toEqual({ kind: 'waiting', retry: true })
    expect(await resolve(record({ sends: [{ id: 's3', stage: 'dispatched', payload: keccak256(SIGNED), at: 1 }, { id: 's4', stage: 'signing' }] }), chain({ nonce: 7n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('ends not bought when the nonce was consumed and no execution of this call by the buyer is in range', async () => {
    const evidence = chain({ nonce: 8n, executions: [execution({ user: '0x' + '22'.repeat(20) }), execution({ call: '0xbeef' })] })
    expect(await resolve(record(), evidence)).toEqual({ kind: 'failed' })
    // Read from the first dispatch's checkpoint through the very block the nonce was read at.
    expect(evidence.executions).toHaveBeenCalledWith(137, STORE, 100n, F)
    expect(evidence.nonceAt).toHaveBeenCalledWith(137, { address: STORE, metaTx: 'functionSignature' }, BUYER, F)
  })

  it('ends bought on a matching execution attributable to a signed send of ours', async () => {
    const evidence = chain({ nonce: 8n, executions: [execution()], txs: { [HASH]: { to: STORE, input: SIGNED } }, receipts: { [HASH]: { status: 'success', blockNumber: 200n } } })
    expect(await resolve(record(), evidence)).toEqual({ kind: 'succeeded', txHash: HASH })
  })

  it('keeps waiting on the same call by the buyer that it can’t attribute (another signature, at another nonce)', async () => {
    const evidence = chain({ nonce: 9n, executions: [execution()], txs: { [HASH]: { to: STORE, input: OTHER_SIGNED } }, receipts: { [HASH]: { status: 'success', blockNumber: 200n } } })
    // And no RETRY: the nonce is gone, so a retry could never run.
    expect(await resolve(record(), evidence)).toEqual({ kind: 'waiting', retry: false })
  })

  it('keeps waiting, with no RETRY, on a nonce read below the sale’s (a lagging node)', async () => {
    expect(await resolve(record(), chain({ nonce: 6n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('keeps waiting on a range that would read backwards (the finalized block below the checkpoint)', async () => {
    const evidence = chain({ nonce: 8n })
    expect(await resolve(record({ fromBlock: String(F + 1n) }), evidence)).toEqual({ kind: 'waiting', retry: false })
    expect(evidence.executions).not.toHaveBeenCalled()
  })

  it('offers no RETRY on a finalized block below the checkpoint, even with the nonce exactly the sale’s', async () => {
    expect(await resolve(record({ fromBlock: String(F + 1n) }), chain({ nonce: 7n }))).toEqual({ kind: 'waiting', retry: false })
  })

  it('gives up on a chain read that stalls, so a later look can try again', async () => {
    const stalled = (_url: string, init?: RequestInit) =>
      new Promise<Response>((_, reject) => init?.signal?.addEventListener('abort', () => reject(new DOMException('timed out', 'TimeoutError'))))
    const reader = { call: async () => '0x' as Hex, receipt: () => new Promise<null>(() => {}), balance: async () => 0n, code: async () => '0x' as Hex }
    const evidence = createManaEvidence({ url: () => 'https://rpc.test', fetchImpl: stalled as unknown as typeof fetch, reader, readMs: 20 })
    await expect(evidence.finalized(137)).rejects.toThrow()
    await expect(evidence.receipt(137, HASH)).rejects.toThrow(/too long/)
  })

  it('never ends a sale as not bought at a venue whose evidence isn’t verified', async () => {
    const other = '0x' + '77'.repeat(20)
    const evidence = chain({ nonce: 8n })
    const out = await resolveMana(record({ venue: other }), { chainId: 137, buyer: BUYER, venue: { address: other, metaTx: 'functionData' }, evidence })
    expect(out).toEqual({ kind: 'waiting', retry: false })
  })

  it('throws on a log range it can’t read, so nothing settles on half a range', async () => {
    await expect(resolve(record(), chain({ nonce: 8n, executions: new Error('range too large') }))).rejects.toThrow()
  })

  it('keeps waiting for a record without what this needs, and for one that sent nothing', async () => {
    expect(await resolve(record({ fromBlock: undefined }), chain({ nonce: 8n }))).toMatchObject({ kind: 'waiting' })
    expect(await resolve(record({ sends: [{ id: 's5', stage: 'refused', payload: keccak256(SIGNED), at: 1 }] }), chain({ nonce: 8n }))).toMatchObject({ kind: 'waiting' })
  })

  it('reads an execution in both layouts the venues log it in', () => {
    const event = parseAbiItem('event MetaTransactionExecuted(address indexed userAddress, address indexed relayerAddress, bytes functionData)')
    const relayer = '0x' + '33'.repeat(20)
    const indexed = decodeExecution({
      topics: encodeEventTopics({ abi: [event], eventName: 'MetaTransactionExecuted', args: { userAddress: BUYER as Hex, relayerAddress: relayer as Hex } }) as Hex[],
      data: encodeAbiParameters([{ type: 'bytes' }], [CALL]),
      transactionHash: HASH,
      blockNumber: '0x10'
    })
    expect(indexed).toEqual({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 16n })
    const flat = decodeExecution({
      topics: ['0x5845892132946850460bff5a0083f71031bc5bf9aadcd40f1de79423eac9b10b'],
      data: encodeAbiParameters([{ type: 'address' }, { type: 'address' }, { type: 'bytes' }], [BUYER as Hex, relayer as Hex, CALL]),
      transactionHash: HASH,
      blockNumber: '0x11'
    })
    expect(flat).toEqual({ user: BUYER, call: CALL, txHash: HASH, blockNumber: 17n })
    expect(() => decodeExecution({ topics: ['0x01', '0x02'], data: '0x', transactionHash: HASH, blockNumber: '0x1' })).toThrow()
  })
})

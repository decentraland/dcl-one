// What the chain says about a MANA sale this page sent through the relay (./manaPurchase): whether it executed, and
// where, attributed to the very execution this page signed. The sale is a meta-transaction at the venue (a Polygon
// marketplace, or the CollectionStore), signed with the buyer's meta-transaction nonce N; every execution at the venue
// emits MetaTransactionExecuted(user, relayer, call bytes), which every supported deployment does (Polygon and Amoy,
// the CollectionStore and each marketplace version) and N can be consumed once only.
//
// It settles only on evidence read through one finalized block F:
//  - succeeded: a successful receipt, at or before F, of a transaction whose input is exactly a signed execution this
//    page sent (its digest kept at dispatch). Identical call bytes alone don't attribute it: the same sale may run at
//    another nonce, signed elsewhere;
//  - failed: N was consumed by F, and the venue's executions from the first dispatch's checkpoint through F hold no
//    possible match (the buyer and the exact call bytes). Then this sale ran at no nonce up to N, and can't run later;
//  - waiting: anything else. N not consumed yet (the sale may still run), a match it can't attribute, a read that fails,
//    or a record without what this needs.

import { decodeAbiParameters, decodeEventLog, keccak256, parseAbiItem, type Hex } from 'viem'
import type { ChainId } from '@shop/model/chain'
import { readMetaTxNonce } from '../../../shop/checkout/chain/metatx'
import { rpcUrl } from '../../../shop/checkout/chain/config'
import type { ChainReader, TxReceipt } from '../../../shop/checkout/chain/rpc'
import type { MetaTxStyle } from '../../../shop/checkout/chain/contracts'
import type { ManaAttemptRecord } from './manaPurchase'

/** One execution at a venue, as its log reads. */
export interface ManaExecution {
  user: string
  call: Hex
  txHash: Hex
  blockNumber: bigint
}

/** The chain reads this needs, each through an explicit block where it matters. */
export interface ManaEvidence {
  finalized(chainId: number): Promise<bigint>
  latest(chainId: number): Promise<bigint>
  /** The venue's meta-transaction nonce for `user`, as of `block`. */
  nonceAt(chainId: number, venue: { address: string; metaTx: MetaTxStyle }, user: string, block: bigint): Promise<bigint>
  /** Every MetaTransactionExecuted of `venue` in [from, to], complete or a throw. */
  executions(chainId: number, venue: string, from: bigint, to: bigint): Promise<ManaExecution[]>
  /** A transaction's target and input, or null when the chain doesn't know it. */
  transaction(chainId: number, hash: string): Promise<{ to: string | null; input: Hex } | null>
  receipt(chainId: number, hash: string): Promise<TxReceipt | null>
}

const EXECUTED = parseAbiItem('event MetaTransactionExecuted(address indexed userAddress, address indexed relayerAddress, bytes functionData)')
/** keccak256("MetaTransactionExecuted(address,address,bytes)"): the same topic whether the addresses are indexed or not. */
const EXECUTED_TOPIC = '0x5845892132946850460bff5a0083f71031bc5bf9aadcd40f1de79423eac9b10b'
/** Blocks per log request: under what the configured RPCs take in one range. */
const LOG_SPAN = 5000n

type RawLog = { topics: Hex[]; data: Hex; transactionHash: Hex; blockNumber: Hex }

/**
 * One execution's log, in either layout the venues use: the marketplaces index the user and the relayer (three
 * topics), the CollectionStore keeps all three in its data (one topic).
 */
export function decodeExecution(log: RawLog): ManaExecution {
  if (log.topics.length === 3) {
    const { args } = decodeEventLog({ abi: [EXECUTED], data: log.data, topics: log.topics as [Hex, Hex, Hex] })
    return { user: args.userAddress.toLowerCase(), call: args.functionData, txHash: log.transactionHash, blockNumber: BigInt(log.blockNumber) }
  }
  if (log.topics.length !== 1) throw new Error('A MetaTransactionExecuted log in an unknown layout')
  const [user, , call] = decodeAbiParameters([{ type: 'address' }, { type: 'address' }, { type: 'bytes' }], log.data)
  return { user: user.toLowerCase(), call, txHash: log.transactionHash, blockNumber: BigInt(log.blockNumber) }
}

/** The chain reads over each chain's configured RPC, as the Shop's own reader reaches it. */
export function createManaEvidence(opts: { url?: (chainId: ChainId) => string | null; fetchImpl?: typeof fetch; reader: ChainReader }): ManaEvidence {
  const urlFor = opts.url ?? rpcUrl
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a))
  let id = 1
  const rpc = async <T>(chainId: number, method: string, params: unknown[]): Promise<T> => {
    const url = urlFor(chainId as ChainId)
    if (!url) throw new Error(`No RPC is configured for chain ${chainId}`)
    const res = await fetchImpl(url, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: id++, method, params }) })
    if (!res.ok) throw new Error(`RPC ${method} answered HTTP ${res.status}`)
    const body = (await res.json()) as { result?: T; error?: { message?: string } }
    if (body.error) throw new Error(body.error.message ?? `RPC ${method} failed`)
    return body.result as T
  }
  const hex = (n: bigint): string => `0x${n.toString(16)}`
  return {
    finalized: async (chainId) => BigInt((await rpc<{ number: Hex }>(chainId, 'eth_getBlockByNumber', ['finalized', false])).number),
    latest: async (chainId) => BigInt(await rpc<Hex>(chainId, 'eth_blockNumber', [])),
    nonceAt: async (chainId, venue, user, block) => {
      // The Shop's own nonce read, asked at the block: the reader's call is redirected through it.
      const atBlock: ChainReader = { ...opts.reader, call: (c, to, data) => rpc<Hex>(c, 'eth_call', [{ to, data }, hex(block)]) }
      return readMetaTxNonce(atBlock, { address: venue.address as Hex, chainId: chainId as ChainId, metaTx: venue.metaTx }, user)
    },
    executions: async (chainId, venue, from, to) => {
      const out: ManaExecution[] = []
      for (let start = from; start <= to; start += LOG_SPAN) {
        const end = start + LOG_SPAN - 1n < to ? start + LOG_SPAN - 1n : to
        const logs = await rpc<RawLog[]>(chainId, 'eth_getLogs', [{ address: venue, topics: [EXECUTED_TOPIC], fromBlock: hex(start), toBlock: hex(end) }])
        if (!Array.isArray(logs)) throw new Error('eth_getLogs answered no list')
        for (const log of logs) out.push(decodeExecution(log))
      }
      return out
    },
    transaction: async (chainId, hash) => {
      const tx = await rpc<{ to: string | null; input: Hex } | null>(chainId, 'eth_getTransactionByHash', [hash])
      return tx ? { to: tx.to, input: tx.input } : null
    },
    receipt: (chainId, hash) => opts.reader.receipt(chainId as ChainId, hash)
  }
}

export type ManaResolution =
  | { kind: 'succeeded'; txHash: Hex }
  | { kind: 'failed' }
  /**
   * `retry`: its nonce wasn't consumed as of the finalized block and none of its sends is on its way, so the same sale
   * may be sent again at that nonce (once enough time passed); never while a match waits to be attributed.
   */
  | { kind: 'waiting'; retry: boolean }

/**
 * How the attempt's sale ended, from the chain, or that it hasn't yet. `venue` is the registry's entry for the sale's
 * venue (its meta-transaction style decides how the nonce reads).
 */
export async function resolveMana(
  attempt: ManaAttemptRecord,
  ctx: { chainId: number; buyer: string; venue: { address: string; metaTx: MetaTxStyle }; evidence: ManaEvidence }
): Promise<ManaResolution> {
  const idle = !attempt.sends.some((s) => s.stage === 'signing')
  const payloads = new Set(attempt.sends.flatMap((s) => (s.stage === 'dispatched' && s.payload ? [s.payload] : [])))
  if (attempt.nonce == null || attempt.data == null || attempt.fromBlock == null || payloads.size === 0) return { kind: 'waiting', retry: false }
  const { chainId, evidence } = ctx
  const buyer = ctx.buyer.toLowerCase()
  const venue = ctx.venue.address.toLowerCase()
  const finalized = await evidence.finalized(chainId)
  /** A finalized, successful execution of a signed payload this page sent. */
  const attributed = async (hash: string): Promise<boolean> => {
    const [tx, receipt] = await Promise.all([evidence.transaction(chainId, hash), evidence.receipt(chainId, hash)])
    return !!tx && !!receipt && receipt.status === 'success' && receipt.blockNumber <= finalized && tx.to?.toLowerCase() === venue && payloads.has(keccak256(tx.input))
  }
  // The relay's own answers first: a hash it gave for one of these sends.
  for (const send of attempt.sends) {
    if (send.hash && (await attributed(send.hash))) return { kind: 'succeeded', txHash: send.hash as Hex }
  }
  const nonce = BigInt(attempt.nonce)
  if ((await evidence.nonceAt(chainId, ctx.venue, buyer, finalized)) <= nonce) return { kind: 'waiting', retry: idle }
  // N was consumed by F: the sale ran at N (one of these sends), or it never can.
  const call = keccak256(attempt.data)
  const matches = (await evidence.executions(chainId, venue, BigInt(attempt.fromBlock), finalized)).filter((e) => e.user === buyer && keccak256(e.call) === call)
  if (matches.length === 0) return { kind: 'failed' }
  for (const match of matches) {
    if (await attributed(match.txHash)) return { kind: 'succeeded', txHash: match.txHash }
  }
  return { kind: 'waiting', retry: false }
}

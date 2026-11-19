// What the chain says about a MANA sale this page sent through the relay (./manaPurchase): whether it executed, and
// where, attributed to the very execution this page signed. The sale is a meta-transaction at the venue (a Polygon
// marketplace, or the CollectionStore), signed with the buyer's meta-transaction nonce N.
//
// The venues it settles on are pinned (VERIFIED_VENUES), each one's runtime code checked against verified source. In
// that source (NativeMetaTransaction), executeMetaTransaction checks the signature over nonces[user], increments it (the
// only write to it), emits MetaTransactionExecuted(user, relayer, call bytes) and reverts everything if the call fails.
// So a successful execution always logs its call, N is consumed once and never again, and the code can't change (no
// proxy, no delegatecall, no selfdestruct). For a venue not pinned there, nothing is ever ended as not bought.
//
// It settles only on evidence read through one finalized block F:
//  - succeeded: a successful receipt, at or before F, of a transaction whose input is exactly a signed execution this
//    page sent (its digest kept at dispatch). Identical call bytes alone don't attribute it: the same sale may run at
//    another nonce, signed elsewhere;
//  - failed: at a pinned venue, N was consumed by F, and the venue's executions from the checkpoint kept before the
//    first dispatch (a finalized block, so no reorg can move the sale below it) through F hold no possible match (the
//    buyer and the exact call bytes). Then this sale ran at no nonce up to N, and can't run later;
//  - waiting: anything else. N not consumed yet (the sale may still run), a nonce read below N (a lagging node), a
//    range that reads backwards, a match it can't attribute, a read that fails, or a record without what this needs.

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

/**
 * The venues a missing execution may end a sale as not bought at, by chain, lowercase. Each one's deployed runtime code
 * is the verified source's: Sourcify reports an exact match for every one but Amoy's OffChainMarketplaceV3, whose code
 * differs from Polygon's (exact match) only inside that source's immutable slots. Checked on 19 Nov 2026.
 */
export const VERIFIED_VENUES: Readonly<Record<number, readonly string[]>> = {
  137: [
    '0x214ffc0f0103735728dc66b61a22e4f163e275ae', // CollectionStore
    '0xe38ef22abe871513555cba89adfe45ab4f548ada', // OffChainMarketplaceV3
    '0xa40b1d129b8906888720686f3a01921ddf37716f', // OffChainMarketplaceV2
    '0x540fb08edb56aae562864b390542c97f562825ba' // OffChainMarketplace
  ],
  80002: [
    '0xe36abc9ec616c83caaa386541380829106149d68', // CollectionStore
    '0x36fd1434a6c4b8ade80c9847c1d15033ce34488c', // OffChainMarketplaceV3
    '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7', // OffChainMarketplaceV2
    '0x6ab20ae56673ed65f520b7be332aeb61b3ed727d' // OffChainMarketplace
  ]
}

/** Whether the chain's evidence about `venue` may end a sale either way (VERIFIED_VENUES). */
export function verifiedVenue(chainId: number, venue: string): boolean {
  return VERIFIED_VENUES[chainId]?.includes(venue.toLowerCase()) ?? false
}

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
   * `retry`: the finalized block reads its nonce exactly (unused, and not a lagging read below it) and none of its sends
   * is on its way, so the same sale may be sent again at that nonce (once enough time passed and while the record has
   * room for its evidence); never while a match waits to be attributed.
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
  const read = await evidence.nonceAt(chainId, ctx.venue, buyer, finalized)
  if (read === nonce) return { kind: 'waiting', retry: idle }
  if (read < nonce) return { kind: 'waiting', retry: false }
  // N was consumed by F: the sale ran at N (one of these sends), or it never can.
  const fromBlock = BigInt(attempt.fromBlock)
  if (finalized < fromBlock) return { kind: 'waiting', retry: false }
  const call = keccak256(attempt.data)
  const matches = (await evidence.executions(chainId, venue, fromBlock, finalized)).filter((e) => e.user === buyer && keccak256(e.call) === call)
  if (matches.length === 0) return verifiedVenue(chainId, venue) ? { kind: 'failed' } : { kind: 'waiting', retry: false }
  for (const match of matches) {
    if (await attributed(match.txHash)) return { kind: 'succeeded', txHash: match.txHash }
  }
  return { kind: 'waiting', retry: false }
}

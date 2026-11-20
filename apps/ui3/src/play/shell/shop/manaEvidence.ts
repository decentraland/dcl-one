// What the chain says about a MANA sale this page sent through the relay (./manaPurchase): whether it executed, and
// where. The sale is a meta-transaction at the venue (a Polygon marketplace, or the CollectionStore), signed with the
// buyer's meta-transaction nonce N.
//
// The venues it settles on are pinned (VERIFIED_VENUES), each one's runtime code checked against verified source. In
// that source (NativeMetaTransaction), executeMetaTransaction checks the signature over nonces[user], increments it (the
// only write to it), emits MetaTransactionExecuted(user, relayer, call bytes) and reverts everything if the call fails.
// So each successful execution consumes its signer's nonce once and logs its call, nothing else moves that nonce, and
// the code can't change (no proxy, no delegatecall, no selfdestruct). The buyer's executions, in chain order, are
// therefore their nonces in order: the block where the nonce first reads past N holds the execution that consumed N, at
// place N − (the nonce before that block) among the buyer's executions there, whatever transaction carried it (the
// relay's own, or a forwarder its upstream uses). A block's logs are read whole only when they hold exactly as many of
// the buyer's executions as the nonce moved in it (a repeated log counted once); any other read decides nothing.
//
// It settles only on evidence read through one finalized block F:
//  - succeeded: at a pinned venue, the execution that consumed N past the checkpoint is this call; or a successful
//    receipt, past the checkpoint and at or before F, of a transaction whose input is exactly the signed execution this
//    page sent;
//  - failed: at a pinned venue, N was consumed by the checkpoint, or the execution that consumed it past the checkpoint
//    is another call. Then no send of this sale can ever run;
//  - waiting: anything else. N not consumed yet (the sale may still run), a nonce read below N (a lagging node), a
//    finalized block below the checkpoint, a block read that isn't whole, a read that fails, or a record without what
//    this needs. At a venue not pinned, only a transaction that is exactly the signed execution ends it, as bought.
//
// No send of the sale can be in a block at or before the checkpoint: that block was finalized before the first relay
// request left. So N consumed by then (another call ran at N while the wallet's prompt was open, or N was read from a
// lagging node) ends it as not bought, whatever call consumed it. Past the checkpoint, the block that consumed N is
// found from the nonce alone (it only grows), by halving between the checkpoint and F.

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
  /** Its place among the block's logs. */
  logIndex: number
}

/** The chain reads this needs, each through an explicit block where it matters. */
export interface ManaEvidence {
  finalized(chainId: number): Promise<bigint>
  /** The finalized block's time (unix seconds): every later block's is past it. */
  finalizedTime?(chainId: number): Promise<number>
  /** The venue's meta-transaction nonce for `user`, as of `block`. */
  nonceAt(chainId: number, venue: { address: string; metaTx: MetaTxStyle }, user: string, block: bigint): Promise<bigint>
  /** Every MetaTransactionExecuted of `venue` in [from, to], complete or a throw. */
  executions(chainId: number, venue: string, from: bigint, to: bigint): Promise<ManaExecution[]>
  /**
   * A transaction's target, input and sender, or null when the chain doesn't know it. The sender is what tells whose a
   * useCredits the buyer's own wallet sent is (a relayed one names its user inside the call).
   */
  transaction(chainId: number, hash: string): Promise<{ to: string | null; input: Hex; from?: string | null } | null>
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
const VERIFIED_VENUES: Readonly<Record<number, readonly string[]>> = {
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

type RawLog = { topics: Hex[]; data: Hex; transactionHash: Hex; blockNumber: Hex; logIndex: Hex }

/**
 * One execution's log, in either layout the venues use: the marketplaces index the user and the relayer (three
 * topics), the CollectionStore keeps all three in its data (one topic).
 */
export function decodeExecution(log: RawLog): ManaExecution {
  // Its place orders it among the block's executions, so a log without one can't be read.
  const place = typeof log.logIndex === 'string' && /^0x[0-9a-fA-F]+$/.test(log.logIndex) ? Number(log.logIndex) : NaN
  if (!Number.isSafeInteger(place)) throw new Error('A MetaTransactionExecuted log without its place in the block')
  if (log.topics.length === 3) {
    const { args } = decodeEventLog({ abi: [EXECUTED], data: log.data, topics: log.topics as [Hex, Hex, Hex] })
    return { user: args.userAddress.toLowerCase(), call: args.functionData, txHash: log.transactionHash, blockNumber: BigInt(log.blockNumber), logIndex: place }
  }
  if (log.topics.length !== 1) throw new Error('A MetaTransactionExecuted log in an unknown layout')
  const [user, , call] = decodeAbiParameters([{ type: 'address' }, { type: 'address' }, { type: 'bytes' }], log.data)
  return { user: user.toLowerCase(), call, txHash: log.transactionHash, blockNumber: BigInt(log.blockNumber), logIndex: place }
}

/** How long one chain read may take before it gives way: the watch reads again later. */
export const EVIDENCE_READ_MS = 15_000

/** What a wait that ran past its deadline rejects with. */
export class DeadlineError extends Error {
  override name = 'DeadlineError'
}

/** `promise`, or a rejection once `ms` passed: a stalled read never holds a later one up. */
export function deadline<T>(promise: Promise<T>, ms: number, what = 'A chain read'): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new DeadlineError(`${what} took too long`)), ms)
    promise.then(
      (v) => {
        clearTimeout(timer)
        resolve(v)
      },
      (e: unknown) => {
        clearTimeout(timer)
        reject(e)
      }
    )
  })
}

/**
 * `reader` with each read bounded by `ms`: a read that stalls rejects, and whoever polls (the rails' receipt wait) reads
 * again or gives up on its own deadline, instead of waiting on it forever. The read is also aborted then (its `signal`),
 * so a reader that honours it lets the request go rather than holding it open.
 */
export function boundedReader(reader: ChainReader, ms: number): ChainReader {
  return {
    call: (chainId, to, data) => deadline(reader.call(chainId, to, data, AbortSignal.timeout(ms)), ms),
    receipt: (chainId, hash) => deadline(reader.receipt(chainId, hash, AbortSignal.timeout(ms)), ms),
    balance: (chainId, address) => deadline(reader.balance(chainId, address, AbortSignal.timeout(ms)), ms),
    code: (chainId, address) => deadline(reader.code(chainId, address, AbortSignal.timeout(ms)), ms)
  }
}

/** The chain reads over each chain's configured RPC, as the Shop's own reader reaches it, each within EVIDENCE_READ_MS. */
export function createManaEvidence(opts: { url?: (chainId: ChainId) => string | null; fetchImpl?: typeof fetch; reader: ChainReader; readMs?: number }): ManaEvidence {
  const urlFor = opts.url ?? rpcUrl
  const fetchImpl = opts.fetchImpl ?? ((...a: Parameters<typeof fetch>) => fetch(...a))
  const readMs = opts.readMs ?? EVIDENCE_READ_MS
  let id = 1
  const rpc = async <T>(chainId: number, method: string, params: unknown[]): Promise<T> => {
    const url = urlFor(chainId as ChainId)
    if (!url) throw new Error(`No RPC is configured for chain ${chainId}`)
    const res = await fetchImpl(url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: id++, method, params }),
      signal: AbortSignal.timeout(readMs)
    })
    if (!res.ok) {
      // An error's body is let go, never left open: the watch reads again and again.
      void res.body?.cancel().catch(() => undefined)
      throw new Error(`RPC ${method} answered HTTP ${res.status}`)
    }
    const body = (await res.json()) as { result?: T; error?: { message?: string } }
    if (body.error) throw new Error(body.error.message ?? `RPC ${method} failed`)
    return body.result as T
  }
  const hex = (n: bigint): string => `0x${n.toString(16)}`
  return {
    finalized: async (chainId) => BigInt((await rpc<{ number: Hex }>(chainId, 'eth_getBlockByNumber', ['finalized', false])).number),
    finalizedTime: async (chainId) => Number(BigInt((await rpc<{ timestamp: Hex }>(chainId, 'eth_getBlockByNumber', ['finalized', false])).timestamp)),
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
      const tx = await rpc<{ from?: string | null; to: string | null; input: Hex } | null>(chainId, 'eth_getTransactionByHash', [hash])
      return tx ? { to: tx.to, input: tx.input, from: tx.from ?? null } : null
    },
    receipt: (chainId, hash) => deadline(opts.reader.receipt(chainId as ChainId, hash, AbortSignal.timeout(readMs)), readMs)
  }
}

type ManaResolution =
  | { kind: 'succeeded'; txHash: Hex }
  | { kind: 'failed' }
  /**
   * `retry`: the finalized block reads its nonce exactly (unused, and not a lagging read below it) and none of its sends
   * is on its way, so the same sale may be sent again at that nonce (once enough time passed and while the record has
   * room for its evidence); never while a match waits to be attributed.
   */
  | { kind: 'waiting'; retry: boolean }

/** Chain order: by block, then by place in the block. */
const inChainOrder = (a: ManaExecution, b: ManaExecution): number => (a.blockNumber === b.blockNumber ? a.logIndex - b.logIndex : a.blockNumber < b.blockNumber ? -1 : 1)

/**
 * How the attempt's sale ended, from the chain, or that it hasn't yet. `venue` is the registry's entry for the sale's
 * venue (its meta-transaction style decides how the nonce reads).
 *
 * At a verified venue each successful execution consumes its signer's nonce once and logs its call, and nothing else
 * moves that nonce. So the buyer's executions over a range, in chain order, are their nonces in order, and the one at N
 * is the execution that consumed N, whatever carried it to the venue (the relay itself, or a forwarder its upstream
 * uses). A read holds all of them only when it counts exactly as many as the nonce moved across the range; a read that
 * counts any other number decides nothing.
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
  const nonce = BigInt(attempt.nonce)
  const call = keccak256(attempt.data)
  const verified = verifiedVenue(chainId, venue)
  const fromBlock = BigInt(attempt.fromBlock)
  const finalized = await evidence.finalized(chainId)
  /** The first block in (`after`, `to`] whose nonce reads past N, given that `after`'s doesn't and `to`'s does: by halving. */
  const consumingBlock = async (after: bigint, to: bigint): Promise<bigint> => {
    // Kept: nonceAt(lo − 1) <= N < nonceAt(hi).
    let lo = after + 1n
    let hi = to
    while (lo < hi) {
      const mid = (lo + hi) / 2n
      if ((await evidence.nonceAt(chainId, ctx.venue, buyer, mid)) > nonce) hi = mid
      else lo = mid + 1n
    }
    return hi
  }
  /** The execution that consumed N, from a whole read of [from, to]; null when the read isn't whole or N isn't in it. */
  const consumedN = async (from: bigint, to: bigint): Promise<ManaExecution | null> => {
    const start = await evidence.nonceAt(chainId, ctx.venue, buyer, from - 1n)
    const end = await evidence.nonceAt(chainId, ctx.venue, buyer, to)
    if (nonce < start || nonce >= end) return null
    // One entry per log, however the node repeats it: a repeat never stands in for a log it left out.
    const logs = new Map((await evidence.executions(chainId, venue, from, to)).filter((e) => e.user === buyer).map((e) => [`${e.blockNumber}:${e.logIndex}`, e]))
    const mine = [...logs.values()].sort(inChainOrder)
    if (BigInt(mine.length) !== end - start) return null
    return mine[Number(nonce - start)] ?? null
  }
  /**
   * Whether `hash` is this sale, finalized and successful, by its bytes alone: a transaction past the checkpoint whose
   * input is exactly the signed execution this page sent (its signature names N). The same bytes at or before the
   * checkpoint are never a send of this sale (a mint signed identically at a stale nonce, say): the nonce decides those.
   * Any other carrier (a forwarder's) is found below, by the execution that consumed N.
   */
  const ours = async (hash: string): Promise<boolean> => {
    const receipt = await evidence.receipt(chainId, hash)
    if (!receipt || receipt.status !== 'success' || receipt.blockNumber <= fromBlock || receipt.blockNumber > finalized) return false
    const tx = await evidence.transaction(chainId, hash)
    return !!tx && tx.to?.toLowerCase() === venue && payloads.has(keccak256(tx.input))
  }
  // The relay's own answers first: a hash it gave for one of these sends.
  for (const send of attempt.sends) {
    if (send.hash && (await ours(send.hash))) return { kind: 'succeeded', txHash: send.hash as Hex }
  }
  // A finalized block below the checkpoint (a lagging node) says nothing yet, neither about N nor about a retry.
  if (finalized < fromBlock) return { kind: 'waiting', retry: false }
  const read = await evidence.nonceAt(chainId, ctx.venue, buyer, finalized)
  if (read === nonce) return { kind: 'waiting', retry: idle }
  if (read < nonce) return { kind: 'waiting', retry: false }
  // N was consumed by F. At a verified venue, the execution that consumed it decides: this call is the sale, bought;
  // any other means none of these sends can ever run.
  if (verified) {
    // Consumed by the checkpoint, before any send of the sale left: never by this sale, so it can never run.
    if ((await evidence.nonceAt(chainId, ctx.venue, buyer, fromBlock)) > nonce) return { kind: 'failed' }
    const block = await consumingBlock(fromBlock, finalized)
    const at = await consumedN(block, block)
    if (!at) return { kind: 'waiting', retry: false }
    return keccak256(at.call) === call ? { kind: 'succeeded', txHash: at.txHash } : { kind: 'failed' }
  }
  // Elsewhere only a signed send of ours ends it, as bought.
  const executions = await evidence.executions(chainId, venue, fromBlock, finalized)
  for (const match of executions.filter((e) => e.user === buyer && keccak256(e.call) === call)) {
    if (await ours(match.txHash)) return { kind: 'succeeded', txHash: match.txHash }
  }
  return { kind: 'waiting', retry: false }
}

/**
 * Whether the attempt's sale is on chain past its checkpoint, final or not yet: a hash the relay gave for one of its
 * sends is a successful transaction past the checkpoint, and either it is this sale's own signed execution (its bytes,
 * sent to the venue), or, at a verified venue, its block holds the execution that consumed the sale's nonce with the
 * sale's call (a forwarder's carriage). A receipt alone is never enough: the relay's hash may name some other
 * transaction. This never settles the sale (only resolveMana's finalized evidence does). It says only that what comes
 * after the sale in a checkout may go, since its nonce is spent by the sale itself. Anything it can't read is a no.
 */
export async function manaIncluded(
  attempt: ManaAttemptRecord,
  ctx: { chainId: number; buyer: string; venue: { address: string; metaTx: MetaTxStyle }; evidence: ManaEvidence }
): Promise<boolean> {
  const payloads = new Set(attempt.sends.flatMap((s) => (s.stage === 'dispatched' && s.payload ? [s.payload] : [])))
  if (attempt.nonce == null || attempt.data == null || attempt.fromBlock == null || payloads.size === 0) return false
  const { chainId, evidence } = ctx
  const buyer = ctx.buyer.toLowerCase()
  const venue = ctx.venue.address.toLowerCase()
  const nonce = BigInt(attempt.nonce)
  const call = keccak256(attempt.data)
  const fromBlock = BigInt(attempt.fromBlock)
  for (const send of attempt.sends) {
    if (!send.hash) continue
    const receipt = await evidence.receipt(chainId, send.hash)
    if (!receipt || receipt.status !== 'success' || receipt.blockNumber <= fromBlock) continue
    const tx = await evidence.transaction(chainId, send.hash)
    if (tx && tx.to?.toLowerCase() === venue && payloads.has(keccak256(tx.input))) return true
    if (!verifiedVenue(chainId, venue)) continue
    // At a verified venue each execution moves its signer's nonce once and logs its call: the block holds the one that
    // consumed N only when it counts exactly as many of the buyer's executions as the nonce moved across it.
    const block = receipt.blockNumber
    const start = await evidence.nonceAt(chainId, ctx.venue, buyer, block - 1n)
    const end = await evidence.nonceAt(chainId, ctx.venue, buyer, block)
    if (nonce < start || nonce >= end) continue
    const logs = new Map((await evidence.executions(chainId, venue, block, block)).filter((e) => e.user === buyer).map((e) => [`${e.blockNumber}:${e.logIndex}`, e]))
    const mine = [...logs.values()].sort(inChainOrder)
    if (BigInt(mine.length) !== end - start) continue
    const at = mine[Number(nonce - start)]
    if (at && keccak256(at.call) === call) return true
  }
  return false
}

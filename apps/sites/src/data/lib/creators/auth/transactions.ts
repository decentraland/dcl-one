// On-chain reads and writes, kept inside the sealed auth module (CONVENTIONS.md, seam 1) so the
// provider never leaks: consumers describe a call (contract + method + args) and get a tx hash back.
// Writes go straight through the wallet when it already sits on the target chain, and otherwise as
// a Polygon meta-transaction through Decentraland's relayer — the same path the legacy builder uses.
import {
  TransactionNotFoundError,
  TransactionReceiptNotFoundError,
  createPublicClient,
  http,
  parseUnits,
  type Address,
  type Hash,
  type Hex,
  type PublicClient
} from 'viem'
import { estimateGas, getTransaction } from 'viem/actions'
import { ProviderType } from '@dcl/schemas'
import { config } from '~/config'
import { encodeContractCall, type ContractCall, type ContractData } from '~/lib/chain/contracts'
import { sendMetaTransaction, type MetaTransactionWallet } from '~/lib/chain/metaTransaction'
import { WrongAccountError, WrongChainError } from '~/lib/walletErrors'
import { type Session } from './auth'
import { readWalletChainId, requireAccount, switchWalletChain } from './provider'
import { walletClientOf } from './walletClient'

export type { ContractCall, ContractData }

/** An EIP-712 domain; `verifyingContract` and `salt` are hex strings. */
export type TypedDataDomain = {
  name?: string
  version?: string
  chainId?: number | bigint
  verifyingContract?: string
  salt?: string
}

export type TypedDataTypes = Record<string, { name: string; type: string }[]>

const readClients = new Map<number, PublicClient>()

async function getReadClient(chainId: number): Promise<PublicClient> {
  const cached = readClients.get(chainId)
  if (cached) return cached
  const { getRpcUrls } = await import('decentraland-connect')
  const url = getRpcUrls(ProviderType.INJECTED)[chainId]
  if (!url) throw new Error(`No RPC url configured for chain ${chainId}`)
  const client = createPublicClient({ transport: http(url) })
  readClients.set(chainId, client)
  return client
}

/** A view call: integers wider than 48 bits come back as `bigint`, several outputs as an array. */
export async function readContract<T>(contract: ContractData, method: string, args: unknown[] = []): Promise<T> {
  const client = await getReadClient(contract.chainId)
  const result: unknown = await client.readContract({
    address: contract.address as Address,
    abi: contract.abi,
    functionName: method,
    args
  })
  return result as T
}

/** Sends the call and resolves with the transaction hash (not the receipt). */
export async function sendContractTransaction(session: Session, call: ContractCall): Promise<string> {
  const chainId = await readWalletChainId(session)
  if (chainId === Number(call.contract.chainId)) return sendFromWallet(session, chainId, call)
  const wallet = metaTransactionWallet(session)
  // The account checked here is the one that signs, as the wallet spelled it (the relayed calldata keeps its case).
  const [active = ''] = await wallet.accounts()
  if (active.toLowerCase() !== session.address.toLowerCase()) {
    throw new WrongAccountError(session.address, active.toLowerCase())
  }
  const target = await getReadClient(call.contract.chainId)
  const callContract = (tx: { data: string; to: string }) =>
    target.request({ method: 'eth_call', params: [tx as { data: Hex; to: Address }, 'latest'] })
  return sendMetaTransaction(
    wallet,
    active,
    callContract,
    encodeContractCall(call),
    call.contract,
    config.get('TRANSACTIONS_API_URL')
  )
}

// An empty answer counts as a failed request, and legacy providers wrap theirs in `result`.
function metaTransactionWallet(session: Session): MetaTransactionWallet & { accounts: () => Promise<string[]> } {
  const request = async (method: string, params: unknown[]) => {
    const answer = await session.provider.request({ method, params })
    if (!answer) throw new Error(`The wallet gave no answer to ${method}`)
    return (answer as { result?: unknown }).result || answer
  }
  return {
    // Some wallets only answer eth_accounts.
    accounts: async () =>
      (await request('eth_requestAccounts', []).catch(() => request('eth_accounts', []))) as string[],
    code: async address => (await request('eth_getCode', [address, 'latest'])) as string,
    signTypedData: async (address, typedData) => (await request('eth_signTypedData_v4', [address, typedData])) as string
  }
}

// Estimated before sending, so a call that would revert fails here instead of reaching the wallet prompt.
async function sendFromWallet(session: Session, chainId: number, call: ContractCall): Promise<Hash> {
  const wallet = walletClientOf(session.provider, session.address)
  const request = {
    to: call.contract.address as Address,
    data: encodeContractCall(call) as Hex,
    ...gasOverrides(chainId)
  }
  const gas = await estimateGas(wallet, { ...request, prepare: false })
  return wallet.sendTransaction({ ...request, gas, chain: null })
}

// Some networks (Amoy) enforce a minimum tip above what wallets propose, so the RPC rejects the transaction. The
// floor, when there is one, comes from the env config.
function gasOverrides(chainId: number): { maxPriorityFeePerGas?: bigint; maxFeePerGas?: bigint } {
  const minTip = config.get('MATIC_MIN_TIP_GWEI')
  if (!minTip || chainId !== Number(config.get('MATIC_CHAIN_ID'))) return {}
  const tip = parseUnits(minTip, 9)
  return { maxPriorityFeePerGas: tip, maxFeePerGas: tip + parseUnits('50', 9) }
}

/**
 * Sends the call from the wallet itself, moving it to the contract's chain first, for contracts that take no
 * meta-transactions: the wallet pays the gas. Call it from a click, since wallets refuse unprompted chain switches.
 * The network is read again after the switch: some providers (Magic) answer it without moving the wallet. It is
 * sent from the session's account only: a call keyed by its caller, like a cancellation, does nothing from another.
 * `onSent` gets the hash as soon as the wallet returns it, before the nonce is read, so a caller can record it first.
 */
export async function sendWalletTransaction(
  session: Session,
  call: ContractCall,
  onSent?: (hash: string) => void
): Promise<{ hash: string; nonce?: number }> {
  const chainId = Number(call.contract.chainId)
  await requireAccount(session)
  if ((await readWalletChainId(session)) !== chainId) {
    await switchWalletChain(session, chainId)
    const current = await readWalletChainId(session)
    if (current !== chainId) throw new WrongChainError(chainId, current)
    // Switching hands control to the wallet, where the account can change too.
    await requireAccount(session)
  }
  // Sent from the session's address, so the wallet refuses to send it from any other account.
  const hash = await sendFromWallet(session, chainId, call)
  onSent?.(hash)
  return { hash, nonce: await readSentNonce(session, chainId, hash) }
}

// The wallet knows the transaction it just sent before a read node may; undefined when neither returns it.
async function readSentNonce(session: Session, chainId: number, hash: Hash): Promise<number | undefined> {
  try {
    return (await getTransaction(walletClientOf(session.provider, session.address), { hash })).nonce
  } catch {
    return (await readTransactionSender(chainId, hash).catch(() => undefined))?.nonce
  }
}

/** EIP-712 signature of `value` by the connected wallet (an off-chain order, not a transaction). */
export async function signTypedData(
  session: Session,
  domain: TypedDataDomain,
  types: TypedDataTypes,
  primaryType: string,
  value: Record<string, unknown>
): Promise<string> {
  return walletClientOf(session.provider, session.address).signTypedData({
    domain,
    types,
    primaryType,
    message: value
  } as Parameters<ReturnType<typeof walletClientOf>['signTypedData']>[0])
}

/**
 * What the chain says about a transaction now: `mined` or `reverted` once it has a receipt; `superseded` when it has
 * none but its sender has already used its nonce, so it can never execute; `unknown` otherwise (still pending, or
 * the read failed).
 */
export async function readTransactionOutcome(
  chainId: number,
  txHash: string,
  from: string,
  nonce?: number
): Promise<'mined' | 'reverted' | 'superseded' | 'unknown'> {
  try {
    const client = await getReadClient(chainId)
    const hash = txHash as Hash
    const receipt = await receiptOf(client, hash)
    if (receipt) return receipt.status === 'success' ? 'mined' : 'reverted'
    const known = nonce ?? (await transactionOf(client, hash))?.nonce
    if (known === undefined) return 'unknown'
    const used = await client.getTransactionCount({ address: from as Address, blockTag: 'latest' })
    return used > known ? 'superseded' : 'unknown'
  } catch {
    return 'unknown'
  }
}

export type TransactionSettlement = 'mined' | 'reverted' | 'dropped' | 'pending'

type TransactionReader = Pick<PublicClient, 'getTransactionReceipt' | 'getTransaction' | 'getTransactionCount'>

// A transaction or receipt the node doesn't have is null here; viem throws for it.
function receiptOf(reader: Pick<TransactionReader, 'getTransactionReceipt'>, hash: Hash) {
  return reader.getTransactionReceipt({ hash }).catch((error: unknown) => {
    if (error instanceof TransactionReceiptNotFoundError) return null
    throw error
  })
}

function transactionOf(reader: Pick<TransactionReader, 'getTransaction'>, hash: Hash) {
  return reader.getTransaction({ hash }).catch((error: unknown) => {
    if (error instanceof TransactionNotFoundError) return null
    throw error
  })
}

/** Who sent a transaction and with which nonce: once the nonce is used, the transaction can never run. */
export type TransactionSender = { from: string; nonce: number }

type WatchOptions = {
  timeoutMs: number
  pollMs: number
  /** Known from when it was sent: nodes usually forget a replaced transaction, so its own lookup can't tell. */
  sender?: TransactionSender
  /** The sender and nonce, the first time the node returns the transaction: worth keeping for a later watch. */
  onSeen?: (sender: TransactionSender) => void
  sleep?: (ms: number) => Promise<void>
  now?: () => number
}

const defaultSleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))

/**
 * Polls a transaction until it settles or `timeoutMs` passes: `dropped` only once its nonce was used by another
 * transaction (a wallet speed-up or cancel), read from the transaction or the `sender` recorded when it was sent;
 * `pending` when it is still waiting by then, or nothing tells, since the read node may simply not have it.
 */
export async function watchTransaction(
  reader: TransactionReader,
  txHash: string,
  { timeoutMs, pollMs, sender, onSeen, sleep = defaultSleep, now = Date.now }: WatchOptions
): Promise<TransactionSettlement> {
  const deadline = now() + timeoutMs
  const hash = txHash as Hash
  const fromReceipt = (receipt: { status: 'success' | 'reverted' }) =>
    receipt.status === 'success' ? 'mined' : 'reverted'
  let seen = false
  for (;;) {
    try {
      const receipt = await receiptOf(reader, hash)
      if (receipt) return fromReceipt(receipt)
      const tx = await transactionOf(reader, hash)
      if (tx && !sender && !seen) {
        seen = true
        onSeen?.({ from: tx.from, nonce: tx.nonce })
      }
      const known = tx ? { from: tx.from, nonce: tx.nonce } : sender
      if (known) {
        const used = await reader.getTransactionCount({ address: known.from as Address, blockTag: 'latest' })
        if (used > known.nonce) {
          // It may have been mined between the two reads.
          const late = await receiptOf(reader, hash)
          return late ? fromReceipt(late) : 'dropped'
        }
      }
    } catch {
      // A failed read says nothing about the transaction.
    }
    if (now() >= deadline) return 'pending'
    await sleep(pollMs)
  }
}

const SETTLE_TIMEOUT_MS = 5 * 60_000
const SETTLE_POLL_MS = 4_000

export async function settleTransaction(
  chainId: number,
  txHash: string,
  sender?: TransactionSender,
  onSeen?: (sender: TransactionSender) => void
): Promise<TransactionSettlement> {
  return watchTransaction(await getReadClient(chainId), txHash, {
    timeoutMs: SETTLE_TIMEOUT_MS,
    pollMs: SETTLE_POLL_MS,
    sender,
    onSeen
  })
}

const SENDER_READS = 5
const SENDER_READ_DELAY_MS = 2_000

/** The sender and nonce of a transaction just sent, once the read node has it; undefined if it never shows up. */
export async function readTransactionSender(chainId: number, txHash: string): Promise<TransactionSender | undefined> {
  const client = await getReadClient(chainId)
  for (let read = 0; read < SENDER_READS; read++) {
    if (read > 0) await defaultSleep(SENDER_READ_DELAY_MS)
    const tx = await transactionOf(client, txHash as Hash).catch(() => null)
    if (tx) return { from: tx.from, nonce: tx.nonce }
  }
  return undefined
}

const RECEIPT_POLL_MS = 4_000

type ReceiptWaitOptions = {
  timeoutMs?: number
  pollMs?: number
  sleep?: (ms: number) => Promise<void>
  now?: () => number
}

/**
 * Polls for this hash's receipt: true once mined successfully, false once reverted. A failed read (network, 5xx,
 * rate limit) is retried, never thrown; only `timeoutMs`, when given, ends the wait without an outcome, as an error.
 * A replacement's receipt never stands in for this transaction's.
 */
export async function waitForReceipt(
  reader: Pick<TransactionReader, 'getTransactionReceipt'>,
  txHash: string,
  { timeoutMs, pollMs = RECEIPT_POLL_MS, sleep = defaultSleep, now = Date.now }: ReceiptWaitOptions = {}
): Promise<boolean> {
  const deadline = timeoutMs === undefined ? Infinity : now() + timeoutMs
  for (;;) {
    const receipt = await receiptOf(reader, txHash as Hash).catch(() => null)
    if (receipt) return receipt.status === 'success'
    if (now() >= deadline) throw new Error(`Transaction ${txHash} was not mined within ${timeoutMs}ms`)
    await sleep(pollMs)
  }
}

/** Whether a contract is deployed at `address`. */
export async function isContractDeployed(chainId: number, address: string): Promise<boolean> {
  const code = await (await getReadClient(chainId)).getCode({ address: address as Address })
  return !!code && code !== '0x'
}

/** Resolves true when the transaction is mined successfully, false when it reverted; see `waitForReceipt`. */
export async function waitForTransaction(chainId: number, txHash: string, timeoutMs?: number): Promise<boolean> {
  return waitForReceipt(await getReadClient(chainId), txHash, { timeoutMs })
}

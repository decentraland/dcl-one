// Gasless calls through Decentraland's transactions relayer, ported from decentraland-transactions@3.3.0
// (Apache-2.0) so the wallet requests, the signed typed data and the relayed calldata stay what the relayer
// and the contracts expect. The wallet itself stays behind lib/auth: it hands in the three requests needed.
import { type ContractData } from './contracts'

export const MetaTransactionErrorCode = {
  UNKNOWN: 'unknown',
  INVALID_ADDRESS: 'invalid_address',
  CONTRACT_ACCOUNT: 'contract_account',
  USER_DENIED: 'user_denied'
} as const

// `cause` keeps the wallet's own error (its 4001 code) reachable by isWalletRejection and the Sentry tags.
export class MetaTransactionError extends Error {
  cause?: unknown

  constructor(
    message: string,
    public code: string,
    cause?: unknown
  ) {
    super(message)
    this.name = 'MetaTransactionError'
    if (cause !== undefined) this.cause = cause
  }
}

/** The wallet requests a meta-transaction makes; lib/auth answers them over the session's provider. */
export type MetaTransactionWallet = {
  /** `eth_getCode` at the latest block. */
  code: (address: string) => Promise<string>
  /** `eth_signTypedData_v4` of the JSON-serialized typed data. */
  signTypedData: (address: string, typedData: string) => Promise<string>
}

/** An `eth_call` on the contract's own chain, which may not be the wallet's. */
export type ContractCaller = (call: { data: string; to: string }) => Promise<string>

const GET_NONCE_SELECTOR = '2d0335ab'
const EXECUTE_META_TRANSACTION_SELECTOR = '0c53c51c'
const EXECUTE_META_TRANSACTION_FUNCTION_DATA_SELECTOR = 'd8ed1acc'
// EIP-7702 delegated accounts carry code (0xef01 + target) yet still sign with their own key.
const DELEGATION_PREFIX = '0xef01'

const DOMAIN_TYPE = [
  { name: 'name', type: 'string' },
  { name: 'version', type: 'string' },
  { name: 'verifyingContract', type: 'address' },
  { name: 'salt', type: 'bytes32' }
]

const to32Bytes = (value: string) => value.replace('0x', '').padStart(64, '0')

// Ledger answers a recovery id of 0 or 1 where the contracts expect 27 or 28.
function normalizeV(v: string): string {
  let parsed = parseInt(v, 16)
  if (parsed < 27) parsed += 27
  if (parsed !== 27 && parsed !== 28) throw new Error(`Invalid signature version "${v}" (parsed: ${parsed})`)
  return parsed.toString(16)
}

function isContractCode(code: string): boolean {
  if (code.toLowerCase().startsWith(DELEGATION_PREFIX)) return false
  if (!/^0x[0-9a-f]*$/i.test(code)) throw new Error(`Not a valid hex string "${code}"`)
  return !/^0x0{0,40}$/.test(code)
}

// executeMetaTransaction(address,bytes,bytes32,bytes32,uint8), assembled by hand as the package does so the
// relayed calldata stays the same string, account casing included.
function functionSignatureCall(account: string, signature: string, calldata: string): string {
  const sig = signature.replace('0x', '')
  const method = calldata.replace('0x', '')
  return [
    '0x',
    EXECUTE_META_TRANSACTION_SELECTOR,
    to32Bytes(account),
    to32Bytes('a0'),
    sig.substring(0, 64),
    sig.substring(64, 128),
    to32Bytes(normalizeV(sig.substring(128, 130))),
    to32Bytes((method.length / 2).toString(16)),
    method.padEnd(64 * Math.ceil(method.length / 64), '0')
  ].join('')
}

// executeMetaTransaction(address,bytes,bytes): the signature travels as one blob, and only a 65-byte one gets
// its recovery id normalized (a contract signature is opaque).
function functionDataCall(account: string, signature: string, calldata: string): string {
  const data = calldata.replace('0x', '')
  let sig = signature.replace('0x', '')
  if (sig.length === 130) sig = sig.slice(0, 128) + normalizeV(sig.slice(128)).padStart(2, '0')
  return [
    '0x',
    EXECUTE_META_TRANSACTION_FUNCTION_DATA_SELECTOR,
    to32Bytes(account),
    to32Bytes('60'),
    to32Bytes((96 + 32 + data.length / 2).toString(16)),
    to32Bytes((data.length / 2).toString(16)),
    data,
    to32Bytes((sig.length / 2).toString(16)),
    sig.padEnd(64 * Math.ceil(sig.length / 64), '0')
  ].join('')
}

/**
 * Has `account` sign `calldata` as a meta-transaction on `contract` and hands it to the relayer at `serverURL`,
 * which pays the gas. Resolves with the relayed transaction's hash; every failure is a `MetaTransactionError`.
 */
export async function sendMetaTransaction(
  wallet: MetaTransactionWallet,
  account: string,
  callContract: ContractCaller,
  calldata: string,
  contract: ContractData,
  serverURL: string
): Promise<string> {
  if (!contract.address.trim()) {
    throw new MetaTransactionError(
      `The contract address for ${contract.name} is empty`,
      MetaTransactionErrorCode.INVALID_ADDRESS
    )
  }
  try {
    if (!account) throw new Error('Could not find a valid connected account')
    if (isContractCode(await wallet.code(account.toLowerCase()))) {
      throw new MetaTransactionError('Contract accounts are not supported', MetaTransactionErrorCode.CONTRACT_ACCOUNT)
    }
    const functionData = contract.metaTransaction === 'functionData'
    const nonce = to32Bytes(
      await callContract({ data: `0x${GET_NONCE_SELECTOR}${to32Bytes(account)}`, to: contract.address })
    )
    const typedData = {
      types: {
        EIP712Domain: DOMAIN_TYPE,
        MetaTransaction: [
          { name: 'nonce', type: 'uint256' },
          { name: 'from', type: 'address' },
          { name: functionData ? 'functionData' : 'functionSignature', type: 'bytes' }
        ]
      },
      domain: {
        name: contract.name,
        version: contract.version,
        verifyingContract: contract.address,
        salt: `0x${to32Bytes(contract.chainId.toString(16))}`
      },
      primaryType: 'MetaTransaction',
      message: {
        nonce: parseInt(nonce, 16),
        from: account,
        ...(functionData ? { functionData: calldata } : { functionSignature: calldata })
      }
    }
    const signature = await wallet.signTypedData(account, JSON.stringify(typedData))
    const txData = (functionData ? functionDataCall : functionSignatureCall)(account, signature, calldata)
    const response = await fetch(`${serverURL}/transactions`, {
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ transactionData: { from: account, params: [contract.address, txData] } }),
      method: 'POST'
    })
    const body = (await response.json().catch(() => ({}))) as {
      ok?: boolean
      message?: string
      code?: string
      txHash?: string
    }
    if (body.ok === false || !response.ok) {
      if (body.message && body.code) throw new MetaTransactionError(body.message, body.code)
      throw new Error(`HTTP Error. Status: ${response.status}.`)
    }
    // Waiting on a missing hash would poll forever.
    if (typeof body.txHash !== 'string') throw new Error('The relayer answered without a transaction hash')
    return body.txHash
  } catch (error) {
    if (error instanceof MetaTransactionError) throw error
    // EIP-1193 wallets often reject with a plain `{ code, message }` object rather than an Error.
    const reason = (error as { message?: unknown } | null)?.message
    const message = typeof reason === 'string' ? reason : String(error)
    const denied = message.includes('User denied message signature')
    throw new MetaTransactionError(
      message,
      denied ? MetaTransactionErrorCode.USER_DENIED : MetaTransactionErrorCode.UNKNOWN,
      error
    )
  }
}

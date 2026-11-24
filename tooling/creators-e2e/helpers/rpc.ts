import {
  decodeAbiParameters,
  decodeFunctionData,
  encodeAbiParameters,
  isHex,
  keccak256,
  maxUint256,
  parseAbi,
  parseAbiParameters,
  parseEther,
  toFunctionSelector,
  toHex,
  zeroHash,
  type Hex,
  type TypedDataParameter,
  type TypedDataDomain
} from 'viem'
import { privateKeyToAccount } from 'viem/accounts'
import { RARITIES } from '../fixtures'
import { TEST_PRIVATE_KEY } from './session'

// One fake chain per run, behind both the app's read node and the mock wallet: a transaction the wallet sends
// is the one the read node returns a receipt for.

const encode = (types: string, values: readonly unknown[]) => encodeAbiParameters(parseAbiParameters(types), values)

/** Functions the app sends, decoded in `app.transactions` and by the fake builder-server. */
export const CONTRACT_FUNCTIONS = parseAbi([
  'function approve(address spender, uint256 amount)',
  'function createCollection(address forwarder, address factory, bytes32 salt, string name, string symbol, string baseURI, address creator, (string rarity, uint256 price, address beneficiary, string metadata)[] items)',
  'function setMinters(address[] minters, bool[] values)',
  'function setManagers(address[] managers, bool[] values)',
  'function issueTokens(address[] beneficiaries, uint256[] itemIds)',
  'function transferCreatorship(address creator)',
  'function editItemsData(uint256[] itemIds, uint256[] prices, address[] beneficiaries, string[] metadata)',
  'function manageCollection(address manager, address forwarder, address collection, bytes[] data)',
  'function setApproved(bool value)',
  'function rescueItems(uint256[] itemIds, string[] contentHashes, string[] metadata)',
  'function useCredits(((uint256 value, uint256 expiresAt, bytes32 salt)[] credits, bytes[] creditsSignatures, (address target, bytes4 selector, bytes data, uint256 expiresAt, bytes32 salt) externalCall, bytes customExternalCallSignature, uint256 maxUncreditedValue, uint256 maxCreditedValue) args)',
  'function reviewThirdPartyWithRoot(string thirdPartyId, bytes32 root, (uint256 qty, bytes32 salt, bytes32 sigR, bytes32 sigS, uint8 sigV)[] slots)'
])

/** Positional arguments that also answer by parameter name; integers are bigints, structs plain objects. */
export type DecodedArgs = readonly unknown[] & Record<string, unknown>

/** Selectors the app sends whose arguments no spec reads yet: named only. */
const NAMED_SELECTORS: Record<string, string> = {
  '0x6f773280': 'cancelSignature',
  '0xd41feedf': 'cancelSignature',
  '0xdb9aa95c': 'addThirdParties',
  '0xd8f0f3c1': 'buyItemSlots'
}

export type SentTransaction = {
  hash: string
  from: string
  to: string
  data: string
  /** The function name, when the selector is known. */
  fn?: string
  /** Decoded arguments, when the function is in CONTRACT_FUNCTIONS. */
  args?: DecodedArgs
  /** Sent through the transactions relayer as a meta-transaction rather than from the wallet. */
  relayed?: boolean
}

// The two executeMetaTransaction layouts lib/chain/metaTransaction encodes: r/s/v apart, or one signature blob.
const META_TRANSACTION = parseAbi([
  'function executeMetaTransaction(address userAddress, bytes functionSignature, bytes32 sigR, bytes32 sigS, uint8 sigV)',
  'function executeMetaTransaction(address userAddress, bytes functionData, bytes signature)'
])

export type ChainOptions = {
  /** MANA balance (wei) of every account. Default 0. */
  manaBalanceWei?: string
  /** MANA allowance (wei) every spender has. Default unlimited, so no approve is needed. */
  manaAllowanceWei?: string
  /** Functions whose first transaction reverts (receipt status 0, never applied); later ones succeed. */
  revertOnce?: string[]
}

export const MANA_USD_AGGREGATOR = '0x' + 'ae'.repeat(20)
// ~0.27 USD per MANA at 8 decimals.
const MANA_USD_RATE = 26960836

type RpcCall = { id?: number | string; method: string; params?: unknown[] }

export function decodeTransaction(tx: Omit<SentTransaction, 'fn' | 'args'>): SentTransaction {
  const data = tx.data ?? '0x'
  try {
    const { functionName, args = [] } = decodeFunctionData({ abi: CONTRACT_FUNCTIONS, data: data as Hex })
    const { inputs } = CONTRACT_FUNCTIONS.find(fn => fn.name === functionName)!
    const named = Object.fromEntries(inputs.map((input, index) => [input.name, args[index]]))
    return { ...tx, fn: functionName, args: Object.assign([...args], named) }
  } catch {
    return { ...tx, fn: NAMED_SELECTORS[data.slice(0, 10)] }
  }
}

export function createChain(opts: ChainOptions = {}, signer = privateKeyToAccount(TEST_PRIVATE_KEY)) {
  const transactions: SentTransaction[] = []
  const listeners: Array<(tx: SentTransaction) => void> = []
  const reverted = new Set<string>()
  const pendingReverts = new Set(opts.revertOnce ?? [])

  const reads: Record<string, (data: string, to: string) => string> = {
    [toFunctionSelector('balanceOf(address)')]: () => encode('uint256', [BigInt(opts.manaBalanceWei ?? 0)]),
    [toFunctionSelector('allowance(address,address)')]: () =>
      encode('uint256', [opts.manaAllowanceWei === undefined ? maxUint256 : BigInt(opts.manaAllowanceWei)]),
    [toFunctionSelector('isApprovedForAll(address,address)')]: () => encode('bool', [true]),
    [toFunctionSelector('decimals()')]: (_, to) => encode('uint8', [to.toLowerCase() === MANA_USD_AGGREGATOR ? 8 : 18]),
    [toFunctionSelector('manaUsdAggregator()')]: () => encode('address', [MANA_USD_AGGREGATOR]),
    [toFunctionSelector('latestRoundData()')]: () => {
      const now = BigInt(Math.floor(Date.now() / 1000))
      return encode('uint80, int256, uint256, uint256, uint80', [1n, BigInt(MANA_USD_RATE), now, now, 1n])
    },
    [toFunctionSelector('getRarityByName(string)')]: data => {
      const [name] = decodeAbiParameters(parseAbiParameters('string'), `0x${data.slice(10)}`)
      const rarity = RARITIES.find(r => r.name === name) ?? RARITIES[0]
      return encode('(string, uint256, uint256)', [[rarity.name, BigInt(rarity.maxSupply), BigInt(rarity.prices.MANA)]])
    },
    [toFunctionSelector('contractSignatureIndex()')]: () => encode('uint256', [0n]),
    // ThirdPartyRegistry: an approved third party already on chain, and its slot price in USD.
    [toFunctionSelector('thirdParties(string)')]: () =>
      encode('bool, bytes32, uint256, uint256, uint256, string, string', [true, zeroHash, 100n, 0n, 1n, '', '']),
    [toFunctionSelector('itemSlotPrice()')]: () => encode('uint256', [parseEther('100')]),
    [toFunctionSelector('programmaticBasePurchasedSlots()')]: () => encode('uint256', [0n]),
    // ChainlinkOracle: USD per MANA at 18 decimals.
    [toFunctionSelector('getRate()')]: () => encode('uint256', [parseEther('0.27')]),
    [toFunctionSelector('signerSignatureIndex(address)')]: () => encode('uint256', [0n]),
    // Every contract's meta-transaction nonce for the signer.
    [toFunctionSelector('getNonce(address)')]: () => encode('uint256', [0n])
  }

  const transactionResult = (hash: string) => {
    const tx = transactions.find(t => t.hash === hash)
    return {
      hash,
      from: tx?.from ?? signer.address.toLowerCase(),
      to: tx?.to ?? null,
      input: tx?.data ?? '0x',
      blockNumber: '0x1',
      blockHash: '0x' + '00'.repeat(32),
      transactionIndex: '0x0',
      nonce: '0x0',
      value: '0x0',
      gas: '0x5208',
      gasPrice: '0x1',
      chainId: '0x13882',
      type: '0x0',
      v: '0x1b',
      r: '0x' + '01'.repeat(32),
      s: '0x' + '01'.repeat(32)
    }
  }

  // Every hash settles: a block at height 1 with the current head at 1 is one confirmation.
  const receipt = (hash: string) => {
    const tx = transactions.find(t => t.hash === hash)
    return {
      status: reverted.has(hash) ? '0x0' : '0x1',
      transactionHash: hash,
      transactionIndex: '0x0',
      blockNumber: '0x1',
      blockHash: '0x' + '00'.repeat(32),
      from: tx?.from ?? signer.address.toLowerCase(),
      to: tx?.to ?? null,
      contractAddress: null,
      gasUsed: '0x5208',
      cumulativeGasUsed: '0x5208',
      effectiveGasPrice: '0x1',
      logs: [],
      logsBloom: '0x' + '00'.repeat(256),
      type: '0x2'
    }
  }

  function answer(call: RpcCall): unknown {
    const params = call.params ?? []
    switch (call.method) {
      case 'eth_chainId':
        return '0x13882'
      case 'net_version':
        return '80002'
      case 'eth_blockNumber':
        return '0x1'
      case 'eth_gasPrice':
      case 'eth_maxPriorityFeePerGas':
        return '0x3b9aca00'
      case 'eth_estimateGas':
        return '0x5208'
      case 'eth_getCode':
        return '0x'
      case 'eth_getTransactionCount':
        return '0x0'
      case 'eth_getBlockByNumber':
        return { number: '0x1', timestamp: '0x0', baseFeePerGas: '0x1', hash: '0x' + '00'.repeat(32) }
      case 'eth_getTransactionReceipt':
        return receipt(params[0] as string)
      case 'eth_getTransactionByHash':
        return transactionResult(params[0] as string)
      case 'eth_call': {
        const { data = '0x', to = '' } = (params[0] ?? {}) as { data?: string; to?: string }
        return reads[data.slice(0, 10)]?.(data, to) ?? '0x'
      }
      default:
        return null
    }
  }

  /** The read node's JSON-RPC endpoint, batched or single. */
  function handleRpc(body: string): unknown {
    const parsed = JSON.parse(body) as RpcCall | RpcCall[]
    const one = (call: RpcCall) => ({ jsonrpc: '2.0', id: call.id, result: answer(call) })
    return Array.isArray(parsed) ? parsed.map(one) : one(parsed)
  }

  /** The mock wallet's EIP-1193 `request`, run in node so signatures are real and transactions are logged. */
  async function walletRequest(method: string, params: unknown[] = []): Promise<unknown> {
    switch (method) {
      case 'eth_requestAccounts':
      case 'eth_accounts':
        return [signer.address.toLowerCase()]
      case 'eth_sendTransaction': {
        const { from = signer.address, to = '', data = '0x' } = (params[0] ?? {}) as Partial<SentTransaction>
        return record({ from, to, data })
      }
      case 'personal_sign': {
        const message = params[0] as string
        return signer.signMessage({ message: isHex(message) ? { raw: message } : message })
      }
      case 'eth_sign':
        return signer.signMessage({ message: { raw: params[1] as Hex } })
      case 'eth_signTypedData':
      case 'eth_signTypedData_v4': {
        const typed = (typeof params[1] === 'string' ? JSON.parse(params[1]) : params[1]) as {
          domain: TypedDataDomain
          types: Record<string, readonly TypedDataParameter[]>
          primaryType: string
          message: Record<string, unknown>
        }
        // Signed over the fields the domain sets, ignoring any EIP712Domain in `types`: viem's own derivation
        // would drop a chainId sent as a string.
        const types: Record<string, readonly TypedDataParameter[]> = {
          ...typed.types,
          EIP712Domain: domainType(typed.domain)
        }
        return signer.signTypedData({
          domain: typed.domain,
          types,
          primaryType: typed.primaryType,
          message: typed.message
        })
      }
      case 'wallet_switchEthereumChain':
      case 'wallet_addEthereumChain':
        return null
      default:
        return answer({ method, params })
    }
  }

  function record({ from, to, data, relayed }: Pick<SentTransaction, 'from' | 'to' | 'data' | 'relayed'>): string {
    const hash = keccak256(toHex(`e2e-tx-${transactions.length}-${data}`))
    const tx = { ...decodeTransaction({ hash, from: from.toLowerCase(), to: to.toLowerCase(), data }), relayed }
    transactions.push(tx)
    if (tx.fn && pendingReverts.delete(tx.fn)) reverted.add(hash)
    else for (const listener of listeners) listener(tx)
    return hash
  }

  /**
   * The transactions relayer: unwraps the meta-transaction and runs its inner call as the signer. It doesn't verify
   * the signature (signer, domain or nonce), so a relayed spec covers the routing, not the signing.
   */
  function relay(body: string): { ok: true; txHash: string } {
    const { transactionData } = JSON.parse(body) as { transactionData: { from: string; params: [string, string] } }
    const [contract, metaData] = transactionData.params
    const { args } = decodeFunctionData({ abi: META_TRANSACTION, data: metaData as Hex })
    return { ok: true, txHash: record({ from: args[0], to: contract, data: args[1], relayed: true }) }
  }

  return {
    handleRpc,
    walletRequest,
    relay,
    transactions,
    onTransaction: (listener: (tx: SentTransaction) => void) => listeners.push(listener)
  }
}

export type Chain = ReturnType<typeof createChain>

const DOMAIN_FIELDS = [
  { name: 'name', type: 'string' },
  { name: 'version', type: 'string' },
  { name: 'chainId', type: 'uint256' },
  { name: 'verifyingContract', type: 'address' },
  { name: 'salt', type: 'bytes32' }
] as const

function domainType(domain: TypedDataDomain) {
  return DOMAIN_FIELDS.filter(field => domain[field.name] != null)
}

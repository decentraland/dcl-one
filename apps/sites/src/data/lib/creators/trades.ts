// Off-chain marketplace orders: the EIP-712 encoding a creator signs and the
// marketplace-server endpoint that stores the signed order. Ported from the legacy builder's
// lib/trades + TradeService; the shop reads these orders back through its unified catalog.
import {
  TradeAssetType,
  type OnChainTrade,
  type Trade,
  type TradeAsset,
  type TradeChecks,
  type TradeCreation
} from '@dcl/schemas'
import { ContractName, getContract, getMarketplaceName } from '~/lib/chain/contracts'
import { numberToHex, zeroAddress } from 'viem'
import { toBytes32 } from '~/lib/bytes32'
import { config } from '~/config'
import { readContract, signedFetch, type ContractData, type TypedDataDomain, type TypedDataTypes } from '~/lib/auth'

export type UnsignedTrade = Omit<TradeCreation, 'signature'>

export const OFFCHAIN_MARKETPLACE_TYPES: TypedDataTypes = {
  Trade: [
    { name: 'checks', type: 'Checks' },
    { name: 'sent', type: 'AssetWithoutBeneficiary[]' },
    { name: 'received', type: 'Asset[]' }
  ],
  Asset: [
    { name: 'assetType', type: 'uint256' },
    { name: 'contractAddress', type: 'address' },
    { name: 'value', type: 'uint256' },
    { name: 'extra', type: 'bytes' },
    { name: 'beneficiary', type: 'address' }
  ],
  AssetWithoutBeneficiary: [
    { name: 'assetType', type: 'uint256' },
    { name: 'contractAddress', type: 'address' },
    { name: 'value', type: 'uint256' },
    { name: 'extra', type: 'bytes' }
  ],
  Checks: [
    { name: 'uses', type: 'uint256' },
    { name: 'expiration', type: 'uint256' },
    { name: 'effective', type: 'uint256' },
    { name: 'salt', type: 'bytes32' },
    { name: 'contractSignatureIndex', type: 'uint256' },
    { name: 'signerSignatureIndex', type: 'uint256' },
    { name: 'allowedRoot', type: 'bytes32' },
    { name: 'externalChecks', type: 'ExternalCheck[]' }
  ],
  ExternalCheck: [
    { name: 'contractAddress', type: 'address' },
    { name: 'selector', type: 'bytes4' },
    { name: 'value', type: 'bytes' },
    { name: 'required', type: 'bool' }
  ]
}

// Signed requests to marketplace-server must declare who signs and why.
const TRADE_AUTH_METADATA = { signer: 'dcl:builder', intent: 'dcl:create-trade' }

/**
 * The marketplace new orders are signed on, and the one that must be able to mint the collection.
 * marketplace-server only accepts new orders on it; older orders are still cancelled on their own contract.
 */
export function getOffchainMarketplaceContract(chainId: number) {
  return getContract(ContractName.OffChainMarketplaceV3, chainId)
}

type SignTypedData = (
  domain: TypedDataDomain,
  types: TypedDataTypes,
  primaryType: string,
  value: Record<string, unknown>
) => Promise<string>

/** Signs an order under the marketplace's domain and types with the given typed-data signer. */
export function tradeSigner(sign: SignTypedData, chainId: number) {
  return (trade: UnsignedTrade) =>
    sign(getTradeDomain(chainId), OFFCHAIN_MARKETPLACE_TYPES, 'Trade', toTradeTypedValues(trade))
}

/** The marketplace's EIP-712 domain: the chain id travels in `salt`, Decentraland's meta-tx convention. */
export function getTradeDomain(chainId: number): TypedDataDomain {
  const contract = getOffchainMarketplaceContract(chainId)
  return {
    name: contract.name,
    version: contract.version,
    salt: numberToHex(chainId, { size: 32 }),
    verifyingContract: contract.address
  }
}

function assetValue(asset: TradeAsset): string {
  switch (asset.assetType) {
    case TradeAssetType.ERC721:
      return asset.tokenId
    case TradeAssetType.COLLECTION_ITEM:
      return asset.itemId
    case TradeAssetType.ERC20:
    case TradeAssetType.USD_PEGGED_MANA:
      return asset.amount
  }
}

const toSeconds = (ms: number) => Math.floor(ms / 1000)
// Empty bytes are encoded as '0x'; the server-side JSON keeps ''.
const bytes = (value: string) => value || '0x'

/** `Checks` as the contracts sign and read them, for orders and coupons alike: timestamps in seconds, bytes32 zero-padded. */
export function toTypedChecks(checks: TradeChecks) {
  return {
    uses: checks.uses,
    expiration: toSeconds(checks.expiration),
    effective: toSeconds(checks.effective),
    salt: toBytes32(checks.salt),
    contractSignatureIndex: checks.contractSignatureIndex,
    signerSignatureIndex: checks.signerSignatureIndex,
    allowedRoot: toBytes32(bytes(checks.allowedRoot)),
    externalChecks: (checks.externalChecks ?? []).map(check => ({
      contractAddress: check.contractAddress,
      selector: check.selector,
      value: bytes(check.value),
      required: check.required
    }))
  }
}

/** The struct the wallet signs: timestamps in seconds, bytes32 fields zero-padded, assets flattened to `value`. */
export function toTradeTypedValues(trade: UnsignedTrade): Record<string, unknown> {
  return {
    checks: toTypedChecks(trade.checks),
    sent: trade.sent.map(asset => ({
      assetType: asset.assetType,
      contractAddress: asset.contractAddress,
      value: assetValue(asset),
      extra: bytes(asset.extra)
    })),
    received: trade.received.map(asset => ({
      assetType: asset.assetType,
      contractAddress: asset.contractAddress,
      value: assetValue(asset),
      extra: bytes(asset.extra),
      beneficiary: asset.beneficiary
    }))
  }
}

/** The stored order's marketplace names a contract this client does not know. */
export class UnknownTradeContractError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'UnknownTradeContractError'
  }
}

/**
 * The marketplace generation a stored trade belongs to (older listings sit on the V1 or V2 contract), always
 * resolved to the canonical address for that generation. The server's `contract` is only a lookup key:
 * a transaction is never sent to an address the client cannot vouch for, nor to another chain.
 */
export function getTradeContract(trade: Pick<Trade, 'contract' | 'chainId'>) {
  const expectedChainId = Number(config.get('MATIC_CHAIN_ID'))
  if (Number(trade.chainId) !== expectedChainId) {
    throw new UnknownTradeContractError(`Trade is on chain ${trade.chainId}, expected ${expectedChainId}`)
  }
  const name = getMarketplaceName(trade.contract)
  if (!name) throw new UnknownTradeContractError(`Trade contract ${trade.contract} is not a known marketplace`)
  const contract = getContract(name, trade.chainId)
  if (contract.address.toLowerCase() !== trade.contract.toLowerCase()) {
    throw new UnknownTradeContractError(`Trade contract ${trade.contract} does not match ${name} on this chain`)
  }
  return contract
}

/** A stored trade in the struct the marketplace contract takes for `cancelSignature` / `accept`. */
export function toOnChainTrade(trade: Trade): OnChainTrade {
  const values = toTradeTypedValues(trade) as Omit<OnChainTrade, 'signer' | 'signature'>
  return {
    signer: trade.signer,
    signature: trade.signature,
    ...values,
    checks: { ...values.checks, allowedProof: [] },
    // Nobody receives the sent asset when cancelling.
    sent: values.sent.map(asset => ({ ...asset, beneficiary: zeroAddress }))
  }
}

export type SignatureIndexes = { contractSignatureIndex: number; signerSignatureIndex: number }

/** The marketplace's current signature indexes; an order signed with stale ones is unredeemable. */
export function fetchSignatureIndexes(signer: string, chainId: number): Promise<SignatureIndexes> {
  return readSignatureIndexes(getOffchainMarketplaceContract(chainId), signer)
}

/** A signatures contract's current indexes for `signer` (a marketplace or a coupon manager). */
export async function readSignatureIndexes(contract: ContractData, signer: string): Promise<SignatureIndexes> {
  const [contractIndex, signerIndex] = await Promise.all([
    readContract<bigint>(contract, 'contractSignatureIndex'),
    readContract<bigint>(contract, 'signerSignatureIndex', [signer])
  ])
  return { contractSignatureIndex: Number(contractIndex), signerSignatureIndex: Number(signerIndex) }
}

/** The server retires an order (re-signing it as supply shrinks, or after it is cancelled), so a cached id can vanish. */
export class TradeNotFoundError extends Error {
  constructor(tradeId: string) {
    super(`Trade ${tradeId} not found`)
    this.name = 'TradeNotFoundError'
  }
}

/** A stored order: GET /v1/trades/:id. */
export async function fetchTrade(tradeId: string): Promise<Trade> {
  const response = await fetch(`${config.get('MARKETPLACE_SERVER_URL')}/v1/trades/${encodeURIComponent(tradeId)}`)
  if (response.status === 404) {
    await response.body?.cancel()
    throw new TradeNotFoundError(tradeId)
  }
  const body = (await response.json().catch(() => null)) as { ok?: boolean; data?: Trade } | null
  if (!response.ok || !body?.ok || !body.data)
    throw new Error(`marketplace-server trade ${tradeId} unavailable (${response.status})`)
  return body.data
}

/** marketplace-server still sees an open order for the item: the previous one isn't indexed as cancelled yet. */
export class TradeConflictError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'TradeConflictError'
  }
}

/** Stores a signed order: POST /v1/trades. Answers the stored trade id. */
export async function createTrade(address: string, trade: TradeCreation): Promise<string> {
  const response = await signedFetch(
    address,
    config.get('MARKETPLACE_SERVER_URL'),
    '/v1/trades',
    { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(trade) },
    TRADE_AUTH_METADATA
  )
  const body = (await response.json().catch(() => null)) as {
    ok?: boolean
    data?: { id?: string }
    message?: string
  } | null
  if (!response.ok || !body?.ok || !body.data?.id) {
    const message = body?.message ?? `marketplace-server rejected the order (${response.status})`
    throw response.status === 409 ? new TradeConflictError(message) : new Error(message)
  }
  return body.data.id
}

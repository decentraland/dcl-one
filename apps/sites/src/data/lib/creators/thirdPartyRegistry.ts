// Calls on the ThirdPartyRegistry (Polygon), ported from the legacy builder's thirdParty sagas and the TP approval
// flow: creators register their third party and buy item slots when publishing; curators set the Merkle root of the
// approved items (consuming the creator's cheque) and enable or disable a third party.
import { ContractName, getContract } from '~/lib/chain/contracts'
import { compactSignatureToSignature, parseCompactSignature, parseSignature, size, zeroHash, type Hex } from 'viem'
import { readContract, type ContractCall } from '~/lib/auth'
import { type SlotsCheque } from '~/lib/builder'
import { contractsFromRawMetadata, toRawMetadata, type LinkedContract, type ThirdParty } from '~/lib/thirdParty'

export function getThirdPartyRegistry(chainId: number) {
  return getContract(ContractName.ThirdPartyRegistry, chainId)
}

// What the registry stores as the third party's resolver; legacy always sends this placeholder.
const DISABLED_RESOLVER = 'Disabled'

/**
 * Registers a virtual third party on chain and buys its first `slots` (legacy `addThirdParties` on the first
 * publish). `maxPriceWei` caps what the registry may pull in MANA for the slots.
 */
export function buildAddThirdPartyCall(
  chainId: number,
  thirdParty: Pick<ThirdParty, 'id' | 'name' | 'description' | 'contracts' | 'managers' | 'isProgrammatic'>,
  slots: number,
  maxPriceWei: bigint
): ContractCall {
  return {
    contract: getThirdPartyRegistry(chainId),
    method: 'addThirdParties',
    args: [
      [
        [
          thirdParty.id,
          toRawMetadata(thirdParty),
          DISABLED_RESOLVER,
          thirdParty.managers,
          thirdParty.managers.map(() => true),
          String(slots)
        ]
      ],
      [thirdParty.isProgrammatic],
      [maxPriceWei.toString()]
    ]
  }
}

/**
 * Rewrites a registered third party's metadata (name, description, linked contracts). Empty resolver, managers and
 * slots leave those as they are: the registry skips every empty field.
 */
export function buildUpdateThirdPartyCall(
  chainId: number,
  thirdParty: Pick<ThirdParty, 'id' | 'name' | 'description' | 'contracts'>
): ContractCall {
  return {
    contract: getThirdPartyRegistry(chainId),
    method: 'updateThirdParties',
    args: [[[thirdParty.id, toRawMetadata(thirdParty), '', [], [], '0']]]
  }
}

/** Buys `qty` more item slots for a registered third party (legacy `buyItemSlots` on later publishes). */
export function buildBuyItemSlotsCall(
  chainId: number,
  thirdPartyId: string,
  qty: number,
  maxPriceWei: bigint
): ContractCall {
  return {
    contract: getThirdPartyRegistry(chainId),
    method: 'buyItemSlots',
    args: [thirdPartyId, String(qty), maxPriceWei.toString()]
  }
}

/** The cheque as the registry's `consumeSlots` tuple: (qty, salt, sigR, sigS, sigV). */
export type ChequeSlot = [string, string, string, string, number]

export function toChequeSlot(cheque: SlotsCheque): ChequeSlot {
  const signature = cheque.signature as Hex
  // A 64-byte EIP-2098 signature packs the recovery bit into `s`; the registry needs it split out.
  const { r, s, yParity } =
    size(signature) === 64 ? compactSignatureToSignature(parseCompactSignature(signature)) : parseSignature(signature)
  if (yParity === undefined) throw new Error('The cheque signature has no recovery id')
  // The registry takes the legacy 27/28 recovery id, whichever form the wallet answered.
  return [String(cheque.qty), cheque.salt, r, s, 27 + yParity]
}

/**
 * Sets the Merkle root of the third party's approved items, consuming the creator's cheque for the new slots
 * (an empty `slots` when the cheque was already consumed: a retry, or pushed changes that bought nothing).
 */
export function buildReviewThirdPartyWithRootCall(
  chainId: number,
  thirdPartyId: string,
  root: string,
  slots: ChequeSlot[]
): ContractCall {
  return {
    contract: getThirdPartyRegistry(chainId),
    method: 'reviewThirdPartyWithRoot',
    args: [thirdPartyId, root, slots]
  }
}

/** Enables (`true`) or disables (`false`) a whole third party, every collection under it included. */
export function buildReviewThirdPartyCall(chainId: number, thirdPartyId: string, value: boolean): ContractCall {
  return {
    contract: getThirdPartyRegistry(chainId),
    method: 'reviewThirdParties',
    args: [[[thirdPartyId, value, []]]]
  }
}

/** What the registry itself holds for a third party, ahead of whatever builder-server has indexed. */
export type RegisteredThirdParty = {
  registered: boolean
  maxItems: number
  /** The Merkle root of its approved items, empty when none was set. */
  root: string
}

const EMPTY_ROOT = zeroHash

type ThirdPartyRecord = { registered: bigint; maxItems: bigint; root: string; metadata: string }

type ThirdPartyOutputs = readonly [
  isApproved: boolean,
  root: string,
  maxItems: bigint,
  consumedSlots: bigint,
  registered: bigint,
  metadata: string,
  resolver: string
]

async function readThirdPartyRecord(chainId: number, thirdPartyId: string): Promise<ThirdPartyRecord> {
  const [, root, maxItems, , registered, metadata] = await readContract<ThirdPartyOutputs>(
    getThirdPartyRegistry(chainId),
    'thirdParties',
    [thirdPartyId]
  )
  return { registered, maxItems, root, metadata }
}

export async function fetchRegisteredThirdParty(chainId: number, thirdPartyId: string): Promise<RegisteredThirdParty> {
  const record = await readThirdPartyRecord(chainId, thirdPartyId)
  return {
    registered: record.registered !== 0n,
    maxItems: Number(record.maxItems),
    root: record.root === EMPTY_ROOT ? '' : record.root
  }
}

/** The contracts the registry itself lists for the third party right now, ahead of the subgraph. */
export async function fetchRegisteredContracts(chainId: number, thirdPartyId: string): Promise<LinkedContract[]> {
  const record = await readThirdPartyRecord(chainId, thirdPartyId)
  return contractsFromRawMetadata(record.metadata)
}

export type ThirdPartyPrice = {
  /** USD per item slot, in wei-like 18 decimals as the registry stores it. */
  itemSlotUsdWei: bigint
  /** MANA per item slot at the oracle's current rate. */
  itemSlotManaWei: bigint
  /** Slots a programmatic third party buys in one go, on its first publish. */
  programmaticMinSlots: number
  /** USD per MANA, 18 decimals: the ChainlinkOracle rate (the registry charges `price * 1e18 / rate`). */
  usdPerManaWei: bigint
}

/** The registry's slot price and the programmatic pack size, with the oracle's MANA rate (legacy getThirdPartyPrice). */
export async function fetchThirdPartyPrice(chainId: number): Promise<ThirdPartyPrice> {
  const registry = getThirdPartyRegistry(chainId)
  const oracle = getContract(ContractName.ChainlinkOracle, chainId)
  const [slotPrice, minSlots, rate] = await Promise.all([
    readContract<bigint>(registry, 'itemSlotPrice'),
    readContract<bigint>(registry, 'programmaticBasePurchasedSlots'),
    readContract<bigint>(oracle, 'getRate')
  ])
  return {
    itemSlotUsdWei: slotPrice,
    itemSlotManaWei: usdToMana(slotPrice, rate),
    programmaticMinSlots: Number(minSlots),
    usdPerManaWei: rate
  }
}

const ONE = 10n ** 18n

/** USD (18 decimals) to MANA wei at `usdPerManaWei`, the oracle's rate: MANA = USD / (USD per MANA). */
export function usdToMana(usdWei: bigint, usdPerManaWei: bigint): bigint {
  if (usdPerManaWei === 0n) return 0n
  return (usdWei * ONE) / usdPerManaWei
}

// The committee's approval flow for a standard collection, ported from the legacy builder's
// handleInitiateApprovalFlow: token ids → rescue content hashes on chain → deploy entities → approve.
// Every step is idempotent, so running the flow again resumes where a closed or failed run stopped.
import { type Entity } from '@dcl/schemas'
import { type ContractCall } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { RESCUE_CHUNK_SIZE, buildRescueItemsCalls, buildSetApprovedCall } from '~/lib/collectionApproval'
import {
  buildDeploymentForm,
  buildItemEntity,
  buildMissingImage,
  computeItemContentHash,
  getEntityContent,
  type AuthLink,
  type ImageDeps,
  type MissingImage
} from '~/lib/catalystEntity'
import { isItemSynced } from '~/lib/itemSync'
import { ItemType, getItemMetadata, type Item } from '~/lib/items'

export { buildMissingImage }

export type RescueTarget = { item: Item; contentHash: string }

export class ApprovalError extends Error {
  reason:
    | 'reverted'
    | 'not_indexed'
    | 'no_identity'
    | 'deploy_failed'
    | 'nothing_pending'
    | 'missing_hash'
    | 'invalid_cheque'
    | 'changed'
    | 'unsubmitted'
    | 'has_video'
  /** The items the error is about, when the curator must be told which. */
  items?: Item[]

  constructor(reason: ApprovalError['reason'], message: string, items?: Item[]) {
    super(message)
    this.name = 'ApprovalError'
    this.reason = reason
    this.items = items
  }
}

/**
 * Published items whose on-chain content hash no longer matches the builder copy. Emotes and items with
 * no server-computed hash are hashed here, in both formats, since older items may carry the Qm one.
 */
export async function findItemsToRescue(
  collection: Collection,
  items: Item[],
  imageOf: (item: Item) => Promise<MissingImage | null>
): Promise<RescueTarget[]> {
  const targets: RescueTarget[] = []
  for (const item of items) {
    if (!item.currentContentHash || item.type === ItemType.EMOTE) {
      const content = getEntityContent(item, (await imageOf(item))?.hash)
      const [v0, v1] = await Promise.all([
        computeItemContentHash(collection, item, content, 'v0'),
        computeItemContentHash(collection, item, content, 'v1')
      ])
      if (v0 !== item.blockchainContentHash && v1 !== item.blockchainContentHash) {
        targets.push({ item, contentHash: v1 })
      }
    } else if (item.currentContentHash !== item.blockchainContentHash) {
      targets.push({ item, contentHash: item.currentContentHash })
    }
  }
  return targets
}

export type RescueDeps = {
  chainId: number
  sendTransaction: (call: ContractCall) => Promise<string>
  waitForTransaction: (txHash: string) => Promise<boolean>
  fetchItems: () => Promise<Item[]>
  sleep?: (ms: number) => Promise<void>
  /** Called with each chunk's targets once its transaction is mined, so a retry can leave them out. */
  onChunkMined?: (targets: RescueTarget[]) => void
  /** Targets a previous attempt already mined: not sent again, but still waited on until indexed. */
  alreadyMined?: RescueTarget[]
}

const INDEXER_POLL_MS = 2000
export const INDEXER_TIMEOUT_MS = 10 * 60_000
/** How long the approval waits for builder-server to read the collection as approved before showing success anyway. */
export const APPROVAL_INDEX_TIMEOUT_MS = 2 * 60_000

const defaultSleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))

/** Rewrites the stale hashes on chain, one transaction per chunk, then waits until builder-server reads the new ones. */
export async function rescueItems(
  collection: Collection,
  targets: RescueTarget[],
  deps: RescueDeps,
  timeoutMs = INDEXER_TIMEOUT_MS
): Promise<Item[]> {
  const entries = targets.map(({ item, contentHash }) => ({
    tokenId: item.tokenId!,
    contentHash,
    metadata: getItemMetadata(item)
  }))
  const calls = buildRescueItemsCalls(deps.chainId, collection, entries)
  for (const [index, call] of calls.entries()) {
    const txHash = await deps.sendTransaction(call)
    if (!(await deps.waitForTransaction(txHash))) throw new ApprovalError('reverted', `Rescue ${txHash} reverted`)
    deps.onChunkMined?.(targets.slice(index * RESCUE_CHUNK_SIZE, (index + 1) * RESCUE_CHUNK_SIZE))
  }

  const expected = new Map(
    [...(deps.alreadyMined ?? []), ...targets].map(({ item, contentHash }) => [item.id, contentHash])
  )
  return waitForIndexer(
    deps.fetchItems,
    items => items.every(item => !expected.has(item.id) || item.blockchainContentHash === expected.get(item.id)),
    timeoutMs,
    deps.sleep
  )
}

/**
 * Polls `read` until builder-server reflects an on-chain change. A failed read counts as "not yet": a retry of
 * the step would resend (and pay for) its transactions.
 */
export async function waitForIndexer<T>(
  read: () => Promise<T>,
  isDone: (value: T) => boolean,
  timeoutMs: number,
  sleep = defaultSleep
): Promise<T> {
  for (let waited = 0; waited <= timeoutMs; waited += INDEXER_POLL_MS) {
    const value = await read().then(
      result => ({ result }),
      () => null
    )
    if (value && isDone(value.result)) return value.result
    await sleep(INDEXER_POLL_MS)
  }
  throw new ApprovalError('not_indexed', 'The change was not indexed in time')
}

/**
 * Published items whose Catalyst entity is missing or differs from the builder copy. `missingOnly` leaves out edited
 * items: the Catalyst rejects them until their new hash is approved on chain.
 */
export function findItemsToDeploy(items: Item[], entities: Entity[], missingOnly = false): Item[] {
  const byPointer = new Map<string, Entity>()
  for (const entity of entities) {
    for (const pointer of entity.pointers) byPointer.set(pointer.toLowerCase(), entity)
  }
  return items.filter(item => {
    if (!item.urn) return false
    const entity = byPointer.get(item.urn.toLowerCase())
    return !entity || (!missingOnly && !isItemSynced(item, entity))
  })
}

export type DeployDeps = ImageDeps & {
  sign: (entityId: string) => Promise<AuthLink[] | null>
  fetchAvailableContent: (hashes: string[]) => Promise<Set<string>>
  deployEntity: (form: FormData) => Promise<void>
}

async function deployItem(collection: Collection, item: Item, deps: DeployDeps): Promise<void> {
  const image = await buildMissingImage(item, deps)
  const content = getEntityContent(item, image?.hash)
  const entity = await buildItemEntity(collection, item, content)
  const authChain = await deps.sign(entity.entityId)
  if (!authChain) throw new ApprovalError('no_identity', 'The session has no signing identity')

  const hashes = [...new Set(Object.values(content))]
  const available = await deps.fetchAvailableContent(hashes)
  const files = new Map<string, Blob>()
  for (const hash of hashes) {
    if (available.has(hash)) continue
    files.set(hash, image && hash === image.hash ? image.blob : await deps.fetchContent(hash))
  }
  await deps.deployEntity(buildDeploymentForm(entity, authChain, files))
}

export type DeployFailure = { item: Item; message: string }
export type DeployResult = { deployed: Item[]; failed: DeployFailure[] }

/** Deploys every item's entity; one failure doesn't stop the rest, the caller retries the failed ones. */
export async function deployItems(
  collection: Collection,
  items: Item[],
  deps: DeployDeps,
  onProgress?: (done: number, total: number) => void
): Promise<DeployResult> {
  const result: DeployResult = { deployed: [], failed: [] }
  for (const item of items) {
    try {
      await deployItem(collection, item, deps)
      result.deployed.push(item)
    } catch (error) {
      if (error instanceof ApprovalError && error.reason === 'no_identity') throw error
      result.failed.push({ item, message: error instanceof Error ? error.message : String(error) })
    }
    onProgress?.(result.deployed.length + result.failed.length, items.length)
  }
  return result
}

export type ApproveDeps = {
  chainId: number
  sendTransaction: (call: ContractCall) => Promise<string>
  waitForTransaction: (txHash: string) => Promise<boolean>
}

/** Makes the collection mintable (also "Enable" for a disabled one). */
export async function approveOnChain(collection: Collection, deps: ApproveDeps): Promise<string> {
  const txHash = await deps.sendTransaction(buildSetApprovedCall(deps.chainId, collection, true))
  if (!(await deps.waitForTransaction(txHash))) throw new ApprovalError('reverted', `Approval ${txHash} reverted`)
  return txHash
}

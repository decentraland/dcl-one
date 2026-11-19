import { keccak256, toHex } from 'viem'
import {
  RARITIES,
  collection,
  wearable,
  type RemoteCollectionRow,
  type RemoteItemRow,
  type RemoteThirdPartyRow
} from '../fixtures'
import { decodeTransaction, type SentTransaction } from './rpc'

export type RemoteItemCurationRow = {
  id: string
  item_id: string
  status: 'pending' | 'approved' | 'rejected'
  content_hash: string | null
  is_mapping_complete: boolean
  created_at: string
  updated_at: string
}

export type BuilderSeed = {
  collections?: RemoteCollectionRow[]
  items?: RemoteItemRow[]
  curations?: RemoteCurationRow[]
  thirdParties?: RemoteThirdPartyRow[]
  /** Item slots each third party has left, by id. */
  slots?: Record<string, number>
  itemCurations?: RemoteItemCurationRow[]
  /** What GET /collections/:id/approvalData answers, by collection id (linked collections). */
  approvalData?: Record<string, LinkedApprovalDataRow>
  /** Addresses seated on the curation committee. */
  committee?: string[]
  /** The review timeline (builder-server #813), in any order: the server lists it newest first. */
  events?: RemoteCollectionEventRow[]
}

export type RemoteCollectionEventRow = {
  id: string
  collection_id: string
  type: string
  actor: 'creator' | 'curator' | 'validator' | 'system'
  actor_address: string | null
  payload: Record<string, unknown>
  created_at: string
}

export type LinkedApprovalDataRow = {
  cheque: { signature: string; qty: number; salt: string }
  root: string | null
  content_hashes: Record<string, string>
  chequeWasConsumed: boolean
}

export type BuilderResponse = { status: number; body: unknown }

type Status = 'draft' | 'published' | 'under_review' | 'rejected' | 'disabled'
const STATUSES: Status[] = ['published', 'draft', 'under_review', 'rejected', 'disabled']

// What builder-server keeps from its own row on a PUT, whatever the client sends.
const SERVER_OWNED_COLLECTION_FIELDS = [
  'eth_address',
  'urn',
  'lock',
  'created_at',
  'updated_at',
  'forum_link',
  'reviewed_at',
  'is_published',
  'is_approved'
] as const
const SERVER_OWNED_ITEM_FIELDS = [
  'eth_address',
  'blockchain_item_id',
  'is_published',
  'is_approved',
  'in_catalyst',
  'total_supply',
  'created_at',
  'updated_at'
] as const

function withoutFields<T extends object>(sent: Partial<T>, fields: readonly string[]): Partial<T> {
  return Object.fromEntries(Object.entries(sent).filter(([key]) => !fields.includes(key))) as Partial<T>
}

const ok = (data: unknown): BuilderResponse => ({ status: 200, body: { ok: true, data } })
const notFound = (what: string): BuilderResponse => ({ status: 404, body: { ok: false, error: `${what} not found` } })

const isLinked = (row: RemoteCollectionRow) => row.urn.split(':')[3] === 'collections-thirdparty'

// Mirrors builder-server's `/:address/collections` status filter (Collection.getStatusStatement).
function statusOf(row: RemoteCollectionRow): Status | null {
  if (isLinked(row)) return null
  if (!row.is_published) return 'draft'
  if (row.curation_status === 'rejected') return 'rejected'
  if (!row.is_approved) {
    const approvedBefore =
      row.curation_status === 'approved' || (row.curation_status == null && row.reviewed_at !== row.created_at)
    return approvedBefore ? 'disabled' : 'under_review'
  }
  return row.curation_status === 'pending' ? 'under_review' : 'published'
}

function paginate<T>(rows: T[], query: URLSearchParams) {
  const page = Number(query.get('page') ?? 1)
  const limit = Number(query.get('limit') ?? rows.length)
  return {
    results: rows.slice((page - 1) * limit, page * limit),
    total: rows.length,
    page,
    limit,
    pages: Math.max(1, Math.ceil(rows.length / Math.max(limit, 1)))
  }
}

export type RemoteCurationRow = {
  id: string
  collection_id: string
  status: 'pending' | 'approved' | 'rejected'
  assignee: string | null
  /** `'validator'` or the deciding curator; null while undecided. */
  reviewed_by?: string | null
  rejection_reasons?: string[] | null
  rejection_message?: string | null
  created_at: string
  updated_at: string
}

const conflict = (error: string, data?: unknown): BuilderResponse => ({ status: 409, body: { ok: false, error, data } })
const locked = (): BuilderResponse => ({ status: 423, body: { ok: false, error: 'The collection is locked' } })
const LOCK_MS = 24 * 60 * 60 * 1000
// builder-server refuses writes to an unpublished collection for a day after its publish transaction was sent.
const isLocked = (row: RemoteCollectionRow | undefined) =>
  !!row?.lock && !row.is_published && Date.parse(row.lock) + LOCK_MS > Date.now()
const now = () => new Date().toISOString()
const hashOf = (text: string) => keccak256(toHex(text))

/**
 * An in-memory builder-server: reads answer from the store and writes mutate it, so a reload shows what was
 * saved and a spec asserts outcomes rather than requests. Routes the suite doesn't need yet answer null.
 */
export function createBuilderServer(seed: BuilderSeed = {}) {
  const collections = new Map((seed.collections ?? []).map(row => [row.id, structuredClone(row)]))
  const items = new Map((seed.items ?? []).map(row => [row.id, structuredClone(row)]))
  const curations = new Map((seed.curations ?? []).map(row => [row.collection_id, structuredClone(row)]))
  const committee = seed.committee ?? []
  const thirdParties = new Map((seed.thirdParties ?? []).map(row => [row.id, structuredClone(row)]))
  const slots = { ...seed.slots }
  const itemCurations = new Map((seed.itemCurations ?? []).map(row => [row.item_id, structuredClone(row)]))
  const approvalData = new Map(Object.entries(structuredClone(seed.approvalData ?? {})))
  const events = structuredClone(seed.events ?? [])

  const eventsOf = (collectionId: string) =>
    events
      .filter(event => event.collection_id === collectionId)
      .sort((a, b) => b.created_at.localeCompare(a.created_at))
  function recordEvent(
    collectionId: string,
    type: string,
    actor: RemoteCollectionEventRow['actor'],
    payload: Record<string, unknown> = {}
  ) {
    const event: RemoteCollectionEventRow = {
      id: `event-${events.length + 1}`,
      collection_id: collectionId,
      type,
      actor,
      actor_address: null,
      payload,
      created_at: now()
    }
    events.push(event)
    return event
  }

  const itemsOf = (collectionId: string) => [...items.values()].filter(item => item.collection_id === collectionId)
  const withCount = (row: RemoteCollectionRow): RemoteCollectionRow => ({
    ...row,
    item_count: String(itemsOf(row.id).length)
  })
  const byContract = (address: string) =>
    [...collections.values()].find(row => row.contract_address?.toLowerCase() === address.toLowerCase())

  function setCuration(collectionId: string, curation: RemoteCurationRow) {
    curations.set(collectionId, curation)
    const row = collections.get(collectionId)
    if (row) row.curation_status = curation.status
  }

  function listCollections(address: string, query: URLSearchParams): BuilderResponse {
    const q = query.get('q')?.toLowerCase()
    const published = query.get('is_published')
    const owned = [...collections.values()]
      .filter(row => [row.eth_address, ...row.managers, ...row.minters].some(a => a.toLowerCase() === address))
      .filter(row => !q || row.name.toLowerCase().includes(q))
      .filter(row => published === null || String(row.is_published) === published)
    const counts = Object.fromEntries(STATUSES.map(s => [s, owned.filter(row => statusOf(row) === s).length]))
    const status = query.get('status')
    const type = query.get('type')
    const rows = owned
      .filter(row => !status || statusOf(row) === status)
      .filter(row => !type || (type === 'third_party') === isLinked(row))
      .sort((a, b) => b.updated_at.localeCompare(a.updated_at))
      .map(withCount)
    return ok({ ...paginate(rows, query), counts })
  }

  // builder-server's curation filters (Collection.model getCurationStatusStatement), standard collections only.
  function curationStatusMatches(row: RemoteCollectionRow, status: string): boolean {
    const curation = curations.get(row.id)
    const onChain = !!row.contract_address && row.is_published
    const reviewed = row.reviewed_at !== null && row.reviewed_at !== row.created_at
    switch (status) {
      case 'pending':
        return curation ? curation.status === 'pending' && !row.is_approved : !row.is_approved
      case 'approved':
        return curation ? curation.status === 'approved' && row.is_approved : row.is_approved
      case 'rejected':
        return curation?.status === 'rejected' && onChain
      case 'to_review':
        return (
          !curation?.assignee &&
          onChain &&
          (curation?.status === 'pending' || (!curation && !row.is_approved && !reviewed))
        )
      case 'under_review':
        return !!curation?.assignee && curation.status === 'pending'
      case 'disabled':
        return !row.is_approved && (curation?.status === 'approved' || (!curation && reviewed))
      default:
        return false
    }
  }

  function listCurationCollections(query: URLSearchParams): BuilderResponse {
    const statuses = query.getAll('status')
    const assignees = query.getAll('assignee').map(a => a.toLowerCase())
    const reviewType = query.get('review_type')
    const q = query.get('q')?.toLowerCase()
    const rows = [...collections.values()]
      .filter(row => row.is_published && !isLinked(row))
      .filter(row => !q || row.name.toLowerCase().includes(q))
      .filter(row => statuses.length === 0 || statuses.some(status => curationStatusMatches(row, status)))
      .filter(row => {
        if (assignees.length === 0) return true
        const assignee = curations.get(row.id)?.assignee?.toLowerCase() ?? null
        return assignees.some(a => (a === 'unassigned' ? assignee === null : a === assignee))
      })
      .filter(row => {
        if (!reviewType) return true
        const approvedBefore = row.is_approved || (row.reviewed_at !== null && row.reviewed_at !== row.created_at)
        return reviewType === 'updates' ? approvedBefore : !approvedBefore
      })
      .map(withCount)
    return ok(paginate(rows, query))
  }

  function saveCollection(id: string, body: unknown): BuilderResponse {
    const { collection: sent = {}, data } = (body ?? {}) as { collection?: Partial<RemoteCollectionRow>; data?: string }
    const row = collections.get(id)
    if (isLocked(row)) return locked()
    const name = sent.name ?? row?.name
    const taken = [...collections.values()].some(
      other => other.id !== id && other.name.toLowerCase() === name?.toLowerCase()
    )
    if (taken) return conflict('Name already in use')
    const saved: RemoteCollectionRow = {
      ...(row ?? collection({ created_at: now() })),
      ...withoutFields(sent, SERVER_OWNED_COLLECTION_FIELDS),
      id,
      updated_at: now()
    }
    // The real server derives both from the factory and the initialize calldata.
    if (data && !saved.is_published) {
      saved.salt = hashOf(id)
      saved.contract_address = '0x' + hashOf(`${saved.salt}${data}`).slice(-40)
    }
    collections.set(id, saved)
    return ok(withCount(saved))
  }

  // What the indexer behind builder-server would learn from a mined transaction.
  function applyTransaction(tx: SentTransaction) {
    const row = byContract(tx.to)
    const args = tx.args
    if (row && tx.fn === 'transferCreatorship' && args) row.eth_address = String(args.creator).toLowerCase()
    if (row && (tx.fn === 'setMinters' || tx.fn === 'setManagers') && args) {
      const key = tx.fn === 'setMinters' ? 'minters' : 'managers'
      const [addresses, values] = [args[0] as string[], args[1] as boolean[]]
      const next = new Set(row[key].map(a => a.toLowerCase()))
      addresses.forEach((address, index) =>
        values[index] ? next.add(address.toLowerCase()) : next.delete(address.toLowerCase())
      )
      row[key] = [...next]
    }
    if (tx.fn === 'reviewThirdPartyWithRoot' && args) {
      const thirdParty = thirdParties.get(String(args.thirdPartyId))
      if (thirdParty) thirdParty.root = String(args.root)
    }
    if (tx.fn === 'manageCollection' && args) {
      const target = byContract(String(args.collection))
      for (const call of args.data as string[]) {
        const inner = decodeTransaction({ hash: tx.hash, from: tx.from, to: String(args.collection), data: call })
        if (target && inner.fn === 'setApproved' && inner.args) {
          target.is_approved = Boolean(inner.args.value)
          target.reviewed_at = now()
        }
        if (target && inner.fn === 'rescueItems' && inner.args) {
          const ids = (inner.args.itemIds as bigint[]).map(String)
          const hashes = inner.args.contentHashes as string[]
          for (const item of itemsOf(target.id)) {
            const index = ids.indexOf(String(item.blockchain_item_id))
            if (index !== -1) item.content_hash = hashes[index]
          }
        }
      }
    }
  }

  function handleCollection(
    method: string,
    id: string,
    sub: string | undefined,
    query: URLSearchParams,
    body: unknown
  ) {
    if (method === 'PUT' && !sub) return saveCollection(id, body)
    const row = collections.get(id)
    if (!row) return notFound('collection')
    if (method === 'GET' && !sub) return ok(withCount(row))
    if (method === 'DELETE' && !sub) {
      if (isLocked(row)) return locked()
      collections.delete(id)
      return ok(true)
    }
    if (method === 'GET' && sub === 'items') {
      const status = query.get('status')
      const synced = query.get('synced')
      const mappingStatus = query.get('mappingStatus')
      const rows = itemsOf(id)
        .filter(item => !status || itemCurations.get(item.id)?.status === status)
        .filter(
          item =>
            synced === null ||
            (synced === 'true') ===
              (!!item.catalyst_content_hash && item.catalyst_content_hash === item.local_content_hash)
        )
        .filter(item => mappingStatus !== 'missing_mapping' || !item.mappings)
        // No fixture has a mapping waiting for migration.
        .filter(() => mappingStatus !== 'unpublished_mapping')
      return ok(query.has('page') ? paginate(rows, query) : rows)
    }
    if (method === 'POST' && sub === 'tos') return { status: 200, body: { ok: true } }
    if (method === 'POST' && sub === 'lock') {
      row.lock = now()
      return ok(row.lock)
    }
    if (method === 'POST' && sub === 'publish' && isLinked(row)) {
      const { itemIds = [] } = (body ?? {}) as { itemIds?: string[] }
      const thirdPartyId = row.urn.split(':').slice(0, 5).join(':')
      if ((slots[thirdPartyId] ?? 0) < itemIds.length) return conflict('Not enough slots')
      slots[thirdPartyId] = (slots[thirdPartyId] ?? 0) - itemIds.length
      row.is_published = true
      const published = itemIds.map(itemId => {
        const item = items.get(itemId)!
        item.is_published = true
        const curation: RemoteItemCurationRow = {
          id: `item-curation-${itemId}`,
          item_id: itemId,
          status: 'pending',
          content_hash: item.local_content_hash ?? null,
          is_mapping_complete: true,
          created_at: now(),
          updated_at: now()
        }
        itemCurations.set(itemId, curation)
        return curation
      })
      return ok({ collection: withCount(row), items: itemsOf(id), itemCurations: published })
    }
    if (method === 'POST' && sub === 'publish') {
      row.is_published = true
      row.urn = `urn:decentraland:amoy:collections-v2:${row.contract_address}`
      itemsOf(id).forEach((item, index) => {
        item.is_published = true
        item.blockchain_item_id ??= String(index)
        item.urn = `${row.urn}:${item.blockchain_item_id}`
      })
      return ok({ collection: withCount(row), items: itemsOf(id) })
    }
    if (method === 'POST' && sub === 'post') {
      if (row.forum_link) return conflict('The forum post already exists', { forum_link: row.forum_link })
      row.forum_link = `https://forum.decentraland.org/t/e2e-${id}`
      return ok(row.forum_link)
    }
    if (method === 'GET' && sub === 'events') return ok(paginate(eventsOf(id), query))
    if (method === 'POST' && sub === 'comments') {
      const { message, itemId } = body as { message: string; itemId?: string }
      const actor = (seed.committee ?? []).length > 0 ? 'curator' : 'creator'
      return ok(recordEvent(id, 'review.comment', actor, { message: message.trim(), ...(itemId ? { itemId } : {}) }))
    }
    if (method === 'POST' && sub === 'validations') {
      return ok(
        recordEvent(id, 'review.automated_started', 'validator', { validationId: `validation-${events.length + 1}` })
      )
    }
    if (method === 'GET' && sub === 'curation') return ok(curations.get(id) ?? null)
    if (method === 'POST' && sub === 'curation') {
      if (curations.get(id)?.status === 'pending')
        return { status: 400, body: { ok: false, error: 'There is already an ongoing review request' } }
      const { assignee = null } = (body as { curation?: { assignee?: string | null } })?.curation ?? {}
      const curation: RemoteCurationRow = {
        id: `curation-${id}`,
        collection_id: id,
        status: 'pending',
        assignee,
        created_at: now(),
        updated_at: now()
      }
      setCuration(id, curation)
      return ok(curation)
    }
    if (method === 'PATCH' && sub === 'curation') {
      const current = curations.get(id)
      if (!current) return notFound('curation')
      const { rejectionReasons, rejectionMessage, ...changes } =
        (body as { curation?: Partial<RemoteCurationRow> & { rejectionReasons?: string[]; rejectionMessage?: string } })
          ?.curation ?? {}
      // builder-server stores the rejection feedback snake-cased, and only on a rejection.
      const feedback =
        changes.status === 'rejected'
          ? { rejection_reasons: rejectionReasons ?? null, rejection_message: rejectionMessage ?? null }
          : {}
      const curation = { ...current, ...changes, ...feedback, updated_at: now() }
      setCuration(id, curation)
      return ok(curation)
    }
    if (method === 'GET' && sub === 'approvalData') {
      const data = approvalData.get(id)
      return data ? ok(data) : notFound('approval data')
    }
    if (method === 'GET' && sub === 'itemCurations') {
      const ids = new Set(query.getAll('itemIds[]'))
      return ok(
        itemsOf(id)
          .filter(item => ids.size === 0 || ids.has(item.id))
          .flatMap(item => itemCurations.get(item.id) ?? [])
      )
    }
    return null
  }

  function handleItem(method: string, id: string, sub: string | undefined, body: unknown) {
    const row = items.get(id)
    const collectionOf = (collectionId: string | null | undefined) =>
      collectionId ? collections.get(collectionId) : undefined
    if (method === 'PUT' && !sub) {
      const sent = (body as { item?: Partial<RemoteItemRow> } | undefined)?.item ?? {}
      if (isLocked(collectionOf(sent.collection_id ?? row?.collection_id))) return locked()
      const saved: RemoteItemRow = {
        ...(row ?? wearable({ created_at: now() })),
        ...withoutFields(sent, SERVER_OWNED_ITEM_FIELDS),
        id,
        updated_at: now()
      }
      items.set(id, saved)
      return ok(saved)
    }
    if (!row) return notFound('item')
    if (method === 'GET' && !sub) return ok(row)
    if (method === 'DELETE' && !sub) {
      if (isLocked(collectionOf(row.collection_id))) return locked()
      items.delete(id)
      return ok(true)
    }
    if (method === 'GET' && sub === 'curation') return ok(itemCurations.get(id) ?? null)
    if (method === 'PATCH' && sub === 'curation') {
      const current = itemCurations.get(id)
      if (!current) return notFound('item curation')
      const { status } = (body as { curation?: Partial<RemoteItemCurationRow> })?.curation ?? {}
      const curation = { ...current, ...(status && { status }), updated_at: now() }
      // builder-server stamps the item's current hash on an approval, which the app checks against what it deployed.
      if (curation.status === 'approved') {
        curation.content_hash = row.local_content_hash ?? null
        row.is_approved = true
        row.catalyst_content_hash = curation.content_hash
      }
      itemCurations.set(id, curation)
      return ok(curation)
    }
    return null
  }

  function handle(method: string, path: string, query: URLSearchParams, body: unknown): BuilderResponse | null {
    const parts = path.split('/').filter(Boolean)
    const [head, id, sub, subPath] = parts

    if (method === 'GET' && parts.length === 1 && head === 'committee')
      return ok(committee.map(address => ({ address })))
    if (method === 'GET' && parts.length === 1 && head === 'rarities') return ok(RARITIES)
    if (method === 'GET' && parts.length === 1 && head === 'curations') return ok([...curations.values()])
    if (method === 'GET' && parts.length === 1 && head === 'thirdParties') return ok([...thirdParties.values()])
    if (head === 'thirdParties' && id) {
      const thirdPartyId = decodeURIComponent(id)
      const row = thirdParties.get(thirdPartyId)
      if (!row) return notFound('third party')
      if (method === 'GET' && !sub) return ok(row)
      if (method === 'GET' && sub === 'slots') return ok(slots[thirdPartyId] ?? 0)
      if (method === 'PATCH' && !sub) {
        Object.assign(row, (body as { thirdParty?: Partial<RemoteThirdPartyRow> })?.thirdParty ?? body)
        return ok(row)
      }
    }
    if (method === 'GET' && parts.length === 2 && /^0x[0-9a-f]{40}$/i.test(head) && id === 'collections') {
      return listCollections(head.toLowerCase(), query)
    }
    if (head === 'collections' && id && sub === 'owner' && subPath === 'sync' && method === 'POST') {
      const row = collections.get(id)
      return row ? ok(withCount(row)) : notFound('collection')
    }
    if (method === 'GET' && parts.length === 1 && head === 'collections') return listCurationCollections(query)
    // An appeal opens a new pending request over the rejection, as builder-server does.
    if (head === 'collections' && id && sub === 'curation' && subPath === 'appeal' && method === 'POST') {
      // The same answers as builder-server, which the client tells apart by message.
      const latest = eventsOf(id)[0]
      if (latest?.type === 'review.appeal_requested' && curations.get(id)?.status === 'pending') {
        return conflict('There is already an open appeal for this collection')
      }
      if (curations.get(id)?.status !== 'rejected') return conflict('Only rejected collections can be appealed')
      const note = (body as { note?: string } | undefined)?.note ?? ''
      recordEvent(id, 'review.appeal_requested', 'creator', { note })
      const curation: RemoteCurationRow = {
        id: `curation-${id}-appeal`,
        collection_id: id,
        status: 'pending',
        assignee: null,
        reviewed_by: null,
        created_at: now(),
        updated_at: now()
      }
      setCuration(id, curation)
      return ok(curation)
    }
    if (head === 'collections' && id && sub === 'curation' && subPath === 'post' && method === 'POST') {
      return collections.has(id) ? { status: 200, body: { ok: true } } : notFound('collection')
    }
    if (head === 'collections' && id) return handleCollection(method, id, sub, query, body)
    if (head === 'items' && id) return handleItem(method, id, sub, body)
    return null
  }

  return {
    handle,
    applyTransaction,
    collections,
    items,
    curations,
    thirdParties,
    slots,
    itemCurations,
    approvalData,
    events
  }
}

export type BuilderServer = ReturnType<typeof createBuilderServer>

// The collection's Discourse topic on the Decentraland forum: the creator ↔ curation committee channel, ported
// from the legacy builder (modules/forum). builder-server writes the topic itself; replies are posted in English.
import { config } from '~/config'
import { errorCode, track } from '~/lib/analytics'
import { BuilderServerError } from '~/lib/builder'
import { CollectionDisplayStatus, canManageCollectionItems, toSafeLink, type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { captureError } from '~/lib/monitoring'
import { openExternal } from '~/lib/navigation'

export type ForumPostSource = 'publish' | 'recovery'
export type ForumPostSurface = 'detail' | 'detail_menu' | 'curation_list' | 'review_verdict'

/** Curators give their feedback on the topic while the collection, or a change to it, is being reviewed. */
export function isForumPostRelevant(collection: Collection, status: CollectionDisplayStatus | undefined): boolean {
  if (!collection.forumLink || !collection.isPublished) return false
  return (
    !collection.isApproved ||
    status === CollectionDisplayStatus.UNDER_REVIEW ||
    status === CollectionDisplayStatus.REJECTED
  )
}

/** Opens the collection's topic in a new tab. */
export function openForumPost(collection: Collection, surface: ForumPostSurface): void {
  if (!collection.forumLink) return
  track('Forum post opened', { collectionId: collection.id, surface })
  openExternal(collection.forumLink)
}

/** The reply posted on the topic when a curator is assigned (or the collection is unassigned). */
export function buildAssigneeReply(assignee: string | null, assigneeName?: string): string {
  if (!assignee) return 'The collection has been unassigned.'
  if (!assigneeName) return `The collection has been assigned to ${assignee}`
  const profileUrl = `${config.get('SITES_URL')}/profile/accounts/${assignee}`
  return `The collection has been assigned to <a target="_blank" href="${profileUrl}">${assigneeName}</a>`
}

const CONFLICT = 409

// The server checks publication against the subgraph, which can lag behind the sync that triggered the post.
// Nothing else is retried: the server opens the topic before saving its link, so a retried 5xx or timeout could
// open a second one. Recovery covers those instead.
const isNotPublishedYet = (error: unknown) =>
  error instanceof BuilderServerError && error.status === CONFLICT && error.message.includes('not published')

const existingLink = (error: unknown) =>
  error instanceof BuilderServerError && error.status === CONFLICT
    ? toSafeLink((error.data as { forum_link?: string } | undefined)?.forum_link ?? null)
    : undefined

const RETRIES = 3
const RETRY_DELAY_MS = 5000

export type ForumTopic = { link: string | undefined; existing: boolean }

/** Asks builder-server to open the collection's topic; `existing` when it already had one. */
export async function createForumTopic(
  collectionId: string,
  createPost: (collectionId: string) => Promise<string>,
  retries = RETRIES,
  retryDelayMs = RETRY_DELAY_MS
): Promise<ForumTopic> {
  for (let attempt = 0; ; attempt++) {
    try {
      return { link: toSafeLink(await createPost(collectionId)), existing: false }
    } catch (error) {
      const link = existingLink(error)
      if (link) return { link, existing: true }
      if (!isNotPublishedYet(error) || attempt >= retries) throw error
      await new Promise(resolve => setTimeout(resolve, retryDelayMs * 2 ** attempt))
    }
  }
}

export type PostCollectionDeps = {
  createPost: (collectionId: string) => Promise<string>
  /** The collection's current link on the server, read again right before posting. */
  fetchForumLink: (collectionId: string) => Promise<string | undefined>
  /** Runs `task` while no other tab of this browser runs one under the same key. */
  exclusive: (key: string, task: () => Promise<void>) => Promise<void>
  onPosted: (forumLink: string | undefined) => void
  retryDelayMs?: number
}

// Collections whose topic this tab is creating or already tried to recover, so it never posts twice. Other tabs
// are kept out by `exclusive` plus the re-read; a collaborator in another browser can still race, which only
// builder-server could prevent.
const posting = new Set<string>()
const recovered = new Set<string>()

/** Opens the collection's forum topic once its publish is synced; failures are reported, never thrown. */
export async function postCollectionToForum(
  collection: Collection,
  source: ForumPostSource,
  deps: PostCollectionDeps
): Promise<void> {
  if (collection.forumLink || posting.has(collection.id)) return
  posting.add(collection.id)
  try {
    await deps.exclusive(`forum-post:${collection.id}`, async () => {
      const current = await deps.fetchForumLink(collection.id)
      if (current) return deps.onPosted(current)
      const { link, existing } = await createForumTopic(collection.id, deps.createPost, RETRIES, deps.retryDelayMs)
      track('Create forum post', { collectionId: collection.id, trigger: source, existing })
      deps.onPosted(link)
    })
  } catch (error) {
    track('Create forum post error', { collectionId: collection.id, trigger: source, error: errorCode(error) })
    captureError(error, { flow: 'forum_post', collectionId: collection.id, source })
  } finally {
    posting.delete(collection.id)
  }
}

// The publishing tab posts right after its publish sync, which polls for up to an hour (`CONSOLIDATE_RETRIES` in
// publishCollection.ts: raise this along with it); recovering before then could race it into a second topic, since
// builder-server's "already has a topic" check isn't atomic. The extra half hour is a margin for slow mining, which
// has no upper bound. Err on the late side: a late topic costs little, a duplicate one is public.
const RECOVERY_GRACE_MS = 90 * 60_000
// Only recent publishes are recovered, so deploying this doesn't open topics for long-settled collections.
const RECOVERY_MAX_AGE_MS = 30 * 24 * 60 * 60_000

/**
 * Whether this tab should create a topic the publishing tab never did: a synced collection waiting for its first
 * approval, opened by its owner or a collaborator, well after publishing started but within a month, once per tab.
 */
export function shouldRecoverForumPost(
  collection: Collection,
  items: Item[],
  address: string | undefined,
  now = Date.now()
): boolean {
  const publishedAt = Math.max(collection.lock ?? 0, collection.updatedAt)
  return (
    collection.isPublished &&
    !collection.isApproved &&
    !collection.forumLink &&
    items.length > 0 &&
    items.every(item => item.tokenId) &&
    now - publishedAt > RECOVERY_GRACE_MS &&
    now - publishedAt < RECOVERY_MAX_AGE_MS &&
    canManageCollectionItems(collection, address) &&
    !recovered.has(collection.id)
  )
}

export function markForumPostRecovered(collectionId: string): void {
  recovered.add(collectionId)
}

export type AssigneeReplyDeps = {
  /** The curator's claimed name: unclaimed names are free text and could impersonate someone. */
  claimedName: (address: string) => Promise<string | undefined>
  createReply: (collectionId: string, raw: string) => Promise<void>
}

/** Tells the forum topic who the collection was assigned to; the assignment itself never depends on it. */
export async function postAssigneeToForum(
  collection: Collection,
  assignee: string | null,
  deps: AssigneeReplyDeps
): Promise<void> {
  if (!collection.forumLink) return
  try {
    const name = assignee ? await deps.claimedName(assignee).catch(() => undefined) : undefined
    await deps.createReply(collection.id, buildAssigneeReply(assignee, name))
  } catch (error) {
    track('Assignee forum post error', { collectionId: collection.id, error: errorCode(error) })
    captureError(error, { flow: 'forum_post', collectionId: collection.id, step: 'assignee' })
  }
}

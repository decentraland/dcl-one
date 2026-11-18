import { beforeEach, describe, expect, it, vi } from 'vitest'
import { BuilderServerError } from './builder'
import { CollectionDisplayStatus, type Collection } from './collections'
import {
  buildAssigneeReply,
  createForumTopic,
  isForumPostRelevant,
  markForumPostRecovered,
  openForumPost,
  postAssigneeToForum,
  postCollectionToForum,
  shouldRecoverForumPost,
  type PostCollectionDeps
} from './forumPost'
import { type Item } from './items'

const analytics = vi.hoisted(() => ({ track: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ track: analytics.track, errorCode: () => 'unknown' }))
const monitoring = vi.hoisted(() => ({ captureError: vi.fn() }))
vi.mock('~/lib/monitoring', () => monitoring)
const navigation = vi.hoisted(() => ({ openExternal: vi.fn() }))
vi.mock('~/lib/navigation', () => navigation)

const OWNER = '0x00000000000000000000000000000000000000aa'
const LINK = 'https://forum.decentraland.org/t/hats/77'
const HOUR = 60 * 60_000
const NOW = Date.UTC(2026, 9, 6)

const collection = (id: string, overrides: Partial<Collection> = {}) =>
  ({
    id,
    name: 'Hats',
    owner: OWNER,
    isPublished: true,
    isApproved: false,
    managers: [],
    minters: [],
    updatedAt: NOW - 8 * HOUR,
    ...overrides
  }) as unknown as Collection
const synced = [{ id: 'i1', tokenId: '1' }] as Item[]
const postDeps = (overrides: Partial<PostCollectionDeps> = {}): PostCollectionDeps => ({
  createPost: vi.fn().mockResolvedValue(LINK),
  fetchForumLink: vi.fn().mockResolvedValue(undefined),
  exclusive: (_key, task) => task(),
  onPosted: vi.fn(),
  ...overrides
})

beforeEach(() => {
  analytics.track.mockReset()
  monitoring.captureError.mockReset()
  navigation.openExternal.mockReset()
})

describe('isForumPostRelevant', () => {
  it('shows the topic while the collection or its changes are reviewed, rejected or disabled', () => {
    const posted = collection('v1', { forumLink: LINK })
    expect(isForumPostRelevant(posted, CollectionDisplayStatus.UNDER_REVIEW)).toBe(true)
    expect(isForumPostRelevant({ ...posted, isApproved: true }, CollectionDisplayStatus.UNDER_REVIEW)).toBe(true)
    expect(isForumPostRelevant({ ...posted, isApproved: true }, CollectionDisplayStatus.REJECTED)).toBe(true)
  })

  it('hides it once the collection is published and settled, or when there is no topic', () => {
    const posted = collection('v2', { forumLink: LINK, isApproved: true })
    expect(isForumPostRelevant(posted, CollectionDisplayStatus.PUBLISHED)).toBe(false)
    expect(isForumPostRelevant(collection('v3'), CollectionDisplayStatus.UNDER_REVIEW)).toBe(false)
    expect(isForumPostRelevant(collection('v4', { forumLink: LINK, isPublished: false }), undefined)).toBe(false)
  })
})

describe('openForumPost', () => {
  it('opens the topic and records where it was opened from', () => {
    openForumPost(collection('c1', { forumLink: LINK }), 'curation_list')
    expect(navigation.openExternal).toHaveBeenCalledWith(LINK)
    expect(analytics.track).toHaveBeenCalledWith('Forum post opened', { collectionId: 'c1', surface: 'curation_list' })
  })
})

describe('buildAssigneeReply', () => {
  it('links the assignee profile by name, or names the address, or says it was unassigned', () => {
    expect(buildAssigneeReply(OWNER, 'Curator')).toMatch(
      /assigned to <a .*href=".*\/profile\/accounts\/0x0+aa">Curator<\/a>/
    )
    expect(buildAssigneeReply(OWNER)).toBe(`The collection has been assigned to ${OWNER}`)
    expect(buildAssigneeReply(null)).toBe('The collection has been unassigned.')
  })
})

describe('createForumTopic', () => {
  it('answers the new topic link, dropping one that is not a web page', async () => {
    await expect(createForumTopic('c1', vi.fn().mockResolvedValue(LINK), 0, 0)).resolves.toEqual({
      link: LINK,
      existing: false
    })
    await expect(createForumTopic('c1', vi.fn().mockResolvedValue('javascript:alert(1)'), 0, 0)).resolves.toEqual({
      link: undefined,
      existing: false
    })
  })

  it('answers the link of a topic that already existed', async () => {
    const createPost = vi
      .fn()
      .mockRejectedValue(new BuilderServerError('Forum post already exists', 409, { id: 'c1', forum_link: LINK }))

    await expect(createForumTopic('c1', createPost, 3, 0)).resolves.toEqual({ link: LINK, existing: true })
    expect(createPost).toHaveBeenCalledTimes(1)
  })

  it('retries while the server cannot see the publication yet, then gives up', async () => {
    const notPublished = new BuilderServerError('The collection is not published', 409, { id: 'c1' })
    const recovers = vi.fn().mockRejectedValueOnce(notPublished).mockResolvedValue(LINK)
    await expect(createForumTopic('c1', recovers, 3, 0)).resolves.toEqual({ link: LINK, existing: false })

    const never = vi.fn().mockRejectedValue(notPublished)
    await expect(createForumTopic('c1', never, 2, 0)).rejects.toBe(notPublished)
    expect(never).toHaveBeenCalledTimes(3)
  })

  it('never retries a failure that may have opened a topic already', async () => {
    for (const error of [new BuilderServerError('Error creating forum post', 500), new TypeError('Failed to fetch')]) {
      const createPost = vi.fn().mockRejectedValue(error)
      await expect(createForumTopic('c1', createPost, 3, 0)).rejects.toBe(error)
      expect(createPost).toHaveBeenCalledTimes(1)
    }
  })
})

describe('postCollectionToForum', () => {
  it('opens the topic once and hands its link over', async () => {
    const deps = postDeps()

    await Promise.all([
      postCollectionToForum(collection('p1'), 'publish', deps),
      postCollectionToForum(collection('p1'), 'recovery', deps)
    ])

    expect(deps.createPost).toHaveBeenCalledTimes(1)
    expect(deps.onPosted).toHaveBeenCalledWith(LINK)
    expect(analytics.track).toHaveBeenCalledWith('Create forum post', {
      collectionId: 'p1',
      trigger: 'publish',
      existing: false
    })
  })

  it('posts under the shared lock and not at all when another tab already did', async () => {
    const exclusive = vi.fn((_key: string, task: () => Promise<void>) => task())
    const deps = postDeps({ exclusive, fetchForumLink: vi.fn().mockResolvedValue(LINK) })

    await postCollectionToForum(collection('p2'), 'recovery', deps)

    expect(exclusive).toHaveBeenCalledWith('forum-post:p2', expect.any(Function))
    expect(deps.createPost).not.toHaveBeenCalled()
    expect(deps.onPosted).toHaveBeenCalledWith(LINK)
    expect(analytics.track).not.toHaveBeenCalled()
  })

  it('tells an existing topic apart in the event', async () => {
    const createPost = vi
      .fn()
      .mockRejectedValue(new BuilderServerError('Forum post already exists', 409, { forum_link: LINK }))
    await postCollectionToForum(collection('p3'), 'publish', postDeps({ createPost }))
    expect(analytics.track).toHaveBeenCalledWith('Create forum post', {
      collectionId: 'p3',
      trigger: 'publish',
      existing: true
    })
  })

  it('skips a collection that already has a topic', async () => {
    const deps = postDeps()
    await postCollectionToForum(collection('p4', { forumLink: LINK }), 'publish', deps)
    expect(deps.fetchForumLink).not.toHaveBeenCalled()
    expect(deps.createPost).not.toHaveBeenCalled()
  })

  it('reports a failure without throwing', async () => {
    const createPost = vi.fn().mockRejectedValue(new BuilderServerError('Error creating forum post', 500))
    await postCollectionToForum(collection('p5'), 'publish', postDeps({ createPost }))
    expect(monitoring.captureError).toHaveBeenCalledWith(
      expect.any(BuilderServerError),
      expect.objectContaining({ flow: 'forum_post' })
    )
  })
})

describe('shouldRecoverForumPost', () => {
  it('recovers a synced collection waiting for review, well after publishing, once per tab', () => {
    expect(shouldRecoverForumPost(collection('r1'), synced, OWNER, NOW)).toBe(true)
    markForumPostRecovered('r1')
    expect(shouldRecoverForumPost(collection('r1'), synced, OWNER, NOW)).toBe(false)
  })

  it('leaves the publishing tab its window, measured from when publishing started', () => {
    expect(shouldRecoverForumPost(collection('r2', { lock: NOW - HOUR }), synced, OWNER, NOW)).toBe(false)
    expect(shouldRecoverForumPost(collection('r3', { updatedAt: NOW - HOUR }), synced, OWNER, NOW)).toBe(false)
  })

  it('recovers once the 90-minute grace has passed', () => {
    expect(
      shouldRecoverForumPost(collection('r9', { lock: NOW - 2 * HOUR, updatedAt: NOW - 2 * HOUR }), synced, OWNER, NOW)
    ).toBe(true)
  })

  it('does not backfill collections published more than a month ago', () => {
    expect(shouldRecoverForumPost(collection('r8', { updatedAt: NOW - 31 * 24 * HOUR }), synced, OWNER, NOW)).toBe(
      false
    )
  })

  it('leaves alone collections that have a topic, are approved, are not synced, or are someone else’s', () => {
    expect(shouldRecoverForumPost(collection('r4', { forumLink: LINK }), synced, OWNER, NOW)).toBe(false)
    expect(shouldRecoverForumPost(collection('r5', { isApproved: true }), synced, OWNER, NOW)).toBe(false)
    expect(shouldRecoverForumPost(collection('r6'), [{ id: 'i1' } as Item], OWNER, NOW)).toBe(false)
    expect(shouldRecoverForumPost(collection('r7'), synced, '0xsomeoneelse', NOW)).toBe(false)
  })
})

describe('postAssigneeToForum', () => {
  it('replies with the claimed name, only on collections with a topic', async () => {
    const createReply = vi.fn().mockResolvedValue(undefined)
    const claimedName = vi.fn().mockResolvedValue('Curator')

    await postAssigneeToForum(collection('a1'), OWNER, { claimedName, createReply })
    expect(createReply).not.toHaveBeenCalled()

    await postAssigneeToForum(collection('a1', { forumLink: LINK }), OWNER, { claimedName, createReply })
    expect(createReply).toHaveBeenCalledWith('a1', expect.stringContaining('>Curator</a>'))
  })

  it('reports a failed reply without throwing', async () => {
    const createReply = vi.fn().mockRejectedValue(new Error('forum down'))
    await postAssigneeToForum(collection('a2', { forumLink: LINK }), null, { claimedName: vi.fn(), createReply })
    expect(analytics.track).toHaveBeenCalledWith('Assignee forum post error', { collectionId: 'a2', error: 'unknown' })
  })
})

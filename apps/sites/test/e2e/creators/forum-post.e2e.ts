import { afterEach, describe, expect, it, vi } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

// Published two hours ago and waiting for its first review: past the grace the publishing tab gets to post.
const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60_000).toISOString()
const pending = publishedCollection({
  name: 'Neon Streetwear',
  is_approved: false,
  curation_status: 'pending',
  created_at: twoHoursAgo,
  reviewed_at: twoHoursAgo,
  updated_at: twoHoursAgo
})
const bandana = publishedItem(pending, 0, { is_approved: false })

describe('forum post', () => {
  it('opens the review topic a publish never posted, and links to it', async () => {
    app = await launchApp({ path: `/collections/${pending.id}`, builder: { collections: [pending], items: [bandana] } })
    const { page } = app

    await waitForTestId(page, 'forum-post', 30_000)
    const link = app.builder.collections.get(pending.id)?.forum_link
    expect(link).toMatch(/^https:\/\/forum\.decentraland\.org\//)
    await app.waitForEvent('Create forum post', { collectionId: pending.id, trigger: 'recovery', existing: false })

    await clickTestId(page, 'forum-post')
    // The page's window.open reaches node asynchronously.
    await vi.waitFor(() => expect(app!.openedUrls).toEqual([link]), { timeout: 10_000 })
    await app.waitForEvent('Forum post opened', { collectionId: pending.id, surface: 'detail' })
  })
})

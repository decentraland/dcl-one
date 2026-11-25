import { afterEach, describe, expect, it } from 'vitest'
import { collection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, isShown, testIdsStartingWith, waitForTestId } from './helpers/dom'
import { decodeTransaction } from './helpers/rpc'
import { TEST_ADDRESS } from './helpers/session'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const CREATOR = '0x' + '22'.repeat(20)
const NOW = new Date().toISOString()
// On chain, never reviewed and without a review request yet: what the committee sees as new and to review.
const submitted = collection({
  name: 'Neon Streetwear',
  eth_address: CREATOR,
  contract_address: '0x' + 'c0'.repeat(20),
  is_published: true,
  created_at: NOW,
  reviewed_at: NOW,
  updated_at: NOW
})
// The stored files are the ones on chain, so the approval needs no rescue.
const items = [0, 1].map(index =>
  publishedItem(submitted, index, {
    is_approved: false,
    in_catalyst: false,
    content_hash: `bafyitem${index}`,
    local_content_hash: `bafyitem${index}`
  })
)
const builder = { collections: [submitted], items, committee: [TEST_ADDRESS] }
const reviewPath = `/collections/editor?collection=${submitted.id}&reviewing=true`

const curationState = (a: App) =>
  a.page.$eval('[data-testid="review-bar"] [data-testid="curation-state"]', el => el.getAttribute('data-state'))

const curationTab = (page: App['page']) => page.getByRole('link', { name: 'Curation', exact: true })

describe('curation', () => {
  it('offers no auto-reviewing filter while the automated review is off', async () => {
    app = await launchApp({ path: '/curation?status=auto_reviewing', builder })
    const { page } = app
    await waitForTestId(page, 'curation-row')
    await clickTestId(page, 'curation-status-filter')
    await waitForTestId(page, 'curation-status-filter-listbox')
    const options = await testIdsStartingWith(page, 'curation-status-filter-option-')
    expect(options).toContain('curation-status-filter-option-to_review')
    expect(options).not.toContain('curation-status-filter-option-auto_reviewing')
  })

  it('lists a submission, assigns it and approves it from the editor', async () => {
    app = await launchApp({ path: '/curation', builder })
    const { page } = app
    await curationTab(page).waitFor({ timeout: 20_000 })
    await waitForTestId(page, 'curation-row')

    await clickTestId(page, 'curation-row-assign-me')
    await waitForTestId(page, 'assign-curator-modal')
    await clickTestId(page, 'assign-curator-submit')
    await waitForTestId(page, 'curation-row-curator')
    expect(app.builder.curations.get(submitted.id)).toMatchObject({ status: 'pending', assignee: TEST_ADDRESS })
    await app.waitForEvent('bd_curator_assigned', { collection_id: submitted.id })

    await clickTestId(page, 'curation-row-link')
    await waitForTestId(page, 'review-bar', 30_000)
    await clickTestId(page, 'review-action-approve')
    await waitForTestId(page, 'approval-steps', 30_000)

    // Items go to the Catalyst first, then the committee approves the collection on chain.
    await clickTestId(page, 'approval-deploy')
    await waitForTestId(page, 'approval-approve', 60_000)
    expect(app.writes.filter(write => write === 'POST /content/entities')).toHaveLength(items.length)
    await clickTestId(page, 'approval-approve')
    await waitForTestId(page, 'success-modal-modal', 60_000)

    const approval = app.transactions.find(tx => tx.fn === 'manageCollection')
    expect(String(approval?.args?.collection).toLowerCase()).toBe(submitted.contract_address)
    expect(app.builder.collections.get(submitted.id)?.is_approved).toBe(true)
    expect(app.builder.curations.get(submitted.id)?.status).toBe('approved')
    await app.waitForEvent('bd_collection_approved', { collection_id: submitted.id })
    await clickTestId(page, 'success-modal-done')
    await page.waitForFunction(
      () =>
        document
          .querySelector('[data-testid="review-bar"] [data-testid="curation-state"]')
          ?.getAttribute('data-state') === 'approved'
    )
  })

  it('rejects a submission, taking it over first', async () => {
    app = await launchApp({ path: reviewPath, builder })
    const { page } = app
    await waitForTestId(page, 'review-bar', 30_000)

    await clickTestId(page, 'review-action-reject')
    // Nobody is assigned yet, so the curator takes the review before deciding.
    await waitForTestId(page, 'assign-curator-modal')
    await clickTestId(page, 'assign-curator-submit')
    await waitForTestId(page, 'review-reject-confirm')
    await clickTestId(page, 'review-reject-confirm')

    await page.waitForFunction(
      () =>
        document
          .querySelector('[data-testid="review-bar"] [data-testid="curation-state"]')
          ?.getAttribute('data-state') === 'rejected',
      undefined,
      { timeout: 15_000 }
    )
    expect(app.builder.curations.get(submitted.id)).toMatchObject({ status: 'rejected', assignee: TEST_ADDRESS })
    expect(await curationState(app)).toBe('rejected')
    await app.waitForEvent('bd_curation_rejected', { collection_id: submitted.id })
  })

  it('is not there for anyone off the committee', async () => {
    app = await launchApp({ path: '/curation', builder: { ...builder, committee: [] } })
    await app.page.waitForSelector('main[data-status="404"]', { state: 'attached' })
    expect(await curationTab(app.page).count()).toBe(0)
  })
})

describe('curation of a collection already approved once', () => {
  const T0 = new Date(Date.now() - 30 * 24 * 60 * 60_000).toISOString()
  const T1 = new Date(Date.now() - 20 * 24 * 60 * 60_000).toISOString()
  // On the market since its first approval: reviewed after it was created.
  const approvedOnce = (overrides: Parameters<typeof collection>[0] = {}) =>
    collection({
      name: 'Neon Streetwear',
      eth_address: CREATOR,
      contract_address: '0x' + 'c1'.repeat(20),
      is_published: true,
      is_approved: true,
      created_at: T0,
      reviewed_at: T1,
      updated_at: T1,
      ...overrides
    })
  const curationOf = (collectionId: string, status: 'pending' | 'approved') => ({
    id: `curation-${collectionId}`,
    collection_id: collectionId,
    status,
    assignee: TEST_ADDRESS,
    created_at: T1,
    updated_at: T1
  })
  const reviewing = (collectionId: string) => `/collections/editor?collection=${collectionId}&reviewing=true`
  const reviewState = (a: App, state: string) =>
    a.page.waitForFunction(
      (expected: string) =>
        document
          .querySelector('[data-testid="review-bar"] [data-testid="curation-state"]')
          ?.getAttribute('data-state') === expected,
      state,
      { timeout: 30_000 }
    )

  it('rescues changed items on chain before deploying them', async () => {
    const updated = approvedOnce()
    // The creator changed both items: their stored files no longer match the hashes on chain.
    const changed = [0, 1].map(index =>
      publishedItem(updated, index, { content_hash: `bafyonchain${index}`, local_content_hash: `bafychanged${index}` })
    )
    app = await launchApp({
      path: reviewing(updated.id),
      builder: {
        collections: [updated],
        items: changed,
        curations: [curationOf(updated.id, 'pending')],
        committee: [TEST_ADDRESS]
      }
    })
    const { page } = app
    await waitForTestId(page, 'review-bar', 30_000)
    await clickTestId(page, 'review-action-approve')
    // Changed items start with the rescue; the collection is already approved, so no approval follows the deploy.
    await waitForTestId(page, 'approval-step-rescue', 30_000)

    await clickTestId(page, 'approval-rescue')
    await waitForTestId(page, 'approval-deploy', 60_000)
    const rescue = app.transactions.find(tx => tx.fn === 'manageCollection')
    expect(String(rescue?.args?.collection).toLowerCase()).toBe(updated.contract_address)
    // The rescue puts each changed item's current hash on chain, and the indexer reports exactly those hashes.
    const inner = decodeTransaction({ ...rescue!, data: (rescue!.args!.data as string[])[0] })
    expect(inner.fn).toBe('rescueItems')
    const sent = Object.fromEntries(
      (inner.args!.itemIds as Array<{ toString(): string }>).map((id, index) => [
        String(id),
        (inner.args!.contentHashes as string[])[index]
      ])
    )
    for (const item of changed) {
      expect(sent[item.blockchain_item_id!]).toBe(item.local_content_hash)
      expect(app.builder.items.get(item.id)?.content_hash).toBe(item.local_content_hash)
    }

    await clickTestId(page, 'approval-deploy')
    await waitForTestId(page, 'success-modal-modal', 60_000)
    expect(app.writes.filter(write => write === 'POST /content/entities')).toHaveLength(changed.length)
    expect(app.transactions.filter(tx => tx.fn === 'manageCollection')).toHaveLength(1)
    expect(app.builder.curations.get(updated.id)?.status).toBe('approved')
    await app.waitForEvent('bd_items_rescued', { collection_id: updated.id, item_count: changed.length })
  })

  it('disables an approved collection on chain', async () => {
    const approved = approvedOnce()
    app = await launchApp({
      path: reviewing(approved.id),
      builder: {
        collections: [approved],
        items: [publishedItem(approved, 0)],
        curations: [curationOf(approved.id, 'approved')],
        committee: [TEST_ADDRESS]
      }
    })
    const { page } = app
    await waitForTestId(page, 'review-bar', 30_000)
    await clickTestId(page, 'review-action-disable')
    await clickTestId(page, 'review-disable-confirm')

    await reviewState(app, 'disabled')
    expect(app.builder.collections.get(approved.id)?.is_approved).toBe(false)
    expect(app.transactions.map(tx => tx.fn)).toEqual(['manageCollection'])
    await app.waitForEvent('bd_collection_rejected', { collection_id: approved.id })
  })

  it('enables a disabled collection again', async () => {
    const disabled = approvedOnce({ is_approved: false })
    const items = [publishedItem(disabled, 0, { content_hash: 'bafyitem0', local_content_hash: 'bafyitem0' })]
    app = await launchApp({
      path: reviewing(disabled.id),
      builder: {
        collections: [disabled],
        items,
        curations: [curationOf(disabled.id, 'approved')],
        committee: [TEST_ADDRESS]
      }
    })
    const { page } = app
    await waitForTestId(page, 'review-bar', 30_000)
    await clickTestId(page, 'review-action-enable')
    // Enabling switches the collection back on as last approved: one approval, nothing redeployed.
    await waitForTestId(page, 'approval-step-approve', 30_000)
    await clickTestId(page, 'approval-approve')
    await waitForTestId(page, 'success-modal-modal', 60_000)
    expect(app.transactions.map(tx => tx.fn)).toEqual(['manageCollection'])
    expect(app.writes).not.toContain('POST /content/entities')

    expect(app.builder.collections.get(disabled.id)?.is_approved).toBe(true)
    await app.waitForEvent('bd_collection_enabled', { collection_id: disabled.id })
  })
})

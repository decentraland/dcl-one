import { zeroHash } from 'viem'
import { afterEach, describe, expect, it } from 'vitest'
import { THIRD_PARTY_ID, linkedCollection, linkedItem, thirdParty } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'
import { TEST_ADDRESS } from './helpers/session'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const CREATOR = '0x' + '44'.repeat(20)
// A cheque the creator signed for two new slots.
const CHEQUE = { signature: '0x' + 'ab'.repeat(32) + '11'.repeat(32) + '1b', qty: 2, salt: '0x' + '01'.repeat(32) }
const NOW = new Date().toISOString()
const submitted = linkedCollection({ name: 'Brand X Hats' })
// Submitted for review and never approved: no Catalyst entity yet.
const items = [0, 1].map(index => linkedItem(submitted, index, { is_published: true, catalyst_content_hash: null }))
const pendingCuration = (itemId: string) => ({
  id: `item-curation-${itemId}`,
  item_id: itemId,
  status: 'pending' as const,
  content_hash: null as string | null,
  is_mapping_complete: true,
  created_at: NOW,
  updated_at: NOW
})

/**
 * Each item's curated content hash, computed in the page by the app's own function: the fake must hold exactly
 * what the creator submitted, or the approval refuses to put a root on chain.
 */
async function curatedHashes(a: App): Promise<string[]> {
  const rows = JSON.stringify({ collection: submitted, items })
  const hashes = await a.page.evaluate(`(async () => {
    const { collection, items } = ${rows};
    const [{ fromRemoteCollection, fromRemoteItem }, { computeLinkedItemContentHash }] = await Promise.all([
      import('/src/data/lib/creators/builder/mappers.ts'),
      import('/src/data/lib/creators/catalystEntity.ts')
    ]);
    return items.map(item => computeLinkedItemContentHash(fromRemoteCollection(collection), fromRemoteItem(item)));
  })()`)
  return hashes as string[]
}

describe('linked collection approval', () => {
  it('puts the merkle root on chain, then deploys every item with its proof', async () => {
    app = await launchApp({
      path: '/',
      builder: {
        collections: [submitted],
        items,
        thirdParties: [thirdParty({ managers: [CREATOR] })],
        itemCurations: items.map(item => pendingCuration(item.id)),
        curations: [
          {
            id: `curation-${submitted.id}`,
            collection_id: submitted.id,
            status: 'pending',
            assignee: TEST_ADDRESS,
            created_at: NOW,
            updated_at: NOW
          }
        ],
        committee: [TEST_ADDRESS]
      }
    })
    const { page } = app

    const hashes = await curatedHashes(app)
    items.forEach((item, index) => {
      app!.builder.items.get(item.id)!.local_content_hash = hashes[index]
      app!.builder.itemCurations.get(item.id)!.content_hash = hashes[index]
    })
    app.builder.approvalData.set(submitted.id, {
      cheque: CHEQUE,
      root: null,
      content_hashes: Object.fromEntries(items.map((item, index) => [item.id, hashes[index]])),
      chequeWasConsumed: false
    })

    await page.goto(new URL(`/create/collections/editor?collection=${submitted.id}&reviewing=true`, page.url()).href)
    await waitForTestId(page, 'review-bar', 30_000)
    // Approval waits until the curator has looked at every item.
    await clickTestId(page, 'review-linked-next')
    await waitForEnabled(page, 'review-action-approve', 30_000)
    await clickTestId(page, 'review-action-approve')

    await waitForTestId(page, 'approval-step-root', 30_000)
    await clickTestId(page, 'approval-root')
    await waitForTestId(page, 'approval-deploy', 60_000)
    const rootTx = app.transactions.find(tx => tx.fn === 'reviewThirdPartyWithRoot')
    expect(rootTx?.args?.thirdPartyId).toBe(THIRD_PARTY_ID)
    expect(rootTx?.args?.root).not.toBe(zeroHash)
    // The root transaction spends the creator's cheque: its two slots, under the salt they signed.
    const slots = rootTx?.args?.slots as Array<{ qty: bigint; salt: string }>
    expect(slots.map(slot => [Number(slot.qty), slot.salt])).toEqual([[CHEQUE.qty, CHEQUE.salt]])

    await clickTestId(page, 'approval-deploy')
    await waitForTestId(page, 'success-modal-modal', 60_000)
    expect(app.writes.filter(write => write === 'POST /content/entities')).toHaveLength(items.length)
    expect(items.map(item => app!.builder.itemCurations.get(item.id)?.status)).toEqual(['approved', 'approved'])
    await app.waitForEvent('bd_linked_collection_approved', { collection_id: submitted.id, items: items.length })
  })
})

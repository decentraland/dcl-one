import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const BOB = '0x' + 'b0'.repeat(20)
const live = publishedCollection({ name: 'Neon Streetwear' })
const bandana = publishedItem(live, 0)

describe('transfer ownership', () => {
  it('hands the collection to a new owner after confirming its name', async () => {
    app = await launchApp({ path: `/collections/${live.id}`, builder: { collections: [live], items: [bandana] } })
    const { page } = app
    await waitForTestId(page, 'item-row')
    await clickTestId(page, 'collection-actions')
    await clickTestId(page, 'transfer-ownership')
    await waitForTestId(page, 'transfer-ownership-modal')

    await page.type(byTestId('new-owner-input'), BOB)
    await waitForTestId(page, 'new-owner-selected')
    await page.type(byTestId('transfer-confirmation-input'), 'neon streetwear')
    await waitForEnabled(page, 'transfer-continue')
    await clickTestId(page, 'transfer-continue')
    await waitForTestId(page, 'transfer-confirm-owner')
    await clickTestId(page, 'transfer-submit')

    await waitForTestId(page, 'success-modal-modal', 30_000)
    const sent = app.transactions.find(tx => tx.fn === 'transferCreatorship')
    expect(sent?.to).toBe(live.contract_address)
    expect(String(sent?.args?.creator).toLowerCase()).toBe(BOB)
    await app.waitForEvent('Transfer collection ownership', { collectionId: live.id })
    // The server learns the new owner from the chain.
    expect(app.writes).toContain(`POST /v1/collections/${live.id}/owner/sync`)

    // Back on the list, the collection belongs to someone else now.
    await clickTestId(page, 'success-modal-done')
    await waitForTestId(page, 'collections-empty')
    expect(new URL(page.url()).pathname).toBe('/collections')
  })
})

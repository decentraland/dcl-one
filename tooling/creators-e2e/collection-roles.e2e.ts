import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const ALICE = '0x' + 'a1'.repeat(20)
const CAROL = '0x' + 'ca'.repeat(20)
const live = publishedCollection({ name: 'Neon Streetwear', minters: [CAROL] })
const bandana = publishedItem(live, 0)

const roleAddresses = (a: App) =>
  a.page.$$eval(byTestId('role-row'), rows => rows.map(row => row.getAttribute('data-address')))

async function openSenders(a: App) {
  await clickTestId(a.page, 'collection-actions')
  await clickTestId(a.page, 'manage-senders')
  await waitForTestId(a.page, 'manage-roles-modal')
}

describe('collection roles', () => {
  it('replaces a sender in one transaction and remembers the new list', async () => {
    app = await launchApp({ path: `/collections/${live.id}`, builder: { collections: [live], items: [bandana] } })
    const { page } = app
    await waitForTestId(page, 'item-row')
    await openSenders(app)
    expect(await roleAddresses(app)).toEqual([CAROL])

    await clickTestId(page, 'role-1-clear')
    await page.waitForFunction(() => !document.querySelector('[data-testid="role-row"]'))
    await page.type(byTestId('role-new-input'), ALICE)
    await page.waitForSelector('[data-testid="role-row"]')
    await waitForEnabled(page, 'role-save')
    await clickTestId(page, 'role-save')

    await waitForTestId(page, 'success-modal-modal', 30_000)
    const sent = app.transactions.find(tx => tx.fn === 'setMinters')
    expect(sent?.to).toBe(live.contract_address)
    // Removals first, then additions, in a single call.
    expect((sent?.args?.minters as string[]).map(a => a.toLowerCase())).toEqual([CAROL, ALICE])
    expect(sent?.args?.[1]).toEqual([false, true])
    await app.waitForEvent('Set minters', { collectionId: live.id })

    await clickTestId(page, 'success-modal-done')
    await openSenders(app)
    expect(await roleAddresses(app)).toEqual([ALICE])
  })
})

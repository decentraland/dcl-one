import { afterEach, describe, expect, it } from 'vitest'
import { LINKED_CONTRACT, THIRD_PARTY_ID, linkedCollection, linkedItem, thirdParty } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, check, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

describe('linked collections', () => {
  it('shows a published linked collection to a manager of its third party', async () => {
    const published = linkedCollection()
    const item = linkedItem(published, 0, { is_published: true, catalyst_content_hash: 'bafylinked0' })
    app = await launchApp({
      path: `/collections/${published.id}`,
      builder: { collections: [published], items: [item], thirdParties: [thirdParty()] }
    })
    const { page } = app
    await waitForTestId(page, 'linked-item-row')
    expect(await page.$eval(byTestId('collection-status'), el => el.getAttribute('data-status'))).toBe('linked')
    expect(await page.$eval(byTestId('linked-contract-address'), el => el.textContent?.toLowerCase())).toContain(
      LINKED_CONTRACT.slice(0, 6)
    )
    await app.waitForEvent('View linked collection', { collectionId: published.id, item_count: 1, network: 'amoy' })
  })

  it('submits mapped items for review with a signed slots cheque', async () => {
    const draft = linkedCollection({ is_published: false })
    const items = [linkedItem(draft, 0), linkedItem(draft, 1)]
    app = await launchApp({
      path: `/collections/${draft.id}`,
      builder: { collections: [draft], items, thirdParties: [thirdParty()], slots: { [THIRD_PARTY_ID]: 10 } }
    })
    const { page } = app
    await waitForTestId(page, 'linked-item-row')

    await clickTestId(page, 'linked-select-all')
    await waitForEnabled(page, 'publish-linked-items')
    await clickTestId(page, 'publish-linked-items')
    await waitForTestId(page, 'publish-linked-modal')

    await waitForTestId(page, 'publish-name-step')
    await page.waitForSelector(byTestId('publish-name-loading'), { hidden: true })
    await page.type(byTestId('publish-email-input'), 'jane.doe@example.com')
    await check(page, 'publish-name-accept')
    await clickTestId(page, 'publish-name-confirm')

    await waitForTestId(page, 'publish-linked-items-step')
    await check(page, 'publish-linked-items-accept')
    await clickTestId(page, 'publish-linked-items-confirm')

    // Enough slots are left, so the payment step only asks to accept the terms.
    await waitForTestId(page, 'publish-linked-payment-step', 30_000)
    await check(page, 'publish-linked-terms-accept')
    await waitForEnabled(page, 'publish-linked-submit', 30_000)
    await clickTestId(page, 'publish-linked-submit')

    await waitForTestId(page, 'publish-success-modal', 60_000)
    expect(app.transactions).toHaveLength(0)
    expect(items.map(item => app!.builder.itemCurations.get(item.id)?.status)).toEqual(['pending', 'pending'])
    expect(app.builder.slots[THIRD_PARTY_ID]).toBe(8)
    expect(app.writes).toContain(`POST /v1/collections/${draft.id}/tos`)
    await app.waitForEvent('Publish TP Items', { collectionId: draft.id, items: 2 })
  })
})

import { keccak256, parseEther, toHex } from 'viem'
import { afterEach, describe, expect, it } from 'vitest'
import { collection, upperBody, wearable } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, check, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'
import { decodeTransaction } from './helpers/rpc'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

// A draft that was saved once: the server has derived its salt and contract address.
const draft = collection({
  name: 'Summer Drop',
  salt: keccak256(toHex('summer-drop')),
  contract_address: '0x' + 'c0'.repeat(20)
})
const items = [
  wearable({ collection_id: draft.id, name: 'Red Bandana', rarity: 'epic' }),
  upperBody({ collection_id: draft.id, rarity: 'epic' })
]

const statusOf = (a: App) => a.page.$eval(byTestId('collection-status'), el => el.getAttribute('data-status'))

/** Walks the wizard from the detail page's publish button to the success modal, paying with `method`. */
async function publishThroughWizard({ page }: App, method: 'mana' | 'credits' = 'mana') {
  await clickTestId(page, 'publish-collection')

  await waitForTestId(page, 'publish-name-step', 30_000)
  await page.waitForSelector(byTestId('publish-name-loading'), { hidden: true })
  await page.type(byTestId('publish-email-input'), 'jane.doe@example.com')
  await check(page, 'publish-name-accept')
  await clickTestId(page, 'publish-name-confirm')

  await waitForTestId(page, 'publish-items-step')
  await check(page, 'publish-items-accept')
  await clickTestId(page, 'publish-items-confirm')

  await waitForTestId(page, 'publish-payment-step')
  await page.waitForSelector(`[data-testid="payment-method-${method}"][data-selected]`)
  await check(page, 'publish-terms-accept')
  await waitForEnabled(page, 'publish-submit')
  await clickTestId(page, 'publish-submit')

  await waitForTestId(page, 'publish-success-modal', 30_000)
}

/** CreditsManager on Amoy: what a credits payment calls, with the publish as its external call. */
const CREDITS_MANAGER = '0x8052a560e6e6ac86eeb7e711a4497f639b322fb3'
const COLLECTION_MANAGER = '0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849'
const CREATE_COLLECTION_SELECTOR = '0x35a629c2'

describe('publish a collection', () => {
  it('pays the fee in MANA, publishes on chain and opens the first review', async () => {
    app = await launchApp({
      path: `/collections/${draft.id}`,
      builder: { collections: [draft], items },
      chain: { manaBalanceWei: parseEther('100').toString() }
    })
    const { page } = app
    await waitForEnabled(page, 'publish-collection', 30_000)
    await clickTestId(page, 'publish-collection')

    await waitForTestId(page, 'publish-name-step', 30_000)
    await page.waitForSelector(byTestId('publish-name-loading'), { hidden: true })
    await page.type(byTestId('publish-email-input'), 'jane.doe@example.com')
    await check(page, 'publish-name-accept')
    await clickTestId(page, 'publish-name-confirm')

    await waitForTestId(page, 'publish-items-step')
    expect(await page.$$eval(byTestId('publish-item-row'), rows => rows.length)).toBe(2)
    await check(page, 'publish-items-accept')
    await clickTestId(page, 'publish-items-confirm')

    await waitForTestId(page, 'publish-payment-step')
    await page.waitForSelector('[data-testid="payment-method-mana"][data-selected]')
    await check(page, 'publish-terms-accept')
    await waitForEnabled(page, 'publish-submit')
    await clickTestId(page, 'publish-submit')

    await waitForTestId(page, 'publish-success-modal', 30_000)
    await app.waitForEvent('Publish collection', { collectionId: draft.id, usedCredits: false, item_count: 2 })
    const publish = app.transactions.find(tx => tx.fn === 'createCollection')
    // The salt the server derived on the save right before publishing.
    expect(publish?.args?.salt).toBe(app.builder.collections.get(draft.id)?.salt)
    expect(publish?.args?.name).toBe('Summer Drop')
    expect(publish?.args?.items).toHaveLength(2)
    expect(app.writes).toContain(`POST /v1/collections/${draft.id}/tos`)

    // Once mined, the server learns about the publication and the first review request opens.
    await clickTestId(page, 'publish-success-done')
    await page.waitForSelector('[data-testid="collection-status"][data-status="under_review"]', { timeout: 30_000 })
    const published = app.builder.collections.get(draft.id)!
    expect(published).toMatchObject({ is_published: true, lock: expect.any(String) as string })
    expect(published.forum_link).toBeTruthy()
    expect(app.builder.curations.get(draft.id)?.status).toBe('pending')
    expect(await statusOf(app)).toBe('under_review')
  })

  it('pays the fee with credits, which publish the collection through the credits manager', async () => {
    app = await launchApp({
      path: `/collections/${draft.id}`,
      builder: { collections: [draft], items },
      flags: { 'builder-shop-credits-for-collections-fee': true },
      credits: { credits: 100, balanceCents: 1000 }
    })
    const { page } = app
    await waitForEnabled(page, 'publish-collection', 30_000)
    // Two items at 1 USD (10 credits) each.
    await publishThroughWizard(app, 'credits')

    await app.waitForEvent('Publish collection', { collectionId: draft.id, usedCredits: true, creditsAmount: 20 })
    expect(app.writes).toContain('POST /credits/authorize-publication')
    // The credits manager runs the publish itself: no createCollection is sent from the wallet.
    expect(app.transactions.map(tx => tx.fn)).toEqual(['useCredits'])
    const [payment] = app.transactions
    expect(payment.to).toBe(CREDITS_MANAGER)
    // Its external call is the publish itself: createCollection on the collection manager.
    const { externalCall } = payment.args!.args as { externalCall: { target: string; selector: string; data: string } }
    expect(externalCall.target.toLowerCase()).toBe(COLLECTION_MANAGER)
    expect(externalCall.selector).toBe(CREATE_COLLECTION_SELECTOR)
    // The call's arguments travel apart from its selector.
    const publish = decodeTransaction({ ...payment, data: externalCall.selector + externalCall.data.slice(2) })
    expect(publish.args?.name).toBe('Summer Drop')

    await clickTestId(page, 'publish-success-done')
    await page.waitForSelector('[data-testid="collection-status"][data-status="under_review"]', { timeout: 30_000 })
  })

  it('publishes again, without edits, after the publish transaction reverted', async () => {
    app = await launchApp({
      path: `/collections/${draft.id}`,
      builder: { collections: [draft], items },
      chain: { manaBalanceWei: parseEther('100').toString(), revertOnce: ['createCollection'] }
    })
    const { page } = app
    await waitForEnabled(page, 'publish-collection', 30_000)
    await publishThroughWizard(app)
    await clickTestId(page, 'publish-success-done')

    // The revert leaves the collection locked but publishable again.
    await app.waitForEvent('Publish collection reverted', { collectionId: draft.id, usedCredits: false })
    await page.waitForSelector('[data-testid="publish-collection"][data-retry="true"]:not([aria-disabled])', {
      timeout: 30_000
    })
    expect(await statusOf(app)).toBe('publishing')
    expect(app.builder.collections.get(draft.id)?.is_published).toBe(false)

    await clickTestId(page, 'publish-collection')
    await waitForTestId(page, 'publish-name-step', 30_000)
    expect(await page.$eval(byTestId('publish-name-input'), el => (el as HTMLInputElement).disabled)).toBe(true)
    await clickTestId(page, 'publish-cancel')
    await publishThroughWizard(app)
    await app.waitForEvent('Publish collection', { collectionId: draft.id, isRetry: true })

    const sent = app.transactions.filter(tx => tx.fn === 'createCollection')
    expect(sent).toHaveLength(2)
    expect(sent[1].args?.salt).toBe(sent[0].args?.salt)

    await clickTestId(page, 'publish-success-done')
    await page.waitForSelector('[data-testid="collection-status"][data-status="under_review"]', { timeout: 30_000 })
    expect(app.builder.collections.get(draft.id)?.is_published).toBe(true)
  })

  it('is blocked by an invalid item while the validation gate is on', async () => {
    app = await launchApp({
      path: `/collections/${draft.id}`,
      builder: { collections: [draft], items: [items[0], upperBody({ collection_id: draft.id }, { invalid: true })] },
      flags: { 'builder-block-publish-on-validation-errors': true }
    })
    await app.page.waitForSelector(
      '[data-testid="publish-collection"][data-blocker="has_invalid_items"][aria-disabled="true"]',
      { timeout: 30_000 }
    )
  })
})

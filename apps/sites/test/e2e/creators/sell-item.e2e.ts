import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'
import { MARKETPLACE_V3 } from './helpers/market'
import { TEST_ADDRESS } from './helpers/session'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const live = publishedCollection({ name: 'Neon Streetwear' })
const bandana = publishedItem(live, 0, { name: 'Neon Bandana' })
const flags = { 'dapps-offchain-public-item-orders': true, 'builder-credits-primary-listings': true }

const saleStatus = (a: App) => a.page.$eval(byTestId('item-sale-status'), el => el.getAttribute('data-status'))
const waitForSaleStatus = (a: App, status: string) =>
  a.page.waitForSelector(`[data-testid="item-sale-status"][data-status="${status}"]`, { timeout: 30_000 })

/** The credits price a stored trade asks for: USD-pegged MANA, 1 credit = 0.1 USD. */
const creditsOf = (trade: Record<string, unknown>) =>
  Number(BigInt((trade.received as Array<{ amount: string }>)[0].amount) / 10n ** 17n)

describe('sell an item', () => {
  it('enables sales, lists in credits, changes the price and takes it off sale', async () => {
    app = await launchApp({
      path: `/collections/${live.id}`,
      builder: { collections: [live], items: [bandana] },
      flags
    })
    const { page } = app
    await waitForSaleStatus(app, 'not_on_sale')

    // First sale: the marketplace becomes a minter of the collection.
    await clickTestId(page, 'item-sale-status')
    await clickTestId(page, 'enable-sales-confirm')
    await waitForTestId(page, 'sell-item-form', 30_000)
    const enabled = app.transactions.find(tx => tx.fn === 'setMinters')
    expect((enabled?.args?.minters as string[]).map(a => a.toLowerCase())).toEqual([MARKETPLACE_V3])

    await page.type(byTestId('sell-price-input'), '25')
    await waitForEnabled(page, 'sell-submit')
    await clickTestId(page, 'sell-submit')
    await waitForTestId(page, 'success-modal-modal', 30_000)
    const [listed] = [...app.market.trades.values()]
    expect(creditsOf(listed)).toBe(25)
    expect(listed.signer).toBe(TEST_ADDRESS)
    expect(listed.signature).toMatch(/^0x[0-9a-f]{130}$/)
    await app.waitForEvent('Set price and beneficiary', { collectionId: live.id, itemId: bandana.id })
    await clickTestId(page, 'success-modal-done')
    await waitForSaleStatus(app, 'on_sale')

    // A new price cancels the old order on chain and signs a new one.
    await clickTestId(page, 'item-row-edit-price')
    await waitForTestId(page, 'update-price-form')
    await page.click(byTestId('update-price-input'), { count: 3 })
    await page.keyboard.type('30')
    await waitForEnabled(page, 'update-price-submit')
    await clickTestId(page, 'update-price-submit')
    await waitForTestId(page, 'success-modal-modal', 30_000)
    expect(app.transactions.filter(tx => tx.fn === 'cancelSignature').map(tx => tx.to)).toEqual([MARKETPLACE_V3])
    const [repriced] = [...app.market.trades.values()]
    expect(creditsOf(repriced)).toBe(30)
    await clickTestId(page, 'success-modal-done')

    await clickTestId(page, 'item-actions')
    await clickTestId(page, 'item-remove-from-sale')
    await clickTestId(page, 'remove-listing-confirm')
    await waitForTestId(page, 'success-modal-modal', 30_000)
    expect(app.market.trades.size).toBe(0)
    await app.waitForEvent('Remove item listing', { collectionId: live.id, itemId: bandana.id })
    await clickTestId(page, 'success-modal-done')
    await page.waitForFunction(
      () => document.querySelector('[data-testid="item-sale-status"]')?.getAttribute('data-status') !== 'on_sale'
    )
    expect(await saleStatus(app)).toBe('not_on_sale')
  })
})

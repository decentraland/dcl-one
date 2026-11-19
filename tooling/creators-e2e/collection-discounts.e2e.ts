import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'
import { COUPON_MANAGER_V3, MARKETPLACE_V3 } from './helpers/market'
import { TEST_ADDRESS } from './helpers/session'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const live = publishedCollection({ name: 'Neon Streetwear', minters: [MARKETPLACE_V3] })
const bandana = publishedItem(live, 0, { name: 'Neon Bandana' })
const contractAddress = live.contract_address!
// Listed for 25 credits through the current marketplace: the kind of listing a discount reaches.
const market = {
  items: [
    {
      itemId: '0',
      contractAddress,
      price: (25n * 10n ** 17n).toString(),
      isOnSale: true,
      tradeId: 'trade-seeded',
      tradeContractAddress: MARKETPLACE_V3
    }
  ],
  catalog: [{ itemId: '0', contractAddress, priceCredits: 25, source: 'native' as const, tradeId: 'trade-seeded' }]
}
const flags = {
  'dapps-offchain-public-item-orders': true,
  'dapps-shop-creator-sales': true,
  'builder-collection-discounts': true
}

describe('collection discounts', () => {
  it('creates a signed discount, shows it on the page and ends it on chain', async () => {
    app = await launchApp({
      path: `/collections/${live.id}`,
      builder: { collections: [live], items: [bandana] },
      market,
      flags
    })
    const { page } = app
    await waitForEnabled(page, 'create-discount', 30_000)
    await clickTestId(page, 'create-discount')
    await waitForTestId(page, 'creator-sale-modal')

    await clickTestId(page, 'creator-sale-pct-30')
    await clickTestId(page, 'creator-sale-continue')
    await waitForTestId(page, 'creator-sale-review')
    await clickTestId(page, 'creator-sale-submit')
    await page.waitForSelector('[data-testid^="creator-sale-success"]', { timeout: 30_000 })

    expect(app.market.coupons).toHaveLength(1)
    const [coupon] = app.market.coupons
    // 30% in parts per million, over this collection only, signed by the creator.
    expect(coupon).toMatchObject({ signer: TEST_ADDRESS, discount: 300_000, collections: [contractAddress] })
    expect(app.transactions).toHaveLength(0)
    await app.waitForEvent('Create collection discount', { collectionId: live.id, discountPct: 30 })

    await page.keyboard.press('Escape')
    await page.goto(page.url(), { waitUntil: 'networkidle2' })
    await waitForTestId(page, 'collection-discounts')
    expect(await page.$$eval(byTestId('collection-discount'), chips => chips.length)).toBe(1)

    // Ending rebuilds the coupon from the stored row and cancels its signature on the manager it was signed for.
    await clickTestId(page, 'end-discount')
    await clickTestId(page, 'end-discount-confirm')
    await waitForTestId(page, 'success-modal-modal', 30_000)
    const ended = app.transactions.find(tx => tx.fn === 'cancelSignature')
    expect(ended?.to).toBe(COUPON_MANAGER_V3)
    await app.waitForEvent('End collection discount', { collectionId: live.id })
    await clickTestId(page, 'success-modal-done')
    await page.waitForFunction(() => !document.querySelector('[data-testid="collection-discount"]'), {
      timeout: 15_000
    })
  })
})

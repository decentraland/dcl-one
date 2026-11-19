import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, check, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const SEPOLIA = 11155111
const live = publishedCollection({ name: 'Neon Streetwear' })
const bandana = publishedItem(live, 0, { name: 'Neon Bandana', total_supply: 3 })
const ALICE = '0x' + 'a1'.repeat(20)

describe('meta-transactions', () => {
  it('relays a Polygon write for a wallet on Ethereum instead of sending it from the wallet', async () => {
    app = await launchApp({
      path: `/collections/${live.id}`,
      builder: { collections: [live], items: [bandana] },
      chainId: SEPOLIA
    })
    const { page } = app
    await waitForEnabled(page, 'send-items')
    await clickTestId(page, 'send-items')
    await waitForTestId(page, 'send-items-modal')
    await page.type(byTestId('transfer-1-recipient-input'), ALICE)
    await check(page, `transfer-1-item-${bandana.id}-check`)
    await waitForEnabled(page, 'send-continue')
    await clickTestId(page, 'send-continue')
    await clickTestId(page, 'send-submit')

    await waitForTestId(page, 'success-modal-modal', 30_000)
    // The wallet only signed: the relayer sent the call to the collection on Polygon.
    expect(app.writes).toContain('POST /v1/transactions')
    expect(app.transactions).toHaveLength(1)
    const [sent] = app.transactions
    expect(sent).toMatchObject({ fn: 'issueTokens', to: live.contract_address, relayed: true })
    expect((sent.args?.beneficiaries as string[]).map(a => a.toLowerCase())).toEqual([ALICE])
    await app.waitForEvent('Mint items', { collectionId: live.id, copies: 1 })
  })
})

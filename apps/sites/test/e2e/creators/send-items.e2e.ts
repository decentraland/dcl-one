import { type Page } from 'puppeteer'
import { afterEach, describe, expect, it } from 'vitest'
import { publishedCollection, publishedItem } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, check, clickTestId, waitForEnabled, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const live = publishedCollection({ name: 'Neon Streetwear' })
const bandana = publishedItem(live, 0, { name: 'Neon Bandana', total_supply: 3 })
const ALICE = '0x' + 'a1'.repeat(20)
const BOB = '0x' + 'b0'.repeat(20)

/** Pastes text into an input the way the browser does, so the app's paste handler sees the clipboard data. */
async function paste(page: Page, testId: string, text: string) {
  await page.$eval(
    byTestId(testId),
    (el, value) => {
      const data = new DataTransfer()
      data.setData('text/plain', value)
      el.dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }))
    },
    text
  )
}

describe('send items', () => {
  it('sends a copy to each pasted recipient in one transaction', async () => {
    app = await launchApp({ path: `/collections/${live.id}`, builder: { collections: [live], items: [bandana] } })
    const { page } = app
    await waitForEnabled(page, 'send-items')
    await clickTestId(page, 'send-items')
    await waitForTestId(page, 'send-items-modal')

    // A pasted list keeps the valid, new addresses and reports the malformed and repeated ones.
    await paste(page, 'transfer-1-recipient-input', [ALICE, BOB, '0x1234', ALICE].join('\n'))
    await app.waitForEvent('Paste recipients', { collectionId: live.id, added: 2, invalid: 1, repeated: 1 })

    await check(page, `transfer-1-item-${bandana.id}-check`)
    await waitForEnabled(page, 'send-continue')
    await clickTestId(page, 'send-continue')
    await waitForTestId(page, 'send-summary')
    await waitForTestId(page, `send-summary-${ALICE}`)
    await waitForTestId(page, `send-summary-${BOB}`)
    await clickTestId(page, 'send-submit')

    await waitForTestId(page, 'success-modal-modal', 30_000)
    const issued = app.transactions.find(tx => tx.fn === 'issueTokens')
    expect(issued?.to).toBe(live.contract_address)
    expect((issued?.args?.beneficiaries as string[]).map(a => a.toLowerCase()).sort()).toEqual([ALICE, BOB])
    expect((issued?.args?.itemIds as Array<{ toString(): string }>).map(String)).toEqual(['0', '0'])
    await app.waitForEvent('Mint items', { collectionId: live.id, copies: 2, recipient_count: 2 })

    await clickTestId(page, 'success-modal-done')
    await page.waitForFunction(
      () => document.querySelector('[data-testid="item-row-sales"]')?.textContent?.trim().startsWith('5/'),
      { timeout: 10_000 }
    )
  })
})

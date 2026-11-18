import { afterEach, describe, expect, it } from 'vitest'
import { collection } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const existing = collection({ name: 'Neon Streetwear' })

async function submitName(a: App, name: string) {
  await clickTestId(a.page, 'new-collection')
  await waitForTestId(a.page, 'collection-name-modal')
  await a.page.type(byTestId('collection-name-input'), name)
  await clickTestId(a.page, 'collection-name-submit')
}

describe('create a collection', () => {
  it('names a new collection and lands on its empty draft page', async () => {
    app = await launchApp({ path: '/collections', builder: { collections: [existing] } })
    await waitForTestId(app.page, 'collections-grid')

    await submitName(app, 'Summer Drop')

    await waitForTestId(app.page, 'collection-empty')
    const id = new URL(app.page.url()).pathname.split('/').pop()
    expect(id).toMatch(/^[0-9a-f-]{36}$/)
    expect(await app.page.$eval(byTestId('collection-name'), el => el.textContent?.trim())).toBe('Summer Drop')
    expect(await app.page.$eval(byTestId('collection-status'), el => el.getAttribute('data-status'))).toBe('draft')
    expect(app.builder.collections.get(id!)?.name).toBe('Summer Drop')
    await app.waitForEvent('Save collection', { collectionId: id, item_count: 0 })
  })

  it('keeps the modal open when the name is taken', async () => {
    app = await launchApp({ path: '/collections', builder: { collections: [existing] } })
    await waitForTestId(app.page, 'collections-grid')

    await submitName(app, 'neon streetwear')

    await waitForTestId(app.page, 'collection-name-error')
    expect(await app.page.$(byTestId('collection-name-modal'))).not.toBeNull()
    expect(app.builder.collections.size).toBe(1)
  })

  it('offers no creation on a phone', async () => {
    app = await launchApp({ path: '/collections', builder: { collections: [existing] }, viewport: 'mobile' })
    await waitForTestId(app.page, 'collections-grid')
    const button = await app.page.$(byTestId('new-collection'))
    expect(await button?.isVisible()).toBe(false)
  })
})

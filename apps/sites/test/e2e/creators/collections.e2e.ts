import { afterEach, describe, expect, it } from 'vitest'
import { collection, publishedCollection } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const seed = {
  collections: [
    collection({ name: 'Summer Drafts', updated_at: '2026-11-03T00:00:00.000Z' }),
    collection({ name: 'Winter Drafts', updated_at: '2026-11-02T00:00:00.000Z' }),
    publishedCollection({ name: 'Neon Streetwear', updated_at: '2026-11-01T00:00:00.000Z' })
  ]
}

/** Each listed card as `name: status`. */
const cards = (a: App) =>
  a.page.$$eval(byTestId('collection-card'), els =>
    els.map(el => {
      const status = el.querySelector('[data-testid="collection-status"]')?.getAttribute('data-status')
      const name = el.querySelector('[data-testid="collection-card-name"]')?.textContent
      return `${name?.trim()}: ${status}`
    })
  )

describe('my collections', () => {
  it('lists the creator collections and filters them by status and name', async () => {
    app = await launchApp({ path: '/collections', builder: seed })
    const { page } = app

    await waitForTestId(page, 'collections-grid')
    expect(await cards(app)).toEqual(['Summer Drafts: draft', 'Winter Drafts: draft', 'Neon Streetwear: published'])

    await clickTestId(page, 'status-filter-published')
    await page.waitForFunction(() => document.querySelectorAll('[data-testid="collection-card"]').length === 1)
    expect(await cards(app)).toEqual(['Neon Streetwear: published'])
    expect(new URL(page.url()).searchParams.get('status')).toBe('published')

    await clickTestId(page, 'status-filter-all')
    await page.type(byTestId('collections-search'), 'winter')
    await page.waitForFunction(() => document.querySelectorAll('[data-testid="collection-card"]').length === 1)
    expect(await cards(app)).toEqual(['Winter Drafts: draft'])
  })

  it('counts a Linked chip pick', async () => {
    app = await launchApp({ path: '/collections', builder: seed })
    await waitForTestId(app.page, 'collections-grid')
    await clickTestId(app.page, 'status-filter-linked')
    await app.waitForEvent('Filter linked collections', { count: 0 })
  })

  it('fits a phone screen', async () => {
    app = await launchApp({ path: '/collections', builder: seed, viewport: 'mobile' })
    await waitForTestId(app.page, 'collections-grid')
    expect(await cards(app)).toHaveLength(3)
    expect(
      await app.page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth)
    ).toBe(false)
  })
})

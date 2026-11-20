import { afterEach, describe, expect, it } from 'vitest'
import { collection } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { isShown, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const builder = { collections: [collection({ name: 'Summer Drop' })] }

describe('navigation', () => {
  it('opens a direct link to a creators page with the section tabs', async () => {
    app = await launchApp({ path: '/collections', builder })
    await waitForTestId(app.page, 'collections-grid')
    expect(await isShown(app.page, 'subnav')).toBe(true)
    expect(new URL(app.page.url()).pathname).toBe('/create/collections')
  })
})

describe('maintenance', () => {
  it('replaces every page with the maintenance notice', async () => {
    app = await launchApp({ path: '/collections', builder, flags: { 'builder-maintenance': true } })
    await waitForTestId(app.page, 'maintenance-page')
    expect(await app.page.$('[data-testid="collections-grid"]')).toBeNull()
  })
})

describe('not found', () => {
  it('treats the live preview as not found while its flag is off', async () => {
    app = await launchApp({ path: '/live-preview', builder })
    await app.page.waitForSelector('main[data-status="404"]', { state: 'attached' })
  })

  it('tells an editor link without a collection that the collection was not found', async () => {
    app = await launchApp({ path: '/collections/editor', builder })
    await waitForTestId(app.page, 'collection-not-found')
  })
})

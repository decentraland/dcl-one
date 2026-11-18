import { afterEach, describe, expect, it } from 'vitest'
import { collection } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, isShown, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const builder = { collections: [collection({ name: 'Summer Drop' })] }

describe('navigation', () => {
  it('gives a visitor the section tabs on the overview', async () => {
    app = await launchApp({ path: '/', builder, signedOut: true })
    await waitForTestId(app.page, 'overview-page')
    expect(await isShown(app.page, 'subnav')).toBe(true)
  })

  it('opens a direct link to an inner page with its tabs', async () => {
    app = await launchApp({ path: '/collections', builder })
    await waitForTestId(app.page, 'collections-grid')
    expect(await isShown(app.page, 'subnav')).toBe(true)
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
  it('shows the not-found page for an unknown path and links home', async () => {
    app = await launchApp({ path: '/nothing-here', builder })
    await waitForTestId(app.page, 'not-found-page')
    await app.waitForEvent('Not found page')
    await clickTestId(app.page, 'not-found-home')
    await waitForTestId(app.page, 'overview-page')
  })

  it('treats the live preview as not found while its flag is off', async () => {
    app = await launchApp({ path: '/live-preview', builder })
    await waitForTestId(app.page, 'not-found-page')
  })
})

describe('served under /create', () => {
  it('routes the same pages below the /create base path', async () => {
    app = await launchApp({ path: '/create/collections', builder })
    await waitForTestId(app.page, 'collections-grid')
    expect(new URL(app.page.url()).pathname).toBe('/create/collections')
    expect(app.pageViews()).toContain('collections')
  })
})

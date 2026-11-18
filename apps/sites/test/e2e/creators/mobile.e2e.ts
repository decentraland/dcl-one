import { afterEach, describe, expect, it } from 'vitest'
import { collection, publishedCollection, publishedItem, wearable } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, isHidden, waitForTestId } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const draft = collection({ name: 'Summer Drop' })
const hat = wearable({ collection_id: draft.id, name: 'Red Bandana' })
const live = publishedCollection({ name: 'Neon Streetwear' })
const liveHat = publishedItem(live, 0, { name: 'Neon Bandana' })
const builder = { collections: [draft, live], items: [hat, liveHat] }

describe('phones view collections without managing them', () => {
  it('hides the draft management actions and skips validation', async () => {
    app = await launchApp({ path: `/collections/${draft.id}`, builder, viewport: 'mobile' })
    const { page } = app
    await waitForTestId(page, 'item-row')

    for (const id of ['publish-collection', 'add-items', 'open-editor']) {
      expect(await isHidden(page, id), id).toBe(true)
    }
    expect(await page.$('[data-testid="validation-spinner"], [data-testid="item-row-validation"]')).toBeNull()

    await clickTestId(page, 'collection-actions')
    await waitForTestId(page, 'preview-items-action')
    // The compact menu leaves deleting out altogether.
    expect(await page.$('[data-testid="delete-collection"]')).toBeNull()
  })

  it('moves the published actions into the menu', async () => {
    app = await launchApp({ path: `/collections/${live.id}`, builder, viewport: 'mobile' })
    const { page } = app
    await waitForTestId(page, 'item-row')
    expect(await isHidden(page, 'send-items')).toBe(true)

    await clickTestId(page, 'collection-actions')
    await waitForTestId(page, 'send-items-action')
  })

  it('turns the editor into a preview strip', async () => {
    app = await launchApp({
      path: `/collections/editor?collection=${draft.id}&item=${hat.id}`,
      builder,
      viewport: 'mobile'
    })
    const { page } = app
    await waitForTestId(page, 'item-editor-mobile')
    await waitForTestId(page, `mobile-strip-item-${hat.id}`)
    expect(await page.$('[data-testid="properties-panel"]')).toBeNull()
  })

  it('hides management between the phone and desktop breakpoints too', async () => {
    app = await launchApp({ path: `/collections/${draft.id}`, builder })
    await app.page.setViewport({ width: 820, height: 1000 })
    await waitForTestId(app.page, 'item-row')
    await app.page.waitForFunction(() => {
      const button = document.querySelector('[data-testid="publish-collection"]')
      return !!button && !button.checkVisibility()
    })
    expect(await isHidden(app.page, 'add-items')).toBe(true)
  })
})

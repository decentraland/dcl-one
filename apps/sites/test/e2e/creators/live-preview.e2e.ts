import { readFileSync } from 'node:fs'
import { afterEach, describe, expect, it } from 'vitest'
import { collection } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, clickTestId, selectOption, waitForEnabled, waitForTestId } from './helpers/dom'
import { waitForProcessedAction } from './helpers/preview'
import { FIXTURE } from './helpers/files'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const draft = collection({ name: 'Summer Drop' })
const PORT = 18080

describe('Blender live preview', () => {
  it('follows the add-on and adds the exported model to a collection', async () => {
    const bridge = {
      port: PORT,
      state: { version: 1, type: 'wearable', name: 'Bandana', category: 'hat' } as Record<string, unknown>,
      model: readFileSync(FIXTURE.hat)
    }
    app = await launchApp({
      path: `/live-preview?bridge=${PORT}`,
      builder: { collections: [draft] },
      flags: { 'builder-blender-live-preview': true },
      bridge,
      // The page keeps polling the bridge, so the network never goes idle.
      waitUntil: 'domcontentloaded'
    })
    const { page } = app
    await page.waitForSelector('[data-testid="live-preview-status"][data-status="connected"]', { timeout: 30_000 })
    await waitForTestId(page, 'live-preview-updated', 30_000)
    await app.waitForEvent('Live preview model', { item_type: 'wearable', category: 'hat' })

    await waitForEnabled(page, 'live-preview-add')
    await clickTestId(page, 'live-preview-add')
    await waitForTestId(page, 'live-preview-pick-collection')
    await selectOption(page, 'live-preview-pick-target', draft.id)
    await clickTestId(page, 'live-preview-pick-confirm')

    // The model and the panel's tuning arrive in the add-items modal; the name is the creator's to give.
    await waitForTestId(page, 'draft-form', 30_000)
    await page.type(byTestId('item-name'), 'Bandana')
    await waitForProcessedAction(app, 'add-items-finish', 30_000)
    await clickTestId(page, 'add-items-finish')

    await page.waitForFunction(
      (id: string) => location.pathname === `/collections/${id}`,
      { timeout: 30_000 },
      draft.id
    )
    const [item] = [...app.builder.items.values()]
    expect(item).toMatchObject({ name: 'Bandana', collection_id: draft.id, data: { category: 'hat' } })
    await app.waitForEvent('Add items', { collectionId: draft.id, origin: 'live_preview' })
  })
})

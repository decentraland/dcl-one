import { type ElementHandle } from 'puppeteer'
import { afterEach, describe, expect, it } from 'vitest'
import {
  collection,
  linkedCollection,
  linkedItem,
  publishedCollection,
  publishedItem,
  thirdParty,
  upperBody,
  wearable
} from './fixtures'
import { launchApp, type App } from './helpers/app'
import { FIXTURE } from './helpers/files'
import { byTestId, clickTestId, selectOption, waitForEditor, waitForTestId, waitForToast } from './helpers/dom'
import { BOOT_SCENE_METRICS } from './helpers/preview'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const draft = collection({ name: 'Summer Drop' })
const hat = wearable({ collection_id: draft.id, name: 'Red Bandana' })
const polo = upperBody({ collection_id: draft.id }, { invalid: true })

const editorPath = (itemId: string) => `/collections/editor?collection=${draft.id}&item=${itemId}`

async function openEditor(itemId: string): Promise<App> {
  const launched = await launchApp({ path: editorPath(itemId), builder: { collections: [draft], items: [hat, polo] } })
  await waitForEditor(launched.page)
  return launched
}

const HAT_METRICS = { triangles: 540, materials: 1, textures: 1, meshes: 1, bodies: 1, entities: 1 }

const metricsText = (a: App) =>
  a.page.$eval(byTestId('properties-panel-metrics'), el => el.textContent ?? '').catch(() => '')

const badgeStatus = (a: App, status: string) =>
  a.page.waitForSelector(`[data-testid="validation-badge"][data-status="${status}"]`, { timeout: 30_000 })

describe('item editor', () => {
  it('saves name, category and hides, and keeps them after a reload', async () => {
    app = await openEditor(hat.id)
    const { page } = app
    await badgeStatus(app, 'pass')

    await page.click(byTestId('properties-panel-name'), { count: 3 })
    await page.keyboard.type('Bandana Deluxe')
    await selectOption(page, 'properties-panel-category', 'helmet')
    await clickTestId(page, 'properties-panel-hides-categories')
    await clickTestId(page, 'properties-panel-hides-categories-option-eyewear')
    await page.keyboard.press('Escape')
    await clickTestId(page, 'properties-panel-save')

    await waitForToast(page)
    await app.waitForEvent('Save item', { itemId: hat.id })
    const saved = app.builder.items.get(hat.id)!
    expect(saved).toMatchObject({ name: 'Bandana Deluxe', data: { category: 'helmet' } })
    expect(saved.data.hides).toContain('eyewear')
    // The item's hides apply to every body shape's model.
    for (const representation of saved.data.representations as Array<{ overrideHides: string[] }>) {
      expect(representation.overrideHides).toContain('eyewear')
    }

    await page.reload({ waitUntil: 'networkidle2' })
    await waitForEditor(page)
    expect(await page.$eval(byTestId('properties-panel-name'), el => (el as HTMLInputElement).value)).toBe(
      'Bandana Deluxe'
    )
    expect(app.pageViews()).toEqual(['item_editor', 'item_editor'])
  })

  it('saves a new thumbnail as soon as it is confirmed, leaving unsaved details pending', async () => {
    app = await openEditor(hat.id)
    const { page } = app
    const before = app.builder.items.get(hat.id)!.contents['thumbnail.png']

    await page.click(byTestId('properties-panel-name'), { count: 3 })
    await page.keyboard.type('Unsaved Name')
    await clickTestId(page, 'properties-panel-thumbnail')
    await waitForTestId(page, 'thumbnail-modal')
    const input = (await page.$(byTestId('thumbnail-file-input'))) as ElementHandle<HTMLInputElement>
    await input.uploadFile(FIXTURE.thumbnail)

    await app.waitForEvent('Save item', { itemId: hat.id })
    const saved = app.builder.items.get(hat.id)!
    expect(saved.contents['thumbnail.png']).not.toBe(before)
    expect(saved.name).toBe(hat.name)
    expect(await page.$eval(byTestId('properties-panel-name'), el => (el as HTMLInputElement).value)).toBe(
      'Unsaved Name'
    )
  })

  it('shows the rule book errors of an invalid model', async () => {
    app = await openEditor(polo.id)
    await badgeStatus(app, 'errors')

    await clickTestId(app.page, 'validation-badge')
    await waitForTestId(app.page, 'validation-badge-modal')
    await app.page.waitForSelector(
      '[data-testid="validation-badge-modal"] [data-severity="error"][data-code="texture-size"]'
    )
    await app.waitForEvent('Item Validation Details Opened', { itemId: polo.id, status: 'errors', trigger: 'editor' })
  })

  it('counts the metrics of a wearable saved without geometry and stores them', async () => {
    const zeroed = wearable({ collection_id: draft.id, name: 'Zeroed Hat', metrics: BOOT_SCENE_METRICS })
    app = await launchApp({ path: editorPath(zeroed.id), builder: { collections: [draft], items: [zeroed] } })
    await waitForEditor(app.page)

    await app.waitForEvent('Item metrics repaired', { itemId: zeroed.id, saved: true })
    expect(await metricsText(app)).toContain('540')
    await expect.poll(() => app!.builder.items.get(zeroed.id)!.metrics).toEqual(HAT_METRICS)
  })

  it("stores counted metrics on a published collection only with the owner's own save", async () => {
    const live = publishedCollection({ name: 'Live Drop' })
    const zeroed = publishedItem(live, 0, { name: 'Zeroed Hat', metrics: BOOT_SCENE_METRICS })
    app = await launchApp({
      path: `/collections/editor?collection=${live.id}&item=${zeroed.id}`,
      builder: { collections: [live], items: [zeroed] },
      flags: { 'dapps-wearable-utility': true }
    })
    const { page } = app
    await waitForEditor(page)

    await app.waitForEvent('Item metrics repaired', { itemId: zeroed.id, saved: false })
    expect(await metricsText(app)).toContain('540')
    expect(app.builder.items.get(zeroed.id)!.metrics).toEqual(BOOT_SCENE_METRICS)

    await page.type(byTestId('properties-panel-utility'), 'Glows in the dark')
    await clickTestId(page, 'properties-panel-save')
    await waitForToast(page)
    expect(app.builder.items.get(zeroed.id)!.metrics).toEqual(HAT_METRICS)
  })

  it('keeps the stored metrics of a linked item already sent for review, whatever its manager saves', async () => {
    const linked = linkedCollection({ name: 'Linked Drop' })
    const zeroed = linkedItem(linked, 0, { metrics: BOOT_SCENE_METRICS })
    const curation = {
      id: `curation-${zeroed.id}`,
      item_id: zeroed.id,
      status: 'approved' as const,
      content_hash: zeroed.local_content_hash ?? null,
      is_mapping_complete: true,
      created_at: new Date(0).toISOString(),
      updated_at: new Date(0).toISOString()
    }
    app = await launchApp({
      path: `/collections/editor?collection=${linked.id}&item=${zeroed.id}`,
      builder: { collections: [linked], items: [zeroed], thirdParties: [thirdParty()], itemCurations: [curation] },
      flags: { 'dapps-wearable-utility': true }
    })
    const { page } = app
    await waitForEditor(page)

    await app.waitForEvent('Item metrics repaired', { itemId: zeroed.id, saved: false })
    expect(await metricsText(app)).toContain('540')

    await page.type(byTestId('properties-panel-utility'), 'Glows in the dark')
    await clickTestId(page, 'properties-panel-save')
    await waitForToast(page)
    expect(app.builder.items.get(zeroed.id)!.metrics).toEqual(BOOT_SCENE_METRICS)
  })
})

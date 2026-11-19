import { type ElementHandle } from 'puppeteer'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { collection } from './fixtures'
import { launchApp, type App, type LaunchOptions } from './helpers/app'
import { byTestId, clickTestId, selectOption, selectOptions, waitForTestId, waitForToast } from './helpers/dom'
import {
  FIXTURE,
  downloadedEmoteFoldersZip,
  downloadedEmoteZip,
  eyesWithMaskZip,
  legacyWearableZip
} from './helpers/files'
import { PREVIEW_METRICS, waitForProcessedAction } from './helpers/preview'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const draft = collection({ name: 'Summer Drop' })

async function openWithFiles(paths: string[], opts: Partial<LaunchOptions> = {}): Promise<App> {
  const launched = await launchApp({ path: `/collections/${draft.id}`, builder: { collections: [draft] }, ...opts })
  await waitForTestId(launched.page, 'collection-empty')
  const input = (await launched.page.$(byTestId('add-items-input'))) as ElementHandle<HTMLInputElement>
  await input.uploadFile(...paths)
  await waitForTestId(launched.page, 'add-items-modal')
  return launched
}

/** The file name of the draft whose form is showing, once it finished processing. */
async function readyDraft(a: App): Promise<string> {
  await waitForTestId(a.page, 'draft-form', 30_000)
  // A single file has no draft list.
  return a.page.evaluate(
    () => document.querySelector('[data-testid^="draft-"][data-selected]')?.getAttribute('data-file-name') ?? ''
  )
}

async function savedItems(a: App) {
  await a.page.waitForFunction(() => !document.querySelector('[data-testid="add-items-modal"]'), { timeout: 30_000 })
  return [...a.builder.items.values()].filter(item => item.collection_id === draft.id)
}

/** Every file a saved item points at was uploaded, so it previews and validates from storage. */
const uploaded = (a: App, contents: Record<string, string>) =>
  Object.values(contents).every(hash => (a.storage[hash]?.length ?? 0) > 0)

describe('add items to a collection', () => {
  it('imports a batch of a manifest zip, an emote and a facial feature', async () => {
    app = await openWithFiles([await legacyWearableZip(), await downloadedEmoteZip(), await eyesWithMaskZip()])

    const seen: string[] = []
    while (seen.length < 3) {
      const file = await readyDraft(app)
      seen.push(file)
      // A zip without emote.json carries no category.
      if (file.startsWith('downloaded-emote')) await selectOption(app.page, 'item-category', 'greetings')
      if (file.startsWith('eyes')) {
        // A texture can only be a facial feature, and it carries no category of its own.
        expect((await selectOptions(app.page, 'item-category')).sort()).toEqual(['eyebrows', 'eyes', 'mouth'])
        await selectOption(app.page, 'item-category', 'eyes')
      }
      // Save & Next on every draft but the last one, which shows Finish.
      const action = seen.length < 3 ? 'add-items-save-next' : 'add-items-finish'
      await waitForProcessedAction(app, action)
      await clickTestId(app.page, action)
      if (seen.length < 3) {
        await app.page.waitForFunction(
          (previous: string) =>
            document.querySelector('[data-testid^="draft-"][data-selected]')?.getAttribute('data-file-name') !==
            previous,
          {},
          file
        )
      }
    }
    expect(seen.sort()).toEqual(['downloaded-emote.zip', 'eyes-with-mask.zip', 'legacy-wearable.zip'])

    const items = await savedItems(app)
    const byName = Object.fromEntries(items.map(item => [item.name, item]))
    expect(byName['Yellow T-Shirt']).toMatchObject({
      type: 'wearable',
      rarity: 'common',
      data: { category: 'upper_body' }
    })
    expect(byName['Yellow T-Shirt'].data.representations).toHaveLength(2)
    const emote = items.find(item => item.type === 'emote')
    expect(emote?.data).toMatchObject({ category: 'greetings' })
    // The downloaded emote ships its model once, at the root, and comes back serving both body shapes.
    expect(emote?.data.representations).toHaveLength(2)
    const eyes = items.find(item => item.data.category === 'eyes')
    expect(Object.keys(eyes?.contents ?? {}).some(path => path.endsWith('eyes_mask.png'))).toBe(true)
    expect(items.every(item => uploaded(app!, item.contents))).toBe(true)

    await waitForToast(app.page)
    await app.waitForEvent('Add items', { collectionId: draft.id, item_count: 3, failed_count: 0 })
    await app.page.waitForFunction(() => document.querySelectorAll('[data-testid="item-row"]').length === 3)
  })

  it('imports an emote downloaded with a folder per body shape', async () => {
    app = await openWithFiles([await downloadedEmoteFoldersZip()])

    await readyDraft(app)
    await selectOption(app.page, 'item-category', 'greetings')
    await waitForProcessedAction(app, 'add-items-finish')
    await clickTestId(app.page, 'add-items-finish')

    const [item] = await savedItems(app)
    expect(item).toMatchObject({ type: 'emote', data: { category: 'greetings' } })
    expect(item.data.representations).toHaveLength(2)
    expect(
      Object.keys(item.contents)
        .filter(path => path.endsWith('.glb'))
        .sort()
    ).toEqual(['female/emote.glb', 'male/emote.glb'])
    expect(uploaded(app, item.contents)).toBe(true)
  })

  it('saves a single model with the picked category and rarity', async () => {
    app = await openWithFiles([FIXTURE.hat])

    await readyDraft(app)
    await selectOption(app.page, 'item-category', 'hat')
    await selectOption(app.page, 'item-rarity', 'rare')
    await waitForProcessedAction(app, 'add-items-finish')
    await clickTestId(app.page, 'add-items-finish')

    const [item] = await savedItems(app)
    expect(item).toMatchObject({ name: 'hat', rarity: 'rare', data: { category: 'hat' } })
    // The metrics and the thumbnail come from the preview the draft was rendered in.
    expect(item.metrics).toEqual(PREVIEW_METRICS)
    expect(uploaded(app, item.contents)).toBe(true)
    await app.page.waitForFunction(() => document.querySelectorAll('[data-testid="item-row"]').length === 1)
  })

  it('shows the thumbnail as being generated until the preview has rendered it', async () => {
    app = await openWithFiles([FIXTURE.hat], { preview: { holdLoads: true } })

    await readyDraft(app)
    await waitForTestId(app.page, 'draft-thumbnail-loading')

    await vi.waitFor(() => expect(app!.previewAnswers).toContainEqual({ method: 'load', result: 'held' }), {
      timeout: 15_000
    })
    await app.page.evaluate(() =>
      document
        .querySelector<HTMLIFrameElement>('[data-testid="draft-processor"] iframe')
        ?.contentWindow?.postMessage({ type: 'e2e_release' }, '*')
    )
    await waitForTestId(app.page, 'draft-thumbnail-img')
    expect(await app.page.$(byTestId('draft-thumbnail-loading'))).toBeNull()
  })

  it('requires the preview video of a smart wearable', async () => {
    app = await openWithFiles([FIXTURE.smartWearable])

    await readyDraft(app)
    expect(await app.page.$eval(byTestId('add-items-finish'), el => (el as HTMLButtonElement).disabled)).toBe(true)

    const video = (await app.page.$(byTestId('draft-video-input'))) as ElementHandle<HTMLInputElement>
    await video.uploadFile(FIXTURE.previewVideo)
    await waitForTestId(app.page, 'draft-video-preview')
    await waitForProcessedAction(app, 'add-items-finish')
    await clickTestId(app.page, 'add-items-finish')

    const [item] = await savedItems(app)
    expect(item.name).toBe('Smart Red Bandana')
    expect(item.contents).toHaveProperty('video.mp4')
    expect(Object.keys(item.contents).some(path => path.endsWith('bin/game.js'))).toBe(true)
    expect(uploaded(app, item.contents)).toBe(true)
    expect(app.writes).toContain(`POST /v1/items/${item.id}/videos`)
    await waitForTestId(app.page, 'item-row-smart')
  })
})

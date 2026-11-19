import { afterEach, describe, expect, it } from 'vitest'
import { HASH, collection, upperBody } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { clickTestId, uploadFiles, waitForEditor, waitForTestId, waitForToast } from './helpers/dom'
import { announceReadyFromAnotherPreview, waitForProcessedAction } from './helpers/preview'
import { FIXTURE } from './helpers/files'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const draft = collection({ name: 'Summer Drop' })
const shirt = upperBody({ collection_id: draft.id })

type Representation = { bodyShapes: string[]; mainFile: string; contents: string[] }
const shapeOf = (representation: Representation) =>
  representation.bodyShapes[0].endsWith('BaseMale') ? 'male' : 'female'

describe('replace a model', () => {
  it('replaces one body shape and keeps the other', async () => {
    app = await launchApp({
      path: `/collections/editor?collection=${draft.id}&item=${shirt.id}`,
      builder: { collections: [draft], items: [shirt] }
    })
    const { page } = app
    await waitForEditor(page)
    const before = structuredClone(app.builder.items.get(shirt.id)!)

    await clickTestId(page, 'properties-panel-actions')
    await clickTestId(page, 'properties-panel-change-file')
    await waitForTestId(page, 'change-file-modal')
    await uploadFiles(page, () => clickTestId(page, 'change-file-browse'), [FIXTURE.poloMale])
    await waitForTestId(page, 'change-file-form', 30_000)
    await clickTestId(page, 'body-shape-male')
    await page.waitForSelector('[data-testid="change-file-effect"][data-mode="replace_shape"]')
    await waitForProcessedAction(app, 'change-file-save')
    await clickTestId(page, 'change-file-save')

    await waitForToast(page)
    await app.waitForEvent('Change item file', { itemId: shirt.id, bodyShape: 'male', mode: 'replace_shape' })
    const after = app.builder.items.get(shirt.id)!
    const representations = (item: typeof after) =>
      Object.fromEntries((item.data.representations as Representation[]).map(r => [shapeOf(r), r]))
    expect(representations(after).female).toEqual(representations(before).female)
    expect(after.contents[representations(after).female.mainFile]).toBe(HASH.tshirtFemale)
    const male = representations(after).male
    expect(after.contents[male.mainFile]).not.toBe(HASH.tshirtMale)
    // The new model is the uploaded file, byte for byte.
    const stored = app.storage[after.contents[male.mainFile]]
    const original = app.storage[HASH.poloMale]
    expect(stored?.equals(original)).toBe(true)
  })

  it('processes the new model when another preview announces itself first', async () => {
    app = await launchApp({
      path: `/collections/editor?collection=${draft.id}&item=${shirt.id}`,
      builder: { collections: [draft], items: [shirt] }
    })
    const { page } = app
    await waitForEditor(page)
    await announceReadyFromAnotherPreview(page)

    await clickTestId(page, 'properties-panel-actions')
    await clickTestId(page, 'properties-panel-change-file')
    await waitForTestId(page, 'change-file-modal')
    await uploadFiles(page, () => clickTestId(page, 'change-file-browse'), [FIXTURE.poloMale])
    await waitForTestId(page, 'change-file-form', 30_000)

    await waitForProcessedAction(app, 'change-file-save')
  })
})

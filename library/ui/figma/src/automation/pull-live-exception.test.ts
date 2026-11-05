import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, runSync, type App } from '../app.js'
import { FigmaClient } from '../figma/client.js'
import { loadSpecsFromSource } from '../specs.js'
import { writeReviewManifest } from '../review-manifest.js'
import { exportImageFills, exportNodes } from './exports.js'
import type { LiveFile, LiveNode } from './live-file.js'
import { pullLive } from './pull-live.js'
import { capturePage } from './rest-capture.js'

vi.mock('./exports.js', () => ({ exportNodes: vi.fn(), exportImageFills: vi.fn() }))

describe('when a Review exception is captured on its ready page', () => {
  let root: string
  let app: App
  let client: FigmaClient
  let file: LiveFile
  let inventoryPath: string
  let originalInventory: string
  const pageId = '8698:2193'
  const componentId = '15906:10331'
  const save = async (file: string, value: unknown) => {
    await mkdir(path.dirname(file), { recursive: true })
    await writeFile(file, JSON.stringify(value) + '\n')
  }
  const ready = (value: boolean) => {
    file.document.children![0]!.children![0]!.componentProperties = {
      ready: { type: 'VARIANT', value: String(value) }
    }
  }
  beforeEach(async () => {
    // Live access reads the key from the environment only; a fake one stands in.
    vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    root = await mkdtemp(path.join(os.tmpdir(), 'ready-exception-'))
    const page: LiveNode = {
      id: pageId,
      name: 'Modals',
      type: 'CANVAS',
      children: [
        {
          id: componentId,
          name: 'ShopModals',
          type: 'COMPONENT',
          absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
        }
      ]
    }
    await save(path.join(root, 'figma/figma.config.json'), {
      snapshotDir: 'figma-snapshot',
      outDir: 'review/src/generated',
      pages: { [pageId]: { id: pageId, name: 'Modals', kind: 'components' } },
      selection: {
        readyOnly: true,
        exceptions: [{ pageId, componentId, reason: 'Review exception' }]
      }
    })
    for (const [name, value] of Object.entries({
      'variables.json': {
        collections: ['Primitives', 'Semantic', 'Spacing', 'Layout'].map((name) => ({
          name,
          modes: ['Value'],
          variables: []
        }))
      },
      'text-styles.json': { styles: [] },
      'paint-styles.json': { gradients: [] },
      'icons.json': { icons: [] },
      'pages.json': { pages: [{ id: pageId, name: 'Modals' }] },
      'sync-state.json': {
        pages: [{ id: pageId, name: 'Modals', ready: false }]
      },
      'generation-inventory.json': {
        schema: 1,
        review: [{ pageId, componentIds: [componentId] }],
        promotion: []
      },
      'components/Modals.json': capturePage(page, new Map())
    }))
      await save(path.join(root, 'figma/figma-snapshot', name), value)
    app = await createApp(path.join(root, 'figma'))
    await runSync(app, { source: 'snapshot', developer: true })
    await writeReviewManifest(app)
    await save(path.join(root, 'figma/review/src/review/references.json'), {
      'ShopModals-0': { file: '/old-shop.png', width: 20, height: 20 }
    })
    inventoryPath = path.join(root, 'figma/figma-snapshot/generation-inventory.json')
    originalInventory = await readFile(inventoryPath, 'utf8')
    file = {
      version: 'fixture-v1',
      styles: {},
      components: { marker: { componentSetId: '15973:15' } },
      document: {
        id: 'root',
        name: 'root',
        type: 'DOCUMENT',
        children: [
          {
            ...page,
            children: [
              {
                id: 'status',
                name: 'Sync status',
                type: 'INSTANCE',
                componentId: 'marker',
                componentProperties: { ready: { type: 'VARIANT', value: 'false' } }
              }
            ]
          }
        ]
      }
    }
    client = new FigmaClient(
      { getToken: async () => 'fixture' },
      vi
        .fn<typeof fetch>()
        .mockImplementation(async (url) =>
          Response.json(
            String(url).includes('/nodes') ? { nodes: { [pageId]: { document: page } } } : file
          )
        )
    )
    vi.mocked(exportImageFills).mockResolvedValue(new Map())
    vi.mocked(exportNodes).mockImplementation(async (_client, _key, _version, ids, format) => {
      if (format !== 'png') return new Map()
      const png = Buffer.alloc(24)
      png.writeUInt32BE(20, 16)
      png.writeUInt32BE(20, 20)
      return new Map(ids.map((id) => [id, png]))
    })
  })
  afterEach(async () => {
    vi.resetAllMocks()
    vi.unstubAllEnvs()
    await rm(root, { recursive: true, force: true })
  })
  it('should keep ShopModals Review-only while Modals is OFF', async () => {
    await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
    expect(await readFile(inventoryPath, 'utf8')).toBe(originalInventory)
    expect(
      (await loadSpecsFromSource(app.createSource('snapshot'), app.config, 'review')).specs.map(
        (spec) => spec.figmaId
      )
    ).toEqual([componentId])
    expect(
      (await loadSpecsFromSource(app.createSource('snapshot'), app.config, 'promotion')).specs
    ).toEqual([])
  })
  it('should admit ShopModals after an ON capture and freeze that scope on a later OFF run', async () => {
    ready(true)
    await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
    expect(JSON.parse(await readFile(inventoryPath, 'utf8')).promotion).toEqual([{ pageId }])
    expect(
      (await loadSpecsFromSource(app.createSource('snapshot'), app.config, 'promotion')).specs.map(
        (spec) => spec.figmaId
      )
    ).toEqual([componentId])
    ready(false)
    await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
    expect(JSON.parse(await readFile(inventoryPath, 'utf8')).promotion).toEqual([{ pageId }])
  })
  it('should admit an unchanged ready capture when only publication inventory needs expansion', async () => {
    ready(true)
    await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
    await writeFile(inventoryPath, originalInventory)
    expect(
      (await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)).changed
    ).toBe(true)
    expect(JSON.parse(await readFile(inventoryPath, 'utf8')).promotion).toEqual([{ pageId }])
    expect(
      (await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)).changed
    ).toBe(false)
  })
  it('should not admit ShopModals from an ON dry run without publishing its capture', async () => {
    ready(true)
    await pullLive(app, { apply: false, report: '.figma-cache/report.json' }, client)
    expect(await readFile(inventoryPath, 'utf8')).toBe(originalInventory)
    expect(
      (await loadSpecsFromSource(app.createSource('snapshot'), app.config, 'promotion')).specs
    ).toEqual([])
  })
})

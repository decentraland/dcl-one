import { createHash } from 'node:crypto'
import { crc32 } from 'node:zlib'
import { readFileSync } from 'node:fs'
import { cp, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, runSync, type App } from '../app.js'
import { FigmaClient } from '../figma/client.js'
import { writeReviewManifest } from '../review-manifest.js'
import type { LiveFile, LiveNode } from './live-file.js'
import { decodePng } from '../capture/png-decode.js'
import { pullLive } from './pull-live.js'
import { encodePageFileName } from '../capture/page-file-name.js'

// A successful `pull-live --page --apply` over a real (frozen) capture and a real generated Review:
// the selected page is refreshed, every unselected page keeps its capture, readiness, baseline entry and
// references, and the only files written are the ones listed in SHARED_REWRITES or the selected page.
const fixtures = path.resolve(import.meta.dirname, '../../test/fixtures')
const SELECTED = { id: '8711:14600', name: 'Text Input' }
const UNSELECTED = { id: '8711:14972', name: 'Search Bar' }
const VERSION = 'v-live'

// 1x1 transparent PNG with valid CRCs.
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=',
  'base64'
)
// What Figma's exporter actually returns: the same image with a tEXt "Software" chunk after IHDR.
const softwareChunk = (() => {
  const body = Buffer.concat([
    Buffer.from('tEXt', 'latin1'),
    Buffer.from('Software\0Figma', 'latin1')
  ])
  const length = Buffer.alloc(4)
  length.writeUInt32BE(body.length - 4)
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(body))
  return Buffer.concat([length, body, crc])
})()
const EXPORTED = Buffer.concat([PNG.subarray(0, 33), softwareChunk, PNG.subarray(33)])

type Readiness = { id: string; name: string; ready: boolean | null }

async function tree(root: string): Promise<Record<string, string>> {
  const entries = await readdir(root, { recursive: true, withFileTypes: true })
  const files = entries
    .filter((entry) => entry.isFile())
    .map((entry) => path.relative(root, path.join(entry.parentPath, entry.name)))
  return Object.fromEntries(
    await Promise.all(
      files.map(async (file) => [
        file,
        createHash('sha256')
          .update(await readFile(path.join(root, file)))
          .digest('hex')
      ])
    )
  )
}

const json = async (file: string) => JSON.parse(await readFile(file, 'utf8'))

const marker = (ready: boolean): LiveNode => ({
  id: `marker-${String(ready)}`,
  name: 'Sync status',
  type: 'INSTANCE',
  componentId: ready ? 'yes' : 'no',
  componentProperties: { ready: { type: 'VARIANT', value: String(ready) } }
})

// The new Figma content of the selected page: one text-only component set (no vector artwork).
const variant = (id: string, state: string, text: string): LiveNode => ({
  id,
  name: `State=${state}`,
  type: 'COMPONENT',
  absoluteBoundingBox: { x: 0, y: 0, width: 120, height: 32 },
  children: [
    {
      id: `${id}:label`,
      name: 'Label',
      type: 'TEXT',
      absoluteBoundingBox: { x: 8, y: 6, width: 80, height: 20 },
      characters: text,
      style: { fontFamily: 'Inter', fontSize: 16, fontWeight: 400, lineHeightPx: 20 },
      fills: [{ type: 'SOLID', color: { r: 1, g: 1, b: 1, a: 1 } }]
    }
  ]
})
// A recapture must still contain every component the saved capture has (capture-completeness), so the
// refreshed page keeps the saved sets, each with its saved variants, beside the new one.
type SavedSet = {
  id: string
  name: string
  base: { id: string; name: string }
  others: { id: string; name: string }[]
}
const savedSets = (
  JSON.parse(
    readFileSync(
      path.join(fixtures, 'figma-snapshot/components', encodePageFileName(SELECTED.name)),
      'utf8'
    )
  ) as { components: SavedSet[] }
).components.map((set): LiveNode => ({
  id: set.id,
  name: set.name,
  type: 'COMPONENT_SET',
  children: [set.base, ...set.others].map((entry) => ({
    ...variant(entry.id, 'Default', 'Saved'),
    name: entry.name
  }))
}))
// Every variant the refreshed page renders: the saved sets' and the new set's.
const SELECTED_VARIANTS = [
  ...savedSets.flatMap((set) => (set.children ?? []).map((node) => node.id)),
  '9000:2',
  '9000:3'
].map((id) => id.replace(':', '-'))
const selectedReference = new RegExp(`/live-(${SELECTED_VARIANTS.join('|')})\\.png$`)
const selectedLive: LiveNode = {
  id: SELECTED.id,
  name: SELECTED.name,
  type: 'CANVAS',
  children: [
    marker(true),
    ...savedSets,
    {
      id: '9000:1',
      name: 'Refreshed Field',
      type: 'COMPONENT_SET',
      componentPropertyDefinitions: {
        State: { type: 'VARIANT', defaultValue: 'Default', variantOptions: ['Default', 'Hover'] }
      },
      children: [variant('9000:2', 'Default', 'Name'), variant('9000:3', 'Hover', 'Name')]
    }
  ]
}

describe('when one page is refreshed with --page --apply', () => {
  let directory: string
  let app: App
  let saved: { pages: Readiness[] }
  let before: Record<string, string>
  let requests: string[]

  beforeEach(async () => {
    // The tool's own layout (layout.ts): the CLI in figma/, its snapshot and Review inside it.
    directory = path.join(await mkdtemp(path.join(os.tmpdir(), 'live-page-')), 'figma')
    vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    await cp(path.join(fixtures, 'figma-snapshot'), path.join(directory, 'figma-snapshot'), {
      recursive: true
    })
    // The frozen capture lists Tabs (15674:34262) on both Buttons and Tabs bar, from before Figma moved it.
    // The scope guard rejects an automated run over two owners, so the stale Buttons copy is dropped.
    const buttons = path.join(directory, 'figma-snapshot/components/Buttons.json')
    const capture = await json(buttons)
    await writeFile(
      buttons,
      JSON.stringify({
        ...capture,
        components: capture.components.filter((entry: { id: string }) => entry.id !== '15674:34262')
      })
    )
    const config = await json(path.join(fixtures, 'figma.config.json'))
    await writeFile(
      path.join(directory, 'figma.config.json'),
      JSON.stringify({
        ...config,
        snapshotDir: 'figma-snapshot',
        outDir: 'review/src/generated',
        contract: undefined
      })
    )
    app = await createApp(directory)
    // The Review the run starts from: generated, catalogued, with one reference per variant.
    // Setup only: a developer regeneration of the starting Review (automated runs are scope-checked).
    await runSync(app, { source: 'snapshot', developer: true })
    await writeReviewManifest(app)
    const catalog = await json(path.join(directory, 'review/src/review/catalog.json'))
    const references: Record<string, unknown> = {}
    for (const component of catalog.components as {
      name: string
      variants: { id: string }[]
    }[])
      component.variants.forEach((entry, index) => {
        references[`${component.name}-${index}`] = {
          file: `/dcl-ui-review/references/${entry.id.replace(/[^a-zA-Z0-9_-]/g, '-')}.png`,
          width: 1,
          height: 1,
          offsetX: 0,
          offsetY: 0
        }
      })
    await writeFile(
      path.join(directory, 'review/src/review/references.json'),
      JSON.stringify(references, null, 2) + '\n'
    )
    saved = await json(path.join(directory, 'figma-snapshot/sync-state.json'))
    before = await tree(directory)

    // Live readiness: the selected page is ready, and the unselected Search Bar differs from its saved
    // entry (now not ready), which a single-page run must not record.
    const file: LiveFile = {
      version: VERSION,
      styles: {},
      components: { yes: { componentSetId: '15973:15' }, no: { componentSetId: '15973:15' } },
      document: {
        id: '0:0',
        name: 'Document',
        type: 'DOCUMENT',
        children: saved.pages.map((page) => ({
          id: page.id,
          name: page.name,
          type: 'CANVAS',
          children:
            page.ready === null
              ? []
              : [marker(page.id === UNSELECTED.id ? false : page.ready === true)]
        }))
      }
    }
    requests = []
    const figma = vi.fn<typeof fetch>(async (input) => {
      const url = String(input)
      // Rendered exports are downloaded through the same client, so they are answered here too.
      if (url.startsWith('https://images.figmausercontent.com/'))
        return new Response(new Uint8Array(EXPORTED))
      requests.push(url.replace(/^https:\/\/api\.figma\.com/, ''))
      if (url.includes('/nodes?'))
        return Response.json({ nodes: { [SELECTED.id]: { document: selectedLive } } })
      if (url.includes('/v1/images/')) {
        const ids = new URL(url).searchParams.get('ids')!.split(',')
        return Response.json({
          images: Object.fromEntries(
            ids.map((id) => [id, `https://images.figmausercontent.com/${id}`])
          )
        })
      }
      return Response.json(file)
    })
    // Nothing may reach the network: every request goes through the fake client above.
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no network in tests'))
    const client = new FigmaClient({ getToken: async () => 'test' }, figma)
    await pullLive(
      app,
      {
        apply: true,
        report: path.join(directory, '..', `${path.basename(directory)}-report.json`),
        page: SELECTED.name
      },
      client
    )
  }, 120_000)

  afterEach(async () => {
    await rm(path.dirname(directory), { recursive: true, force: true })
    await rm(path.join(directory, '..', `${path.basename(directory)}-report.json`), { force: true })
    vi.restoreAllMocks()
    vi.unstubAllEnvs()
  })

  it('should read only the selected page from Figma', () => {
    expect(requests.filter((url) => url.includes('/nodes?'))).toEqual([
      `/v1/files/FAKEKEY000example000000/nodes?ids=${encodeURIComponent(SELECTED.id)}&version=${VERSION}&geometry=paths`
    ])
  })

  it('should write the selected references without metadata and with the same pixels', async () => {
    const dir = path.join(directory, 'review/public/dcl-ui-review/references')
    const written = (await readdir(dir)).filter((file) => file.startsWith('live-9000-'))

    expect(written.length).toBeGreaterThan(0)
    expect(decodePng(EXPORTED).problem).toBeUndefined()
    for (const file of written) {
      const bytes = await readFile(path.join(dir, file))
      expect(bytes.includes(Buffer.from('tEXt', 'latin1'))).toBe(false)
      expect(decodePng(bytes).png!.pixels.equals(decodePng(EXPORTED).png!.pixels)).toBe(true)
    }
  })

  it('should refresh the selected capture and baseline entry', async () => {
    const page = await json(
      path.join(directory, 'figma-snapshot/components', encodePageFileName(SELECTED.name))
    )
    const baseline = await json(path.join(directory, 'figma-snapshot/live-source.json'))

    expect(page.components.map((component: { name: string }) => component.name)).toEqual([
      ...savedSets.map((set) => set.name),
      'Refreshed Field'
    ])
    expect(baseline.pages[SELECTED.id].version).toBe(VERSION)
  })

  it('should keep every unselected capture, readiness entry, baseline entry and reference', async () => {
    const after = await tree(directory)
    const state = await json(path.join(directory, 'figma-snapshot/sync-state.json'))
    const baselineBefore = JSON.parse(
      await readFile(path.join(fixtures, 'figma-snapshot/live-source.json'), 'utf8')
    )
    const baselineAfter = await json(path.join(directory, 'figma-snapshot/live-source.json'))
    const catalog = await json(path.join(directory, 'review/src/review/catalog.json'))
    const references = await json(path.join(directory, 'review/src/review/references.json'))

    expect(state.pages).toEqual(saved.pages)
    expect(state.pages.find((page: Readiness) => page.id === UNSELECTED.id).ready).toBe(true)
    for (const [id, entry] of Object.entries(baselineBefore.pages))
      if (id !== SELECTED.id) expect(baselineAfter.pages[id]).toEqual(entry)
    for (const file of Object.keys(before).filter(
      (file) =>
        file.startsWith('figma-snapshot/components/') &&
        file !== `figma-snapshot/components/${encodePageFileName(SELECTED.name)}`
    ))
      expect(after[file]).toBe(before[file])
    for (const component of catalog.components as {
      name: string
      page: string
      variants: { id: string }[]
    }[])
      component.variants.forEach((entry, index) => {
        const reference = references[`${component.name}-${index}`]
        if (component.page === SELECTED.name) expect(reference.file).toMatch(selectedReference)
        else
          expect(reference.file).toBe(
            `/dcl-ui-review/references/${entry.id.replace(/[^a-zA-Z0-9_-]/g, '-')}.png`
          )
      })
  })

  it('should rewrite only the selected capture and the shared files a refresh owns', async () => {
    const after = await tree(directory)
    const changed = Object.keys({ ...before, ...after }).filter(
      (file) => before[file] !== after[file]
    )
    // Shared files rewritten whole on every applied refresh; their unselected entries are checked above.
    const SHARED_REWRITES = [
      /^figma-snapshot\/sync-state\.json$/,
      /^figma-snapshot\/live-source\.json$/,
      /^review\/src\/review\/(catalog|references)\.json$/,
      /^review\/src\/generated\//,
      {
        test: (file: string) =>
          /^review\/public\/dcl-ui-review\/references\//.test(file) && selectedReference.test(file)
      }
    ]

    expect(changed).toContain(`figma-snapshot/components/${encodePageFileName(SELECTED.name)}`)
    expect(
      changed.filter(
        (file) =>
          file !== `figma-snapshot/components/${encodePageFileName(SELECTED.name)}` &&
          !SHARED_REWRITES.some((pattern) => pattern.test(file))
      )
    ).toEqual([])
  })
})

import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, runSync, type App } from '../app.js'
import { FigmaClient } from '../figma/client.js'
import { capturePage } from './rest-capture.js'
import { writeReviewManifest } from '../review-manifest.js'
import { exportNodes, exportImageFills } from './exports.js'
import { pullLive, scopeReport } from './pull-live.js'
import * as catalogEvidence from './catalog-diff.js'
import type { LiveFile, LiveNode } from './live-file.js'

vi.mock('./exports.js', () => ({ exportNodes: vi.fn(), exportImageFills: vi.fn() }))

async function filesUnder(directory: string): Promise<Record<string, string>> {
  const output: Record<string, string> = {}
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === '.figma-cache') continue
    if (entry.isDirectory()) {
      for (const [file, bytes] of Object.entries(
        await filesUnder(path.join(directory, entry.name))
      ))
        output[`${entry.name}/${file}`] = bytes
    } else
      output[entry.name] = (await readFile(path.join(directory, entry.name))).toString('base64')
  }
  return output
}

describe('when applying a fixture single-page sync', () => {
  let root: string
  let app: App
  let selected: LiveNode
  let frozen: LiveNode
  let file: LiveFile
  let fetchFn: ReturnType<typeof vi.fn<typeof fetch>>
  let client: FigmaClient
  let before: Record<string, string>
  let previousState: string
  const save = async (file: string, value: unknown) => {
    await mkdir(path.dirname(file), { recursive: true })
    await writeFile(file, JSON.stringify(value, null, 2) + '\n')
  }
  beforeEach(async () => {
    // Live access reads the key from the environment only; a fake one stands in.
    vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    root = await mkdtemp(path.join(os.tmpdir(), 'sync-scope-fixture-'))
    selected = {
      id: '1:1',
      name: 'Alpha',
      type: 'CANVAS',
      children: [
        {
          id: '10:1',
          name: 'First',
          type: 'COMPONENT',
          absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
        }
      ]
    }
    frozen = {
      id: '1:2',
      name: 'Beta',
      type: 'CANVAS',
      children: [
        {
          id: '10:2',
          name: 'Second',
          type: 'COMPONENT',
          absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
        }
      ]
    }
    await save(path.join(root, 'figma/figma.config.json'), {
      snapshotDir: 'figma-snapshot',
      outDir: 'review/src/generated',
      pages: {
        '1:1': { id: '1:1', name: 'Alpha', kind: 'components' },
        '1:2': { id: '1:2', name: 'Beta', kind: 'components' }
      },
      selection: { readyOnly: true }
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
      'pages.json': {
        pages: [
          { id: '1:1', name: 'Alpha' },
          { id: '1:2', name: 'Beta' }
        ]
      },
      'generation-inventory.json': {
        schema: 1,
        review: [{ pageId: '1:1' }, { pageId: '1:2' }],
        promotion: [{ pageId: '1:1' }, { pageId: '1:2' }]
      },
      'sync-state.json': {
        pages: [
          { id: '1:1', name: 'Alpha', ready: true },
          { id: '1:2', name: 'Beta', ready: true }
        ]
      },
      'retirements.json': { schema: 1, retired: [] },
      'components/Alpha.json': capturePage(selected, new Map()),
      'components/Beta.json': capturePage(frozen, new Map())
    }))
      await save(path.join(root, 'figma/figma-snapshot', name), value)
    app = await createApp(path.join(root, 'figma'))
    await runSync(app, { source: 'snapshot', developer: true })
    await writeReviewManifest(app)
    await save(path.join(root, 'figma/review/src/review/references.json'), {
      'First-0': { file: '/old-first.png', width: 20, height: 20 },
      'Second-0': { file: '/old-second.png', width: 20, height: 20 }
    })
    await mkdir(path.join(root, 'figma/review/public'), { recursive: true })
    before = await filesUnder(root)
    previousState = await readFile(path.join(root, 'figma/figma-snapshot/sync-state.json'), 'utf8')
    file = {
      version: 'fixture-v1',
      styles: {},
      components: { marker: { componentSetId: '15973:15' } },
      document: {
        id: 'root',
        name: 'root',
        type: 'DOCUMENT',
        children: [selected, frozen].map((page, index) => ({
          ...page,
          children: [
            {
              id: `marker${index}`,
              name: 'Sync status',
              type: 'INSTANCE',
              componentId: 'marker',
              componentProperties: {
                ready: { type: 'VARIANT', value: index === 0 ? 'true' : 'false' }
              }
            }
          ]
        }))
      }
    }
    selected.children![0]!.absoluteBoundingBox = { x: 0, y: 0, width: 22, height: 20 }
    fetchFn = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json(file))
      .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
      .mockResolvedValueOnce(Response.json(file))
    client = new FigmaClient({ getToken: async () => 'fixture' }, fetchFn)
    vi.mocked(exportImageFills).mockResolvedValue(new Map())
    vi.mocked(exportNodes).mockImplementation(async (_client, _key, _version, ids, format) => {
      if (format !== 'png') return new Map()
      const png = Buffer.alloc(24)
      png.writeUInt32BE(22, 16)
      png.writeUInt32BE(20, 20)
      return new Map(ids.map((id) => [id, png]))
    })
  })
  afterEach(async () => {
    vi.resetAllMocks()
    vi.unstubAllEnvs()
    await rm(root, { recursive: true, force: true })
  })
  it('should keep the unselected snapshot, generated files, catalog entries and references unchanged', async () => {
    await pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
    expect((await filesUnder(root))['figma/figma-snapshot/components/Beta.json']).toBe(
      before['figma/figma-snapshot/components/Beta.json']
    )
    expect((await filesUnder(root))['figma/review/src/generated/components/beta/Second.tsx']).toBe(
      before['figma/review/src/generated/components/beta/Second.tsx']
    )
    expect(
      JSON.parse(await readFile(path.join(root, 'figma/figma-snapshot/sync-state.json'), 'utf8'))
        .pages[1]
    ).toEqual(JSON.parse(previousState).pages[1])
    expect(
      JSON.parse(
        await readFile(path.join(root, 'figma/review/src/review/references.json'), 'utf8')
      )['Second-0']
    ).toEqual({ file: '/old-second.png', width: 20, height: 20 })
    expect(
      await readFile(
        path.join(root, 'figma/review/src/generated/components/alpha/First.module.css'),
        'utf8'
      )
    ).toContain('22')
  })
  it('should preserve an existing library promotion manifest during Review-only capture', async () => {
    await save(path.join(root, 'components/promotion.json'), {
      components: ['First', 'Second'],
      files: {}
    })
    before = await filesUnder(root)
    await pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
    expect((await filesUnder(root))['components/promotion.json']).toBe(
      before['components/promotion.json']
    )
  })
  describe('and Figma saves a newer version during the capture', () => {
    const latest = () => ({ ...file, version: 'fixture-v2' })
    const replaceLatest = (next: unknown) => {
      fetchFn.mockReset()
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
        .mockResolvedValueOnce(Response.json(next))
    }

    it('should publish the output pinned to the starting version and record both versions', async () => {
      const pinnedRoot = await mkdtemp(path.join(os.tmpdir(), 'pull-live-pinned-'))
      await cp(root, pinnedRoot, { recursive: true })
      const pinnedApp = await createApp(path.join(pinnedRoot, 'figma'))
      await pullLive(
        pinnedApp,
        { apply: true, page: '1:1', report: '.figma-cache/report.json' },
        client
      )

      replaceLatest(latest())
      const report = await pullLive(
        app,
        { apply: true, page: '1:1', report: '.figma-cache/report.json' },
        client
      )
      expect(report).toMatchObject({ version: file.version, latestVersion: 'fixture-v2' })
      const published = await filesUnder(root)
      const pinned = await filesUnder(pinnedRoot)
      for (const key of Object.keys(published))
        if (!key.startsWith('figma/.figma-cache/') && !key.endsWith('live-source.json'))
          expect(published[key], key).toBe(pinned[key])
      await rm(pinnedRoot, { recursive: true, force: true })
    })

    it('should still abort when page readiness changes', async () => {
      const next = latest()
      next.document = structuredClone(file.document)
      next.document.children![0]!.children![0]!.componentProperties = {
        ready: { type: 'VARIANT', value: 'false' }
      }
      replaceLatest(next)
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('Figma readiness changed during capture')
      expect(await filesUnder(root)).toEqual(before)
    })

    it('should read every capture input at the pinned version or by content address', async () => {
      // A structured photo fill in the pinned page: its bytes may only come from its content address.
      const pinnedPage = structuredClone(selected)
      pinnedPage.children![0]!.children = [
        {
          id: '10:9',
          name: 'Photo',
          type: 'FRAME',
          absoluteBoundingBox: { x: 0, y: 0, width: 22, height: 20 },
          fills: [{ type: 'IMAGE', imageRef: 'pinned-photo-ref', scaleMode: 'FILL' }],
          // A caption keeps the photo frame structured instead of exported as artwork.
          children: [
            {
              id: '10:10',
              name: 'Caption',
              type: 'TEXT',
              characters: 'Caption',
              absoluteBoundingBox: { x: 0, y: 0, width: 22, height: 10 },
              style: { fontFamily: 'Inter', fontWeight: 400, fontSize: 10, lineHeightPx: 12 }
            }
          ]
        }
      ]
      fetchFn.mockReset()
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: pinnedPage } } }))
        .mockResolvedValueOnce(Response.json(latest()))
      vi.mocked(exportImageFills).mockResolvedValue(
        // A complete 1x1 PNG: the ingest rejects a truncated one (imageDataProblem).
        new Map([
          [
            'pinned-photo-ref',
            'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII='
          ]
        ])
      )

      await pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      const urls = fetchFn.mock.calls.map((call) => String(call[0]))
      const fileRead = `https://api.figma.com/v1/files/${process.env.FIGMA_FILE_KEY}?depth=2`
      // The two whole-file reads select pages and readiness; every other read is pinned.
      expect(urls.filter((url) => url === fileRead)).toHaveLength(2)
      for (const url of urls.filter((url) => url !== fileRead))
        expect(url).toContain(`version=${file.version}`)
      for (const call of vi.mocked(exportNodes).mock.calls) expect(call[2]).toBe(file.version)
      // Fill bytes are requested by exactly the refs found in the pinned node read.
      expect(vi.mocked(exportImageFills).mock.calls).toEqual([
        [client, process.env.FIGMA_FILE_KEY, ['pinned-photo-ref']]
      ])
    })
  })

  describe('and frozen catalog metadata would change', () => {
    beforeEach(async () => {
      const catalogPath = path.join(root, 'figma/review/src/review/catalog.json')
      const catalog = JSON.parse(await readFile(catalogPath, 'utf8'))
      catalog.foundations.effects.status = 'Frozen fixture metadata'
      await save(catalogPath, catalog)
      before = await filesUnder(root)
    })
    it('should retain the staged catalog diff after failure without publishing it', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('Sync scope conflict')
      const reportFile = path.join(root, 'figma/.figma-cache/report.json')
      const report = JSON.parse(await readFile(reportFile, 'utf8'))
      expect(report.catalogDiff).toBe('report.json.catalog.diff')
      const diff = await readFile(path.join(path.dirname(reportFile), report.catalogDiff), 'utf8')
      expect(diff).toContain('-      "status": "Frozen fixture metadata"')
      expect(diff).toContain(
        '+      "status": "Disabled in Figma. Existing captured effects are frozen."'
      )
      expect(diff).not.toContain(root)
      expect(report.removalsBlocked).toEqual([])
      expect(await filesUnder(root)).toEqual(before)
    })
    describe('and retaining diagnostics fails', () => {
      beforeEach(() => {
        vi.spyOn(catalogEvidence, 'saveCatalogDiff').mockRejectedValueOnce(
          new Error('Fixture diagnostics disk error')
        )
      })
      afterEach(() => vi.restoreAllMocks())
      it('should preserve the capture failure and record the diagnostic error', async () => {
        await expect(
          pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
        ).rejects.toThrow('Sync scope conflict')
        const report = JSON.parse(
          await readFile(path.join(root, 'figma/.figma-cache/report.json'), 'utf8')
        )
        expect(report.error).toContain('Sync scope conflict')
        expect(report.catalogDiffError).toBe('Fixture diagnostics disk error')
        expect(report).not.toHaveProperty('catalogDiff')
        expect(await filesUnder(root)).toEqual(before)
      })
    })
  })
  describe('and a previous run left catalog evidence', () => {
    let evidenceFile: string
    beforeEach(async () => {
      evidenceFile = path.join(root, 'figma/.figma-cache/report.json.catalog.diff')
      await mkdir(path.dirname(evidenceFile), { recursive: true })
      await writeFile(evidenceFile, 'stale diff')
    })
    it('should discard stale evidence after a successful apply', async () => {
      const report = await pullLive(
        app,
        { apply: true, page: '1:1', report: '.figma-cache/report.json' },
        client
      )
      expect(report.catalogDiff).toBeUndefined()
      await expect(readFile(evidenceFile)).rejects.toMatchObject({ code: 'ENOENT' })
    })
    describe('and the first live read fails', () => {
      beforeEach(() => {
        fetchFn.mockReset().mockRejectedValueOnce(new Error('Fixture API unavailable'))
      })
      it('should remove stale evidence before reporting the early failure', async () => {
        await expect(
          pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
        ).rejects.toThrow('Fixture API unavailable')
        await expect(readFile(evidenceFile)).rejects.toMatchObject({ code: 'ENOENT' })
      })
    })
  })
  describe('and the repository holds a promoted library outside the publish set', () => {
    beforeEach(async () => {
      await save(path.join(root, 'components/promotion.json'), {
        schema: 1,
        files: {},
        components: ['First', 'Second']
      })
      await mkdir(path.join(root, 'components'), { recursive: true })
      await writeFile(path.join(root, 'index.ts'), 'export {}\n')
      before = await filesUnder(root)
    })
    it('should keep the unstaged library paths, report no scope conflict and leave it unchanged', async () => {
      const report = await pullLive(
        app,
        { apply: true, page: '1:1', report: '.figma-cache/report.json' },
        client
      )
      expect(report).toMatchObject({ removalsBlocked: [], scopeConflicts: [] })
      const after = await filesUnder(root)
      for (const file of ['components/promotion.json', 'index.ts'])
        expect(after[file]).toBe(before[file])
    })
  })
  describe('and catalog metadata changes outside the captured scope', () => {
    beforeEach(async () => {
      const catalogPath = path.join(root, 'figma/review/src/review/catalog.json')
      const catalog = JSON.parse(await readFile(catalogPath, 'utf8'))
      catalog.foundations.effects.status = 'Baseline diagnostic marker 987654321'
      await save(catalogPath, catalog)
      before = await filesUnder(root)
    })
    it('should retain exact metadata values in the report and only fields in the error', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('/foundations/effects/status')
      const report = JSON.parse(
        await readFile(path.join(root, 'figma/.figma-cache/report.json'), 'utf8')
      )
      expect(report.metadataChanges).toEqual([
        {
          path: 'figma/review/src/review/catalog.json',
          field: '/foundations/effects/status',
          before: 'Baseline diagnostic marker 987654321',
          after: 'Disabled in Figma. Existing captured effects are frozen.'
        }
      ])
      expect(report.error).not.toContain('987654321')
      expect(report.error).not.toContain('Disabled in Figma')
      expect(report.removalsBlocked).toEqual([])
      expect(report.scopeConflicts).toHaveLength(1)
      expect(report).not.toHaveProperty('scopeViolations')
      expect(await filesUnder(root)).toEqual(before)
    })
  })
  describe('and a selected component collides with a frozen component name', () => {
    beforeEach(() => {
      selected.children!.push({
        id: '10:3',
        name: 'Second',
        type: 'COMPONENT',
        absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
      })
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
        .mockResolvedValueOnce(Response.json(file))
    })
    it('should fail before changing any published artifact', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow(/scope|conflict|remov/i)
      expect(await filesUnder(root)).toEqual(before)
    })
  })
  describe('and the capture omits an existing component', () => {
    beforeEach(() => {
      selected.children = []
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
    })
    it('should fail before exporting or writing any populated output', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('Incomplete capture: missing component')
      expect(exportNodes).not.toHaveBeenCalled()
      expect(await filesUnder(root)).toEqual(before)
    })
  })
  describe('and Figma omits the requested page response', () => {
    beforeEach(() => {
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': null } }))
    })
    it('should fail without changing any published artifact', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('Missing Figma page')
      expect(await filesUnder(root)).toEqual(before)
    })
  })
  describe('and a ready page contains a permanently retired component', () => {
    beforeEach(async () => {
      await save(path.join(root, 'figma/figma-snapshot/retirements.json'), {
        schema: 1,
        retired: [
          {
            pageId: '1:1',
            componentId: '10:3',
            name: 'RetiredControl',
            reason: 'Permanently retired',
            owned: [],
            sharedExclusions: []
          }
        ]
      })
      selected.children!.push({
        id: '10:3',
        name: 'RetiredControl',
        type: 'COMPONENT',
        absoluteBoundingBox: { x: 0, y: 0, width: 20, height: 20 }
      })
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
        .mockResolvedValueOnce(Response.json(file))
    })
    it('should report it as skipped and keep it out of the snapshot and catalog', async () => {
      expect(
        await pullLive(
          app,
          { apply: true, page: '1:1', report: '.figma-cache/report.json' },
          client
        )
      ).toMatchObject({ retiredNotCaptured: [{ pageId: '1:1', componentId: '10:3' }] })
      expect(
        await readFile(path.join(root, 'figma/figma-snapshot/components/Alpha.json'), 'utf8')
      ).not.toContain('RetiredControl')
      expect(
        await readFile(path.join(root, 'figma/review/src/review/catalog.json'), 'utf8')
      ).not.toContain('RetiredControl')
    })
  })
  describe('and the inventory snapshot is missing over populated output', () => {
    beforeEach(async () => {
      await rm(path.join(root, 'figma/figma-snapshot/components/Alpha.json'))
      before = await filesUnder(root)
    })
    it('should fail rather than reconstructing and overwriting published output', async () => {
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).rejects.toThrow('Missing saved capture for inventory page')
      expect(exportNodes).not.toHaveBeenCalled()
      expect(await filesUnder(root)).toEqual(before)
    })
  })
  describe('and two known page IDs exchange display names', () => {
    beforeEach(() => {
      selected.name = 'Beta'
      frozen.name = 'Alpha'
      file.document.children![0]!.name = 'Beta'
      file.document.children![1]!.name = 'Alpha'
      file.document.children![1]!.children![0]!.componentProperties = {
        ready: { type: 'VARIANT', value: 'true' }
      }
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:2': { document: frozen } } }))
        .mockResolvedValueOnce(Response.json(file))
    })
    it('should preserve ownership during a later single-page update', async () => {
      await pullLive(app, { apply: true, report: '.figma-cache/report.json' }, client)
      selected.children![0]!.absoluteBoundingBox = { x: 0, y: 0, width: 24, height: 20 }
      fetchFn
        .mockReset()
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { '1:1': { document: selected } } }))
        .mockResolvedValueOnce(Response.json(file))
      await expect(
        pullLive(app, { apply: true, page: '1:1', report: '.figma-cache/report.json' }, client)
      ).resolves.toMatchObject({ capturedPages: [{ id: '1:1', name: 'Beta' }] })
    })
  })
})

describe('when reporting scope violations', () => {
  it('should include each metadata field once while retaining every scope conflict', () => {
    expect(
      scopeReport([
        {
          path: 'catalog.json',
          owner: null,
          kind: 'aggregate-entry-changed',
          cause: 'invalid derivation',
          metadataChanges: [{ field: '/foundations/strokeWidths', before: [], after: [3] }]
        },
        {
          path: 'catalog.json',
          owner: null,
          kind: 'aggregate-entry-changed',
          cause: 'other frozen metadata changed',
          metadataChanges: [
            { field: '/foundations/strokeWidths', before: [], after: [3] },
            { field: '/foundations/status', before: null }
          ]
        }
      ])
    ).toEqual({
      removalsBlocked: [],
      scopeConflicts: [
        'aggregate-entry-changed: catalog.json: invalid derivation',
        'aggregate-entry-changed: catalog.json: other frozen metadata changed'
      ],
      metadataChanges: [
        { path: 'catalog.json', field: '/foundations/strokeWidths', before: [], after: [3] },
        { path: 'catalog.json', field: '/foundations/status', before: null }
      ]
    })
  })

  it('should list only actual removals as blocked removals and the rest as scope conflicts', () => {
    expect(
      scopeReport([
        {
          path: 'gone.ts',
          owner: null,
          kind: 'removed',
          cause: 'removal without a retirement record'
        },
        {
          path: 'frozen.ts',
          owner: { pageId: '1:2' },
          kind: 'out-of-scope-change',
          cause: 'owned by page 1:2, which was not captured in this run'
        }
      ])
    ).toEqual({
      removalsBlocked: ['gone.ts: removal without a retirement record'],
      scopeConflicts: [
        'out-of-scope-change: frozen.ts: owned by page 1:2, which was not captured in this run'
      ],
      metadataChanges: []
    })
  })
})

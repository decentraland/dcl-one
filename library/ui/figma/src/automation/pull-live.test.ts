import { mkdtemp, writeFile, readFile, rm, mkdir } from 'node:fs/promises'
import os from 'node:os'
import { createHash } from 'node:crypto'
import path from 'node:path'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, type App } from '../app.js'
import { FigmaClient } from '../figma/client.js'
import { pullLive } from './pull-live.js'
import { capturePage } from './rest-capture.js'
import type { LiveFile, LiveNode } from './live-file.js'

describe('when auditing a live file', () => {
  let directory: string
  let app: App
  let client: FigmaClient
  let fetchFn: ReturnType<typeof vi.fn<typeof fetch>>
  let file: LiveFile
  let page: LiveNode
  let previous: string
  beforeEach(async () => {
    // No test reaches the network: Figma answers through the injected client only, and the export
    // downloads (automation/exports.ts, global fetch) fail loudly if a case ever reaches them.
    vi.spyOn(globalThis, 'fetch').mockRejectedValue(new Error('no network in tests'))
    directory = path.join(await mkdtemp(path.join(os.tmpdir(), 'live-test-')), 'figma')
    await mkdir(directory)
    vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    await writeFile(
      path.join(directory, 'figma.config.json'),
      JSON.stringify({
        snapshotDir: '.',
        outDir: 'review/src/generated',
        pages: {
          page: { id: 'page', name: 'Nav bar', kind: 'components' },
          effects: { id: 'effects', name: 'Effects', kind: 'components' }
        },
        selection: { readyOnly: true }
      })
    )
    previous = JSON.stringify({
      pages: [
        { id: 'page', name: 'Nav bar', ready: true },
        { id: 'effects', name: 'Effects', ready: false }
      ]
    })
    await writeFile(path.join(directory, 'sync-state.json'), previous)
    await writeFile(
      path.join(directory, 'generation-inventory.json'),
      JSON.stringify({
        schema: 1,
        review: [],
        promotion: []
      })
    )
    await writeFile(
      path.join(directory, 'pages.json'),
      JSON.stringify({
        pages: [
          { id: 'page', name: 'Nav bar' },
          { id: 'effects', name: 'Effects' }
        ]
      })
    )
    app = await createApp(directory)
    page = { id: 'page', name: 'Nav bar', type: 'CANVAS', children: [] }
    file = {
      version: 'v1',
      styles: {},
      components: { yes: { componentSetId: '15973:15' }, no: { componentSetId: '15973:15' } },
      document: {
        id: 'root',
        name: 'root',
        type: 'DOCUMENT',
        children: [
          {
            ...page,
            children: [
              {
                id: 'marker',
                name: 'Sync status',
                type: 'INSTANCE',
                componentId: 'yes',
                componentProperties: { ready: { type: 'VARIANT', value: 'true' } }
              }
            ]
          },
          {
            id: 'effects',
            name: 'Effects',
            type: 'CANVAS',
            children: [
              {
                id: 'off',
                name: 'Sync status',
                type: 'INSTANCE',
                componentId: 'no',
                componentProperties: { ready: { type: 'VARIANT', value: 'false' } }
              }
            ]
          }
        ]
      }
    }
    fetchFn = vi.fn<typeof fetch>()
    client = new FigmaClient({ getToken: async () => 'test' }, fetchFn)
  })
  afterEach(async () => {
    await rm(path.dirname(directory), { recursive: true, force: true })
    vi.restoreAllMocks()
    vi.unstubAllEnvs()
  })
  it('should fetch only the ready page at the pinned version', async () => {
    fetchFn
      .mockResolvedValueOnce(Response.json(file))
      .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
    await pullLive(app, { report: 'report.json' }, client)
    expect(fetchFn.mock.calls.map((call) => String(call[0]))).toEqual([
      'https://api.figma.com/v1/files/FAKEKEY000example000000?depth=2',
      'https://api.figma.com/v1/files/FAKEKEY000example000000/nodes?ids=page&version=v1&geometry=paths'
    ])
  })
  it('should leave the saved readiness unchanged during an audit', async () => {
    fetchFn
      .mockResolvedValueOnce(Response.json(file))
      .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
    await pullLive(app, { report: 'report.json' }, client)
    expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
  })
  describe('and the ready page is renamed', () => {
    beforeEach(() => {
      file.document.children![0]!.name = 'Navigation ✅'
      page.name = 'Navigation ✅'
    })
    it('should capture by ID and report the stable output name', async () => {
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
      expect(await pullLive(app, { report: 'report.json', page: 'page' }, client)).toMatchObject({
        capturedPages: [{ id: 'page', name: 'Navigation ✅' }],
        renamedPages: [{ id: 'page', name: 'Navigation ✅', previousName: 'Nav bar' }],
        changedPages: ['Nav bar']
      })
    })
  })
  describe('and every configured page is OFF', () => {
    beforeEach(() => {
      file.document.children![0]!.children![0]!.componentProperties = {
        ready: { type: 'VARIANT', value: 'false' }
      }
    })
    it('should capture nothing and preserve the saved readiness bytes', async () => {
      fetchFn.mockResolvedValueOnce(Response.json(file))
      expect(await pullLive(app, { apply: true, report: 'report.json' }, client)).toMatchObject({
        changed: false,
        capturedPages: []
      })
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
      expect(fetchFn).toHaveBeenCalledTimes(1)
    })
  })
  describe('and the same name has a new page ID', () => {
    beforeEach(() => {
      file.document.children![0]!.id = 'replacement'
    })
    it('should fail before reading any component nodes or changing the saved capture', async () => {
      fetchFn.mockResolvedValueOnce(Response.json(file))
      await expect(pullLive(app, { apply: true, report: 'report.json' }, client)).rejects.toThrow(
        'Missing configured Figma page'
      )
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
      expect(fetchFn).toHaveBeenCalledTimes(1)
    })
  })
  describe('and a single page is requested', () => {
    it('should fail on a page that does not exist', async () => {
      fetchFn.mockResolvedValueOnce(Response.json(file))
      await expect(pullLive(app, { report: 'report.json', page: 'Nope' }, client)).rejects.toThrow(
        'Unknown Figma page: Nope'
      )
    })
    it('should fail on a page whose sync switch is off', async () => {
      fetchFn.mockResolvedValueOnce(Response.json(file))
      await expect(
        pullLive(app, { report: 'report.json', page: 'Effects' }, client)
      ).rejects.toThrow('Page is not ready for sync: Effects')
    })
    it('should read only that page even when others are ready', async () => {
      file.document.children![1]!.children![0]!.componentProperties = {
        ready: { type: 'VARIANT', value: 'true' }
      }
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
      const report = await pullLive(app, { report: 'report.json', page: 'Nav bar' }, client)
      expect(report.page).toBe('Nav bar')
      expect(fetchFn.mock.calls.map((call) => String(call[0]))).toEqual([
        'https://api.figma.com/v1/files/FAKEKEY000example000000?depth=2',
        'https://api.figma.com/v1/files/FAKEKEY000example000000/nodes?ids=page&version=v1&geometry=paths'
      ])
    })
  })
  describe('and component content, readiness and inventory have not changed', () => {
    beforeEach(async () => {
      await mkdir(path.join(directory, 'components'), { recursive: true })
      await writeFile(
        path.join(directory, 'components/Nav bar.json'),
        JSON.stringify(capturePage(page, new Map()))
      )
      await writeFile(
        path.join(directory, 'generation-inventory.json'),
        JSON.stringify({
          schema: 1,
          review: [{ pageId: 'page' }],
          promotion: [{ pageId: 'page' }]
        })
      )
      await writeFile(
        path.join(directory, 'live-source.json'),
        JSON.stringify({
          schema: 1,
          pages: {
            page: {
              hash: createHash('sha256').update('[]').digest('hex'),
              version: 'older-version'
            }
          }
        })
      )
    })
    it('should report no changes and avoid generation even in apply mode', async () => {
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
      expect((await pullLive(app, { apply: true, report: 'report.json' }, client)).changed).toBe(
        false
      )
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
    })
  })
  describe('and Figma rejects the credential scope', () => {
    beforeEach(() => {
      fetchFn.mockResolvedValueOnce(new Response('Missing file_content:read', { status: 403 }))
    })
    it('should preserve an actionable failure report without touching the saved capture', async () => {
      await expect(pullLive(app, { report: 'report.json' }, client)).rejects.toMatchObject({
        status: 403
      })
      expect(JSON.parse(await readFile(path.join(directory, 'report.json'), 'utf8'))).toMatchObject(
        { changed: false, error: expect.stringContaining('Missing file_content:read') }
      )
      expect(await readFile(path.join(directory, 'report.json'), 'utf8')).not.toContain(
        'FAKEKEY000example000000'
      )
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
    })
  })
  describe('and a formerly ready page loses its control', () => {
    beforeEach(() => {
      file.document.children![0]!.children = []
    })
    it('should fail instead of removing existing previews', async () => {
      fetchFn.mockResolvedValueOnce(Response.json(file))
      await expect(pullLive(app, { apply: true, report: 'report.json' }, client)).rejects.toThrow(
        'Previously ready page has no valid control'
      )
    })
  })
  describe('and a component has an unsupported structured mask', () => {
    beforeEach(() => {
      page.children = [
        {
          id: 'component',
          name: 'Masked',
          type: 'COMPONENT',
          absoluteBoundingBox: { x: 0, y: 0, width: 100, height: 20 },
          children: [
            {
              id: 'text',
              name: 'Text',
              type: 'TEXT',
              isMask: true,
              characters: 'Mask',
              absoluteBoundingBox: { x: 0, y: 0, width: 100, height: 20 }
            }
          ]
        }
      ]
    })
    it('should leave the saved capture untouched after failure', async () => {
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
      await expect(pullLive(app, { apply: true, report: 'report.json' }, client)).rejects.toThrow(
        'Unsupported structured mask'
      )
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
    })
  })
  describe('and the Colors page is ready', () => {
    const swatchNode = (
      id: string,
      label: string,
      value: string,
      fill: Record<string, number>
    ): LiveNode => ({
      id,
      name: 'Swatch',
      type: 'INSTANCE',
      children: [
        {
          id: `${id}:rect`,
          name: 'Rectangle 1',
          type: 'RECTANGLE',
          fills: [{ type: 'SOLID', color: { ...fill, a: 1 } }]
        },
        { id: `${id}:label`, name: 'label', type: 'TEXT', characters: label },
        { id: `${id}:value`, name: 'value', type: 'TEXT', characters: value }
      ]
    })
    const colorsPage = (
      value: string,
      fill: Record<string, number>,
      extra: LiveNode[] = [],
      label = 'Primary'
    ): LiveNode => ({
      id: 'colors',
      name: 'Colors',
      type: 'CANVAS',
      children: [
        {
          id: 'reds',
          name: 'Reds',
          type: 'FRAME',
          children: [
            {
              id: 'swatch',
              name: 'Swatch',
              type: 'INSTANCE',
              children: [
                {
                  id: 'rect',
                  name: 'Rectangle 1',
                  type: 'RECTANGLE',
                  fills: [{ type: 'SOLID', color: { ...fill, a: 1 } }]
                },
                { id: 'label', name: 'label', type: 'TEXT', characters: label },
                { id: 'value', name: 'value', type: 'TEXT', characters: value }
              ]
            },
            ...extra
          ]
        }
      ]
    })
    beforeEach(async () => {
      const config = JSON.parse(await readFile(path.join(directory, 'figma.config.json'), 'utf8'))
      config.pages.colors = { id: 'colors', name: 'Colors', kind: 'tokens' }
      await writeFile(path.join(directory, 'figma.config.json'), JSON.stringify(config))
      await writeFile(
        path.join(directory, 'variables.json'),
        JSON.stringify({
          collections: [
            {
              name: 'Primitives',
              modes: ['Value'],
              variables: [
                {
                  name: 'Primary/Primary',
                  type: 'COLOR',
                  values: { Value: { hex: '#ff2d55', alpha: 1 } }
                }
              ]
            }
          ]
        })
      )
      app = await createApp(directory)
      file.document.children!.push({
        id: 'colors',
        name: 'Colors',
        type: 'CANVAS',
        children: [
          {
            id: 'colors-marker',
            name: 'Sync status',
            type: 'INSTANCE',
            componentId: 'yes',
            componentProperties: { ready: { type: 'VARIANT', value: 'true' } }
          }
        ]
      })
    })
    it('should report the colour its swatch text and fill now show', async () => {
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
        .mockResolvedValueOnce(
          Response.json({
            nodes: { colors: { document: colorsPage('#0400FF', { r: 4 / 255, g: 0, b: 1 }) } }
          })
        )
      const report = await pullLive(app, { report: 'report.json' }, client)
      expect(report).toMatchObject({
        changed: true,
        changedPages: expect.arrayContaining(['Colors']),
        colorChanges: [{ kind: 'updated', name: 'Primary/Primary', from: '#ff2d55', to: '#0400ff' }]
      })
    })
    it('should report an excluded primitive without changing the saved colours', async () => {
      const config = JSON.parse(await readFile(path.join(directory, 'figma.config.json'), 'utf8'))
      config.excludedPrimitives = ['Primitives/Primary/Primary blue TEST']
      await writeFile(path.join(directory, 'figma.config.json'), JSON.stringify(config))
      const saved = JSON.stringify({
        collections: [
          {
            name: 'Primitives',
            modes: ['Value'],
            variables: [
              {
                name: 'Primary/Primary blue TEST',
                type: 'COLOR',
                values: { Value: { hex: '#0400ff', alpha: 1 } }
              }
            ]
          }
        ]
      })
      await writeFile(path.join(directory, 'variables.json'), saved)
      app = await createApp(directory)
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(
          Response.json({
            version: file.version,
            nodes: {
              page: {
                document: {
                  ...page,
                  children: [
                    ...page.children!,
                    { ...file.document.children![0]!.children![0]!, children: [] }
                  ]
                }
              }
            }
          })
        )
        .mockResolvedValueOnce(
          Response.json({
            version: file.version,
            nodes: {
              colors: {
                document: colorsPage('#00FF00', { r: 0, g: 1, b: 0 }, [], 'Primary blue TEST')
              }
            }
          })
        )
      const report = await pullLive(app, { report: 'report.json' }, client)
      expect(report.changedPages).not.toContain('Colors')
      expect(report.colorChanges).toEqual([{ kind: 'excluded', name: 'Primary/Primary blue TEST' }])
      expect(await readFile(path.join(directory, 'variables.json'), 'utf8')).toBe(saved)
    })
    it('should fail when a swatch text disagrees with its fill', async () => {
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
        .mockResolvedValueOnce(
          Response.json({
            nodes: {
              colors: { document: colorsPage('#0400FF', { r: 1, g: 45 / 255, b: 85 / 255 }) }
            }
          })
        )
      await expect(pullLive(app, { report: 'report.json' }, client)).rejects.toThrow(
        'Colors swatches need fixing in Figma'
      )
    })
    // Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
    const fileUrl = 'https://www.figma' + '.com/design/FAKEKEY000example000000/Leak'
    it.each([
      ['the configured key', 'FAKEKEY000example000000'],
      ['a Figma file URL', fileUrl]
    ])('should reject a swatch label naming %s before staging anything', async (_case, label) => {
      const variables = await readFile(path.join(directory, 'variables.json'), 'utf8')
      fetchFn
        .mockResolvedValueOnce(Response.json(file))
        .mockResolvedValueOnce(Response.json({ nodes: { page: { document: page } } }))
        .mockResolvedValueOnce(
          Response.json({
            nodes: {
              colors: {
                document: colorsPage('#FF2D55', { r: 1, g: 45 / 255, b: 85 / 255 }, [
                  swatchNode('leak', label, '#0400FF', { r: 4 / 255, g: 0, b: 1 })
                ])
              }
            }
          })
        )

      await expect(pullLive(app, { apply: true, report: 'report.json' }, client)).rejects.toThrow(
        'Colors swatch labels name the Figma file or its key'
      )

      const report = await readFile(path.join(directory, 'report.json'), 'utf8')
      expect(report).not.toContain('FAKEKEY000example000000')
      expect(report).not.toContain('figma.com')
      expect(await readFile(path.join(directory, 'variables.json'), 'utf8')).toBe(variables)
      expect(await readFile(path.join(directory, 'sync-state.json'), 'utf8')).toBe(previous)
      await expect(readFile(path.join(directory, 'generated/index.ts'))).rejects.toThrow()
    })
  })
})

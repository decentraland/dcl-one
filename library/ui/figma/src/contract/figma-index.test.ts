import { beforeEach, describe, expect, it, vi } from 'vitest'
import { configSchema, type FigmaConfig } from '../config.js'
import type { RawComponentPage } from '../ir/raw-components.js'
import type { FigmaSource } from '../sources/types.js'
import { buildFigmaIndex } from './figma-index.js'

const capture = (pageId: string, componentId: string): RawComponentPage => ({
  page: { id: pageId, name: 'Renamed in Figma' },
  components: [
    {
      kind: 'set',
      id: componentId,
      name: 'Button',
      section: [],
      x: 0,
      y: 0,
      properties: { Kind: { type: 'VARIANT', options: ['Primary', 'Secondary'] } },
      base: {
        id: `${componentId}:base`,
        name: 'Primary',
        props: { Kind: 'Primary' },
        tree: { type: 'FRAME', w: 10, h: 10 }
      },
      others: [
        { id: `${componentId}:other`, name: 'Secondary', props: { Kind: 'Secondary' }, patches: [] }
      ]
    }
  ]
})

describe('when indexing configured component pages for mapping verification', () => {
  let config: FigmaConfig
  let source: FigmaSource
  beforeEach(() => {
    config = configSchema.parse({
      fileKey: 'AAAAAAAAAAAAAAAAAAAAAA',
      outDir: 'out',
      pages: {
        '1:1': { id: '1:1', name: 'Shared name', kind: 'components' },
        '1:2': { id: '1:2', name: 'Shared name', kind: 'components' },
        '1:3': { id: '1:3', name: 'Icons', kind: 'icons' }
      }
    })
    source = {
      name: 'snapshot',
      listPages: vi.fn(),
      getTokenCollections: vi.fn(),
      getTextStyles: vi.fn(),
      getGradients: vi.fn(),
      getIcons: vi.fn(),
      getComponentPage: vi.fn(),
      getComponentPageById: vi.fn(async (id) => capture(id, id === '1:1' ? '2:1' : '2:2'))
    }
  })
  it('should iterate by stable IDs despite duplicate display names and renamed captures', async () => {
    const result = await buildFigmaIndex(source, config)
    expect(source.getComponentPageById).toHaveBeenNthCalledWith(1, '1:1', 'Shared name')
    expect(source.getComponentPageById).toHaveBeenNthCalledWith(2, '1:2', 'Shared name')
    expect(source.getComponentPageById).toHaveBeenCalledTimes(2)
    expect(source.getComponentPage).not.toHaveBeenCalled()
    expect(result.map((entry) => entry.setId)).toEqual(['2:1', '2:2'])
    expect(result[0]).toMatchObject({
      page: 'Shared name',
      name: 'Button',
      kind: 'set',
      properties: { Kind: { type: 'VARIANT', options: ['Primary', 'Secondary'] } },
      variants: [
        { id: '2:1:base', name: 'Primary', props: { Kind: 'Primary' } },
        { id: '2:1:other', name: 'Secondary', props: { Kind: 'Secondary' } }
      ]
    })
  })
  it('should reject a replacement identity returned for the configured ID', async () => {
    vi.mocked(source.getComponentPageById!).mockResolvedValueOnce(capture('9:9', '2:1'))
    await expect(buildFigmaIndex(source, config)).rejects.toThrow('Snapshot page identity changed')
  })
  it('should reject duplicate configured IDs before reading captures', async () => {
    config.pages['1:2']!.id = '1:1'
    await expect(buildFigmaIndex(source, config)).rejects.toThrow('Ambiguous configured page ID')
    expect(source.getComponentPageById).not.toHaveBeenCalled()
  })
  it('should retain name-based lookup only for an unresolved legacy entry', async () => {
    config.pages = { Legacy: { id: null, name: 'Legacy', kind: 'components' } }
    vi.mocked(source.getComponentPage).mockResolvedValueOnce(capture('7:7', '2:7'))
    expect((await buildFigmaIndex(source, config)).map((entry) => entry.setId)).toEqual(['2:7'])
    expect(source.getComponentPage).toHaveBeenCalledWith('Legacy')
    expect(source.getComponentPageById).not.toHaveBeenCalled()
  })
})

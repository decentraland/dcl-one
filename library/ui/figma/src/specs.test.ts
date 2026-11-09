import { addInventoryPages } from './generation-inventory.js'
import { rawComponentPageSchema } from './ir/raw-components.js'
import { beforeEach, describe, expect, it } from 'vitest'
import type { FigmaConfig } from './config.js'
import { snapshotSource, testConfig } from '../test/snapshot.js'
import type { FigmaSource } from './sources/types.js'
import { loadSpecsFromSource } from './specs.js'

describe('when generating from committed inventory', () => {
  let config: FigmaConfig
  let source: FigmaSource
  let state: Awaited<ReturnType<NonNullable<FigmaSource['getSyncState']>>>
  beforeEach(async () => {
    config = await testConfig()
    source = snapshotSource()
    state = await source.getSyncState!()
    source.getSyncState = async () => state
  })
  it('should preserve the intentional Review scope', async () => {
    expect(
      await loadSpecsFromSource(source, config).then(({ specs }) => ({
        dialogs: specs.filter((spec) => spec.page === 'Demo Dialogs').map((spec) => spec.figmaId),
        hasControls: specs.some((spec) => spec.page === 'Demo Controls')
      }))
    ).toEqual({ dialogs: ['30:1'], hasControls: false })
  })
  describe.each([false, null] as const)('and readiness becomes %s', (ready) => {
    beforeEach(() => {
      state!.pages.find((page) => page.name === 'Demo Nav')!.ready = ready
    })
    it('should retain the frozen page output', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some((spec) => spec.page === 'Demo Nav')
      ).toBe(true)
    })
  })
  describe('and the page display name changes', () => {
    beforeEach(() => {
      state!.pages.find((page) => page.name === 'Demo Nav')!.name = 'Demo Navigation ✅'
    })
    it('should retain the output name and selection', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some((spec) => spec.page === 'Demo Nav')
      ).toBe(true)
    })
  })
  describe('and the readiness state is missing', () => {
    beforeEach(() => {
      state = undefined
    })
    it('should preserve inventory selection', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some((spec) => spec.page === 'Demo Nav')
      ).toBe(true)
    })
  })
  describe('and capture readiness gating is disabled', () => {
    beforeEach(() => {
      config.selection = undefined
    })
    it('should still use committed publication inventory', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some(
          (spec) => spec.page === 'Demo Controls'
        )
      ).toBe(false)
    })
  })
  describe('and promotion is requested', () => {
    it('should keep the Review exception exclusively in Review', async () => {
      expect(
        (await loadSpecsFromSource(source, config, 'review')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(true)
      expect(
        (await loadSpecsFromSource(source, config, 'promotion')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(false)
    })
  })
  describe('and the exception page becomes ready without a new capture', () => {
    beforeEach(() => {
      state!.pages.find((page) => page.id === '10:8')!.ready = true
    })
    it('should keep the exception out of promotion without captured inventory', async () => {
      expect(
        (await loadSpecsFromSource(source, config, 'promotion')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(false)
    })
  })
  describe('and the ready exception page is captured into promotion inventory', () => {
    beforeEach(async () => {
      const inventory = await source.getGenerationInventory!()
      const captured = addInventoryPages(inventory!, ['10:8'], {
        '10:8': ['30:1']
      })
      source.getGenerationInventory = async () => captured
    })
    it('should admit the exception through the captured promotion scope', async () => {
      expect(
        (await loadSpecsFromSource(source, config, 'promotion')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(true)
    })
  })
  describe('and inventory is unavailable without readiness gating', () => {
    beforeEach(() => {
      config.selection!.readyOnly = false
      config.pages = Object.fromEntries(
        Object.entries(config.pages).filter(([, page]) => page.id === '10:8')
      )
      source.getGenerationInventory = async () => undefined
    })
    it('should not treat the Review exception as promotion authorization', async () => {
      expect(
        (await loadSpecsFromSource(source, config, 'review')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(true)
      expect(
        (await loadSpecsFromSource(source, config, 'promotion')).specs.some(
          (spec) => spec.figmaId === '30:1'
        )
      ).toBe(false)
    })
  })
  describe('and inventory is missing', () => {
    beforeEach(() => {
      source.getGenerationInventory = async () => undefined
    })
    it('should fail before generating output', async () => {
      await expect(loadSpecsFromSource(source, config)).rejects.toThrow(
        'Generation inventory required'
      )
    })
  })
  describe('and an inventory page is no longer configured', () => {
    beforeEach(async () => {
      const inventory = await source.getGenerationInventory!()
      source.getGenerationInventory = async () => ({
        ...inventory!,
        review: [...inventory!.review, { pageId: 'unknown' }]
      })
    })
    it('should reject the missing configuration rather than excluding published output', async () => {
      await expect(loadSpecsFromSource(source, config)).rejects.toThrow(
        'Inventory page is not configured'
      )
    })
  })
  describe('and an inventory snapshot is missing', () => {
    beforeEach(() => {
      source.getComponentPageById = async () => undefined
    })
    it('should fail instead of silently omitting the page', async () => {
      await expect(loadSpecsFromSource(source, config)).rejects.toThrow(
        'Missing inventory snapshot'
      )
    })
  })
  describe('and a component-only inventory entry names a missing component', () => {
    beforeEach(async () => {
      const inventory = await source.getGenerationInventory!()
      source.getGenerationInventory = async () => ({
        ...inventory!,
        review: [{ pageId: '10:8', componentIds: ['missing'] }],
        promotion: []
      })
    })
    it('should fail instead of silently shrinking the Review catalog', async () => {
      await expect(loadSpecsFromSource(source, config)).rejects.toThrow(
        'Missing inventory component'
      )
    })
  })
  describe('and a component has a permanent retirement record', () => {
    beforeEach(() => {
      source.getRetirements = async () => ({
        schema: 1,
        retired: [
          {
            pageId: '10:8',
            componentId: '30:1',
            name: 'Dialog',
            reason: 'Retired by developer',
            owned: [],
            sharedExclusions: []
          }
        ]
      })
    })
    it('should exclude it despite the Review inventory entry', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some((spec) => spec.figmaId === '30:1')
      ).toBe(false)
    })
  })
  describe('and a page has a permanent retirement record', () => {
    beforeEach(() => {
      source.getRetirements = async () => ({
        schema: 1,
        retired: [
          {
            pageId: '10:7',
            name: 'Demo Nav',
            reason: 'Retired by developer',
            owned: [],
            sharedExclusions: []
          }
        ]
      })
    })
    it('should exclude its output despite readiness and inventory', async () => {
      expect(
        (await loadSpecsFromSource(source, config)).specs.some((spec) => spec.page === 'Demo Nav')
      ).toBe(false)
    })
  })

  describe('and a retired duplicate was removed from its snapshot', () => {
    beforeEach(() => {
      config.pages = { '1:1': { id: '1:1', name: 'Page', kind: 'components' } }
      source.getGenerationInventory = async () => ({
        schema: 1,
        review: [{ pageId: '1:1' }],
        promotion: [{ pageId: '1:1' }]
      })
      source.getComponentPageById = async () =>
        rawComponentPageSchema.parse({
          page: { id: '1:1', name: 'Page' },
          components: [
            {
              kind: 'component',
              id: '2:2',
              name: 'Control',
              section: [],
              x: 0,
              y: 0,
              properties: {},
              base: {
                id: '2:2',
                name: 'Control',
                props: {},
                tree: { type: 'FRAME', w: 20, h: 20 }
              },
              others: []
            }
          ]
        })
      source.getRetirements = async () => ({
        schema: 1,
        retired: [
          {
            pageId: '1:1',
            componentId: '2:1',
            name: 'Control',
            reason: 'Retired duplicate',
            owned: ['figma/review/src/generated/components/manifest.ts#component:Control'],
            sharedExclusions: []
          }
        ]
      })
    })
    describe('and the duplicate was on an earlier retired page', () => {
      beforeEach(async () => {
        config.pages = {
          '1:1': { id: '1:1', name: 'Earlier', kind: 'components' },
          '1:2': { id: '1:2', name: 'Page', kind: 'components' }
        }
        source.getGenerationInventory = async () => ({
          schema: 1,
          review: [{ pageId: '1:1' }, { pageId: '1:2' }],
          promotion: [{ pageId: '1:1' }, { pageId: '1:2' }]
        })
        const getPage = source.getComponentPageById!
        source.getComponentPageById = async (id) => {
          const data = await getPage(id)
          if (!data) throw new Error('Missing fixture capture')
          return { ...data, page: { id: '1:2', name: 'Page' } }
        }
        const records = await source.getRetirements!()
        source.getRetirements = async () => ({
          ...records!,
          retired: [{ ...records!.retired[0]!, componentId: undefined }]
        })
      })
      it('should preserve the surviving public name without reordering allocation', async () => {
        expect((await loadSpecsFromSource(source, config)).specs.map((spec) => spec.name)).toEqual([
          'Control2'
        ])
      })
    })
    it('should preserve the surviving component public name', async () => {
      expect((await loadSpecsFromSource(source, config)).specs.map((spec) => spec.name)).toEqual([
        'Control2'
      ])
    })
  })
})

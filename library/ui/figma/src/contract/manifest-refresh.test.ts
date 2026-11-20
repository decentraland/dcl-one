import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { FigmaClient } from '../figma/client.js'
import type { FigmaSet } from './figma-index.js'
import {
  figmaSetFromNode,
  manifestDifferences,
  readMappedSets,
  variantProps,
  type RestNode
} from './manifest-refresh.js'

const setNode: RestNode = {
  id: '20:1',
  name: 'Button',
  type: 'COMPONENT_SET',
  componentPropertyDefinitions: {
    Hover: { type: 'VARIANT', defaultValue: 'No', variantOptions: ['No', 'Yes'] },
    'Show icon#12:3': { type: 'BOOLEAN', defaultValue: true },
    'Icon#12:4': { type: 'INSTANCE_SWAP', defaultValue: '9:9', variantOptions: undefined }
  },
  children: [
    { id: '20:2', name: 'Hover=No, Size=Large', type: 'COMPONENT' },
    { id: '20:3', name: 'Hover=Yes, Size=Large', type: 'COMPONENT' },
    { id: '20:9', name: 'A note', type: 'TEXT' }
  ]
}

describe('reading a mapped node from the live file', () => {
  describe('when the node is a component set', () => {
    let set: FigmaSet

    beforeEach(() => {
      set = figmaSetFromNode('20:1', setNode)
    })

    it('should keep the property definitions under their Figma names', () => {
      expect(set.properties).toEqual({
        Hover: { type: 'VARIANT', default: 'No', options: ['No', 'Yes'] },
        'Show icon#12:3': { type: 'BOOLEAN', default: true },
        'Icon#12:4': { type: 'INSTANCE_SWAP', default: '9:9' }
      })
    })

    it('should list only its component children as variants, with their properties', () => {
      expect(set.variants).toEqual([
        { id: '20:2', name: 'Hover=No, Size=Large', props: { Hover: 'No', Size: 'Large' } },
        { id: '20:3', name: 'Hover=Yes, Size=Large', props: { Hover: 'Yes', Size: 'Large' } }
      ])
    })

    it('should be a set named as in Figma', () => {
      expect(set).toMatchObject({ setId: '20:1', name: 'Button', kind: 'set' })
    })
  })

  describe('when the node is a single component', () => {
    it('should be its own only variant', () => {
      expect(figmaSetFromNode('3:40', { id: '3:40', name: 'Badge', type: 'COMPONENT' })).toEqual({
        setId: '3:40',
        name: 'Badge',
        page: '',
        kind: 'component',
        properties: {},
        variants: [{ id: '3:40', name: 'Badge', props: {} }]
      })
    })
  })

  describe('when the node cannot stand for a component', () => {
    it('should refuse a node the file does not have', () => {
      expect(() => figmaSetFromNode('1:1', undefined)).toThrow('Figma node 1:1 is not in the file')
    })

    it('should refuse a frame', () => {
      expect(() => figmaSetFromNode('1:1', { id: '1:1', name: 'Hero', type: 'FRAME' })).toThrow(
        'Figma node 1:1 ("Hero") is a FRAME, not a component or a component set'
      )
    })

    it('should refuse a component set with no variants', () => {
      expect(() =>
        figmaSetFromNode('1:1', { id: '1:1', name: 'Empty', type: 'COMPONENT_SET', children: [] })
      ).toThrow('Figma node 1:1 ("Empty") has no variants')
    })

    it('should refuse a property with no type', () => {
      expect(() =>
        figmaSetFromNode('1:1', {
          id: '1:1',
          name: 'Odd',
          type: 'COMPONENT',
          componentPropertyDefinitions: { Label: {} }
        })
      ).toThrow('Figma node 1:1: property "Label" has no type')
    })
  })

  describe('when a variant name is split into properties', () => {
    it('should trim names and values and keep an equals sign inside a value', () => {
      expect(variantProps(' Size = Large ,Formula=a=b')).toEqual({ Size: 'Large', Formula: 'a=b' })
    })

    it('should give no properties for a plain name', () => {
      expect(variantProps('Badge')).toEqual({})
    })
  })
})

describe('reading every mapped node', () => {
  let get: ReturnType<typeof vi.fn>
  let client: FigmaClient

  beforeEach(() => {
    get = vi.fn()
    client = { get } as unknown as FigmaClient
  })

  describe('when every node is returned', () => {
    let sets: FigmaSet[]

    beforeEach(async () => {
      get.mockResolvedValue({
        nodes: {
          '20:1': { document: setNode },
          '3:40': { document: { id: '3:40', name: 'Badge', type: 'COMPONENT' } }
        }
      })
      sets = await readMappedSets(client, 'KEY', ['20:1', '3:40'])
    })

    it('should ask for the ids in one request at depth one', () => {
      expect(get).toHaveBeenCalledTimes(1)
      expect(get).toHaveBeenCalledWith('/v1/files/KEY/nodes?ids=20%3A1,3%3A40&depth=1')
    })

    it('should return the sets in the order asked', () => {
      expect(sets.map((set) => set.name)).toEqual(['Button', 'Badge'])
    })
  })

  describe('when there are more ids than one request takes', () => {
    it('should ask in batches of fifty', async () => {
      const ids = Array.from({ length: 51 }, (_, index) => `1:${index + 1}`)
      get.mockImplementation(async (path: string) => ({
        nodes: Object.fromEntries(
          decodeURIComponent(path.split('ids=')[1]!.split('&')[0]!)
            .split(',')
            .map((id) => [id, { document: { id, name: `C${id}`, type: 'COMPONENT' } }])
        )
      }))

      expect(await readMappedSets(client, 'KEY', ids)).toHaveLength(51)
      expect(get).toHaveBeenCalledTimes(2)
    })
  })

  describe('when the file answers null for a node', () => {
    it('should fail the whole read naming the node', async () => {
      get.mockResolvedValue({ nodes: { '20:1': { document: setNode }, '3:40': null } })

      await expect(readMappedSets(client, 'KEY', ['20:1', '3:40'])).rejects.toThrow(
        'Figma node 3:40 is not in the file'
      )
    })
  })

  describe('when the request fails', () => {
    it('should pass the failure on', async () => {
      get.mockRejectedValue(new Error('Figma API 429'))

      await expect(readMappedSets(client, 'KEY', ['20:1'])).rejects.toThrow('Figma API 429')
    })
  })
})

describe('comparing the tracked manifest with a fresh read', () => {
  const tracked = figmaSetFromNode('20:1', setNode)

  it('should report nothing when they are equal', () => {
    expect(manifestDifferences([tracked], [figmaSetFromNode('20:1', setNode)])).toEqual([])
  })

  it('should report a rename, an added, a removed and a changed property', () => {
    const live: FigmaSet = {
      ...tracked,
      name: 'Button v2',
      properties: {
        Hover: { type: 'VARIANT', default: 'No', options: ['No', 'Yes', 'Pressed'] },
        'Icon#12:4': tracked.properties['Icon#12:4']!,
        Tone: { type: 'VARIANT', default: 'Calm', options: ['Calm'] }
      }
    }

    expect(manifestDifferences([tracked], [live])).toEqual([
      {
        setId: '20:1',
        name: 'Button v2',
        changes: [
          'renamed from "Button"',
          'property "Hover" changed',
          'property "Show icon#12:3" removed',
          'property "Tone" added'
        ]
      }
    ])
  })

  it('should report changed variants', () => {
    const live = { ...tracked, variants: tracked.variants.slice(0, 1) }

    expect(manifestDifferences([tracked], [live])[0]!.changes).toEqual(['variants changed'])
  })

  it('should report a live set the manifest lacks and a tracked set no mapping names', () => {
    const other = figmaSetFromNode('3:40', { id: '3:40', name: 'Badge', type: 'COMPONENT' })

    expect(manifestDifferences([tracked], [other])).toEqual([
      { setId: '3:40', name: 'Badge', changes: ['not in the manifest'] },
      { setId: '20:1', name: 'Button', changes: ['no mapping names it'] }
    ])
  })
})

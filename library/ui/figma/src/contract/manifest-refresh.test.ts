import { mkdtemp, readFile, readdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { FigmaClient } from '../figma/client.js'
import type { FigmaSet } from './figma-index.js'
import {
  parseMappingManifest,
  serializeMappingManifest,
  writeMappingManifest
} from './mapping-manifest.js'
import {
  figmaSetFromNode,
  manifestDifferences,
  mergeRefreshed,
  readLiveNodes,
  requireLiveSets,
  selectNodeIds,
  variantProps,
  verifyReport,
  type LiveRead,
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
        'Figma node 1:1 is FRAME, not a component or a component set'
      )
    })

    it('should refuse a component set with no variants', () => {
      expect(() =>
        figmaSetFromNode('1:1', { id: '1:1', name: 'Empty', type: 'COMPONENT_SET', children: [] })
      ).toThrow('Figma node 1:1 has no variants')
    })

    it('should refuse a property with no type', () => {
      expect(() =>
        figmaSetFromNode('1:1', {
          id: '1:1',
          name: 'Odd',
          type: 'COMPONENT',
          componentPropertyDefinitions: { Label: {} }
        })
      ).toThrow('Figma node 1:1: property 1 has no type')
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

// Built by concatenation so the tree holds no literal that looks like a Figma key.
const SECRET = 'aB3dE6gH9jK2' + 'mN5pQ8sT1v'

const idsOf = (path: string) => decodeURIComponent(path.split('ids=')[1]!.split('&')[0]!).split(',')

/** A client whose node read answers from `nodes` and whose page read finds only the ids in `onPages`. */
function fakeFile(
  nodes: Record<string, RestNode | null>,
  onPages: string[] = Object.keys(nodes)
): { get: ReturnType<typeof vi.fn>; client: FigmaClient } {
  const get = vi.fn(async (path: string) => {
    const ids = idsOf(path)
    if (path.includes('/nodes?'))
      return {
        nodes: Object.fromEntries(
          ids.map((id) => [id, nodes[id] === null ? null : { document: nodes[id] }])
        )
      }
    // The file read with ids holds the pages leading to each listed node that a page holds.
    return {
      document: {
        id: '0:0',
        children: [
          {
            id: '0:1',
            type: 'CANVAS',
            children: ids.filter((id) => onPages.includes(id)).map((id) => ({ id }))
          }
        ]
      }
    }
  })
  return { get, client: { get } as unknown as FigmaClient }
}

const badge: RestNode = { id: '3:40', name: 'Badge', type: 'COMPONENT' }

describe('reading the mapped nodes from the live file', () => {
  describe('when every node is a component on a page', () => {
    let get: ReturnType<typeof vi.fn>
    let read: LiveRead

    beforeEach(async () => {
      const file = fakeFile({ '20:1': setNode, '3:40': badge })
      get = file.get
      read = await readLiveNodes(file.client, 'KEY', ['20:1', '3:40'])
    })

    it('should read the nodes at depth one and then the pages that hold them', () => {
      expect(get.mock.calls.map(([path]) => path)).toEqual([
        '/v1/files/KEY/nodes?ids=20%3A1,3%3A40&depth=1',
        '/v1/files/KEY?ids=20%3A1,3%3A40'
      ])
    })

    it('should give the sets in the order asked and no problem', () => {
      expect(read.sets.map((set) => set.name)).toEqual(['Button', 'Badge'])
      expect(read.problems).toEqual([])
    })
  })

  describe('when there are more ids than one request takes', () => {
    it('should read nodes and pages in batches of fifty', async () => {
      const ids = Array.from({ length: 51 }, (_, index) => `1:${index + 1}`)
      const { get, client } = fakeFile(
        Object.fromEntries(ids.map((id) => [id, { id, name: `C${id}`, type: 'COMPONENT' }]))
      )

      expect(requireLiveSets(await readLiveNodes(client, 'KEY', ids))).toHaveLength(51)
      expect(get).toHaveBeenCalledTimes(4)
    })
  })

  describe('when the file answers null for a node', () => {
    it('should report it missing and not read pages for it', async () => {
      const { get, client } = fakeFile({ '3:40': null })

      const read = await readLiveNodes(client, 'KEY', ['3:40'])

      expect(read.problems).toEqual([
        { nodeId: '3:40', kind: 'missing', reason: 'Figma node 3:40 is not in the file' }
      ])
      expect(get).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the file resolves a node that no page holds', () => {
    let read: LiveRead

    beforeEach(async () => {
      // A deleted set keeps its id and definitions but answers without variants, as the real file does.
      const deleted: RestNode = { id: '9:1', name: SECRET, type: 'COMPONENT_SET', children: [] }
      const { client } = fakeFile({ '9:1': deleted, '3:40': badge }, ['3:40'])
      read = await readLiveNodes(client, 'KEY', ['9:1', '3:40'])
    })

    it('should report it deleted from the library instead of a set with no variants', () => {
      expect(read.problems).toEqual([
        {
          nodeId: '9:1',
          kind: 'deleted-from-library',
          reason:
            'Figma node 9:1 is deleted from the library: the file resolves it but no page holds it'
        }
      ])
    })

    it('should still give the live set beside it', () => {
      expect(read.sets.map((set) => set.setId)).toEqual(['3:40'])
    })
  })

  describe('when a node on a page is not a component', () => {
    it('should report it invalid without repeating its name or type', async () => {
      const { client } = fakeFile({ '1:1': { id: '1:1', name: SECRET, type: SECRET } })

      const read = await readLiveNodes(client, 'KEY', ['1:1'])

      expect(read.problems).toEqual([
        {
          nodeId: '1:1',
          kind: 'invalid',
          reason: 'Figma node 1:1 is another kind of node, not a component or a component set'
        }
      ])
    })
  })

  describe('when a request fails', () => {
    it('should pass a failed node read on', async () => {
      const get = vi.fn().mockRejectedValue(new Error('Figma API 429'))

      await expect(
        readLiveNodes({ get } as unknown as FigmaClient, 'KEY', ['20:1'])
      ).rejects.toThrow('Figma API 429')
    })

    it('should pass a failed page read on', async () => {
      const get = vi
        .fn()
        .mockResolvedValueOnce({ nodes: { '20:1': { document: setNode } } })
        .mockRejectedValueOnce(new Error('Figma API 500'))

      await expect(
        readLiveNodes({ get } as unknown as FigmaClient, 'KEY', ['20:1'])
      ).rejects.toThrow('Figma API 500')
    })

    it('should treat a page read with no document as no page holding the nodes', async () => {
      const get = vi
        .fn()
        .mockResolvedValueOnce({ nodes: { '20:1': { document: setNode } } })
        .mockResolvedValueOnce({})

      const read = await readLiveNodes({ get } as unknown as FigmaClient, 'KEY', ['20:1'])

      expect(read.problems.map((problem) => problem.kind)).toEqual(['deleted-from-library'])
    })
  })
})

describe('requiring every mapped node to give a set', () => {
  it('should name every problem with its class', () => {
    expect(() =>
      requireLiveSets({
        sets: [],
        problems: [
          { nodeId: '1:1', kind: 'missing', reason: 'Figma node 1:1 is not in the file' },
          { nodeId: '2:2', kind: 'deleted-from-library', reason: 'Figma node 2:2 is deleted' }
        ]
      })
    ).toThrow(
      '2 mapped Figma node(s) cannot be read:\n- [missing] Figma node 1:1 is not in the file\n- [deleted-from-library] Figma node 2:2 is deleted'
    )
  })

  it('should give the sets when there is no problem', () => {
    const set = figmaSetFromNode('3:40', badge)

    expect(requireLiveSets({ sets: [set], problems: [] })).toEqual([set])
  })
})

describe('choosing the nodes of a partial refresh', () => {
  const mappings = [
    { name: 'CloseButton', nodeId: '5:1' },
    { name: 'CloseButtonStyle', nodeId: '5:1' },
    { name: 'TitleBar', nodeId: '7:1' }
  ]

  it('should take a mapping name or a node id, each node once', () => {
    expect(selectNodeIds(mappings, ['TitleBar', '5:1', 'CloseButtonStyle'])).toEqual(['7:1', '5:1'])
  })

  it('should refuse a selector that matches no mapping', () => {
    expect(() => selectNodeIds(mappings, ['TitleBar', 'Titlebar'])).toThrow(
      '--only: no mapping is named or names the node "Titlebar"'
    )
  })

  it('should refuse a node id no mapping names', () => {
    expect(() => selectNodeIds(mappings, ['9:9'])).toThrow('names the node "9:9"')
  })

  it('should refuse an empty selection', () => {
    expect(() => selectNodeIds(mappings, [])).toThrow('--only takes at least one')
  })
})

describe('merging a partial refresh into the tracked manifest', () => {
  const a = figmaSetFromNode('3:40', badge)
  const b = figmaSetFromNode('20:1', setNode)
  const c = figmaSetFromNode('30:1', { id: '30:1', name: 'Chip', type: 'COMPONENT' })
  const entryText = (text: string, setId: string) =>
    JSON.stringify(
      (JSON.parse(text) as { sets: FigmaSet[] }).sets.find((set) => set.setId === setId)
    )

  it('should replace the refreshed entry and keep every other entry byte for byte', () => {
    const before = serializeMappingManifest([a, b, c])
    const refreshedB = { ...b, name: 'Button v2' }

    const { sets, dropped } = mergeRefreshed(
      parseMappingManifest(before),
      [refreshedB],
      ['3:40', '20:1', '30:1']
    )
    const after = serializeMappingManifest(sets)

    expect(dropped).toEqual([])
    expect(entryText(after, '20:1')).toContain('Button v2')
    expect(entryText(after, '3:40')).toBe(entryText(before, '3:40'))
    expect(entryText(after, '30:1')).toBe(entryText(before, '30:1'))
    expect(after).toBe(serializeMappingManifest([a, refreshedB, c]))
  })

  it('should add a newly mapped node and drop the entry its mapping no longer names', () => {
    const { sets, dropped } = mergeRefreshed([a, b], [c], ['3:40', '30:1'])

    expect(sets.map((set) => set.setId)).toEqual(['3:40', '30:1'])
    expect(dropped).toEqual(['20:1'])
  })

  it('should leave the file as it was when the merged manifest is refused', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'manifest-partial-'))
    const file = path.join(directory, 'figma-mapping-manifest.json')
    await writeMappingManifest(file, [a, b])
    const before = await readFile(file, 'utf8')

    const { sets } = mergeRefreshed([a, b], [{ ...b, name: `Button ${SECRET}` }], ['3:40', '20:1'])
    const error = await writeMappingManifest(file, sets).catch((caught: Error) => caught)

    expect((error as Error).message).not.toContain(SECRET)
    expect(await readFile(file, 'utf8')).toBe(before)
    expect(await readdir(directory)).toEqual(['figma-mapping-manifest.json'])
  })
})

describe('verifying the live file against the tracked manifest', () => {
  const button = figmaSetFromNode('20:1', setNode)
  const chip = figmaSetFromNode('30:1', { id: '30:1', name: 'Chip', type: 'COMPONENT' })
  const mappings = [
    { name: 'Button', nodeId: '20:1' },
    { name: 'Chip', nodeId: '30:1' },
    { name: 'Gone', nodeId: '40:1' },
    { name: 'Lost', nodeId: '50:1' },
    { name: 'NotMapped', nodeId: '60:1' }
  ]
  const tracked = [button, chip, { ...chip, setId: '40:1', name: 'Gone set' }]

  it('should sort every mapped node into one class, naming problems from the repository only', () => {
    const report = verifyReport(
      tracked,
      {
        sets: [button, { ...chip, name: 'Chip v2' }],
        problems: [
          { nodeId: '40:1', kind: 'deleted-from-library', reason: 'deleted' },
          { nodeId: '50:1', kind: 'missing', reason: 'missing' },
          { nodeId: '60:1', kind: 'invalid', reason: 'Figma node 60:1 has no variants' }
        ]
      },
      mappings
    )

    expect(report).toEqual({
      ok: false,
      sets: 5,
      clean: ['20:1'],
      changed: [{ setId: '30:1', name: 'Chip v2', changes: ['renamed from "Chip"'] }],
      deletedFromLibrary: [{ setId: '40:1', name: 'Gone set' }],
      missing: [{ setId: '50:1', name: 'Lost' }],
      invalid: [{ setId: '60:1', name: 'NotMapped', reason: 'Figma node 60:1 has no variants' }]
    })
  })

  it('should pass when every node is clean', () => {
    expect(verifyReport([button], { sets: [button], problems: [] }, mappings)).toMatchObject({
      ok: true,
      clean: ['20:1'],
      changed: []
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

describe('a refresh followed by a verify', () => {
  it('should find no difference when Figma lists variant properties in another order', () => {
    const live = figmaSetFromNode('20:1', {
      id: '20:1',
      name: 'Toggle',
      type: 'COMPONENT_SET',
      componentPropertyDefinitions: {
        Z: { type: 'VARIANT', defaultValue: 'On', variantOptions: ['On', 'Off'] },
        A: { type: 'VARIANT', defaultValue: 'Off', variantOptions: ['Off', 'On'] }
      },
      children: [{ id: '20:2', name: 'Z=On, A=Off', type: 'COMPONENT' }]
    })
    const tracked = parseMappingManifest(serializeMappingManifest([live]))

    expect(Object.keys(tracked[0]!.variants[0]!.props)).toEqual(['A', 'Z'])
    expect(manifestDifferences(tracked, [live])).toEqual([])
  })
})

describe('when the live file gives a name the manifest refuses', () => {
  const leaking = (where: 'name' | 'property' | 'variant'): RestNode => ({
    id: '20:1',
    name: where === 'name' ? `Card ${SECRET}` : 'Card',
    type: 'COMPONENT_SET',
    componentPropertyDefinitions: {
      [where === 'property' ? SECRET : 'Size']: {
        type: 'VARIANT',
        defaultValue: 'L',
        variantOptions: ['L']
      }
    },
    children: [
      { id: '20:2', name: where === 'variant' ? `Size=${SECRET}` : 'Size=L', type: 'COMPONENT' }
    ]
  })
  const tracked = [figmaSetFromNode('20:1', leaking('name'))].map((set) => ({
    ...set,
    name: 'Card'
  }))

  it.each(['name', 'property', 'variant'] as const)(
    'should refuse the %s in verify without repeating it',
    (where) => {
      const read = () => manifestDifferences(tracked, [figmaSetFromNode('20:1', leaking(where))])

      expect(read).toThrow(/The live file gave data the manifest refuses/)
      try {
        read()
      } catch (error) {
        expect((error as Error).message).not.toContain(SECRET)
      }
    }
  )

  it.each(['name', 'property', 'variant'] as const)(
    'should refuse the %s in refresh without repeating it and keep the previous manifest',
    async (where) => {
      const directory = await mkdtemp(path.join(tmpdir(), 'manifest-refresh-'))
      const file = path.join(directory, 'figma-mapping-manifest.json')
      await writeMappingManifest(file, tracked)
      const before = await readFile(file, 'utf8')

      const error = await writeMappingManifest(file, [
        figmaSetFromNode('20:1', leaking(where))
      ]).catch((caught: Error) => caught)

      expect(error).toBeInstanceOf(Error)
      expect((error as Error).message).not.toContain(SECRET)
      expect(await readFile(file, 'utf8')).toBe(before)
      expect(await readdir(directory)).toEqual(['figma-mapping-manifest.json'])
    }
  )

  it('should not repeat a refused node type or property name in a read error', () => {
    const errors = [
      () => figmaSetFromNode('1:1', { id: '1:1', name: SECRET, type: SECRET }),
      () =>
        figmaSetFromNode('1:1', {
          id: '1:1',
          name: SECRET,
          type: 'COMPONENT',
          componentPropertyDefinitions: { [SECRET]: {} }
        }),
      () =>
        figmaSetFromNode('1:1', { id: '1:1', name: SECRET, type: 'COMPONENT_SET', children: [] })
    ].map((read) => {
      try {
        read()
        return ''
      } catch (error) {
        return (error as Error).message
      }
    })

    expect(errors).toEqual([
      'Figma node 1:1 is another kind of node, not a component or a component set',
      'Figma node 1:1: property 1 has no type',
      'Figma node 1:1 has no variants'
    ])
  })
})

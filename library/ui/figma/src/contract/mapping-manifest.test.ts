import { mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { createRuntime } from '../runtime.js'
import type { FigmaSet } from './figma-index.js'
import { loadSource, checkMappingsReport, type ResolveContext } from './resolve.js'
import {
  loadMappingManifest,
  manifestPath,
  orphanedSets,
  parseMappingManifest,
  serializeMappingManifest,
  writeMappingManifest
} from './mapping-manifest.js'

const projectRoot = path.resolve(import.meta.dirname, '../..')

const button: FigmaSet = {
  setId: '20:1',
  name: 'Button',
  page: '',
  kind: 'set',
  properties: {
    Hover: { type: 'VARIANT', default: 'No', options: ['No', 'Yes'] },
    'Show icon#12:3': { type: 'BOOLEAN', default: true }
  },
  variants: [{ id: '20:2', name: 'Hover=No', props: { Hover: 'No' } }]
}
const badge: FigmaSet = {
  setId: '3:40',
  name: 'Badge',
  page: '',
  kind: 'component',
  properties: {},
  variants: [{ id: '3:40', name: 'Badge', props: {} }]
}
const manifestText = (sets: unknown[], schema: unknown = 1) => JSON.stringify({ schema, sets })

describe('the mapping manifest', () => {
  describe('when it is serialized', () => {
    it('should order sets by node id and properties by name, and read back unchanged', () => {
      const text = serializeMappingManifest([button, badge])

      expect(parseMappingManifest(text).map((set) => set.setId)).toEqual(['3:40', '20:1'])
      expect(Object.keys(parseMappingManifest(text)[1]!.properties)).toEqual([
        'Hover',
        'Show icon#12:3'
      ])
      expect(serializeMappingManifest(parseMappingManifest(text))).toBe(text)
    })

    it('should give the same bytes whatever order the sets arrive in', () => {
      expect(serializeMappingManifest([badge, button])).toBe(
        serializeMappingManifest([button, badge])
      )
    })

    it('should refuse a set whose name carries a key-shaped string', () => {
      expect(() =>
        serializeMappingManifest([{ ...badge, name: 'Badge aB3dE6gH9jK2mN5pQ8sT1v' }])
      ).toThrow(/key-shaped string at sets\[0\]\.name/)
    })
  })

  describe('when it is parsed', () => {
    it('should refuse text that is not JSON', () => {
      expect(() => parseMappingManifest('{')).toThrow(/is not valid JSON/)
    })

    it('should refuse another schema number', () => {
      expect(() => parseMappingManifest(manifestText([badge], 2))).toThrow(/schema at schema/)
    })

    it('should refuse a field the schema does not name', () => {
      expect(() => parseMappingManifest(manifestText([{ ...badge, fileKey: 'x' }]))).toThrow(
        /schema at sets\[0\] \(unrecognized_keys\)/
      )
    })

    it('should refuse a set id that is not a node id', () => {
      expect(() => parseMappingManifest(manifestText([{ ...badge, setId: '3-40' }]))).toThrow(
        /schema at sets\[0\]\.setId/
      )
    })

    it('should refuse a set with no variants', () => {
      expect(() => parseMappingManifest(manifestText([{ ...badge, variants: [] }]))).toThrow(
        /schema at sets\[0\]\.variants/
      )
    })

    it('should refuse the same node listed twice', () => {
      expect(() => parseMappingManifest(manifestText([badge, badge]))).toThrow(
        /lists Figma node 3:40 twice/
      )
    })

    it('should refuse a Figma URL in any value', () => {
      const url = 'https://www.figma' + '.com/design/x/y'
      expect(() =>
        parseMappingManifest(
          manifestText([{ ...button, properties: { Hover: { type: 'VARIANT', default: url } } }])
        )
      ).toThrow(/Figma URL or a key-shaped string at sets\[0\]\.properties\.<name>\.default/)
    })

    it('should refuse a key-shaped property name', () => {
      expect(() =>
        parseMappingManifest(
          manifestText([{ ...badge, properties: { aB3dE6gH9jK2mN5pQ8sT1v: { type: 'TEXT' } } }])
        )
      ).toThrow(/key-shaped string/)
    })

    it.each([
      [
        'a property name',
        { ...badge, properties: { ['aB3dE6gH9jK2' + 'mN5pQ8sT1v']: { type: 'TEXT' } } }
      ],
      [
        'a variant property name',
        {
          ...badge,
          variants: [{ id: '3:40', name: 'x', props: { ['aB3dE6gH9jK2' + 'mN5pQ8sT1v']: 'x' } }]
        }
      ],
      ['a value the schema refuses', { ...badge, kind: 'aB3dE6gH9jK2' + 'mN5pQ8sT1v' }],
      ['a key under an unknown field', { ...badge, ['aB3dE6gH9jK2' + 'mN5pQ8sT1v']: 1 }]
    ])('should not repeat %s in its diagnostic', (_label, set) => {
      let message = ''
      try {
        parseMappingManifest(manifestText([set]))
      } catch (error) {
        message = (error as Error).message
      }

      expect(message).not.toBe('')
      expect(message).not.toContain('aB3dE6gH9jK2' + 'mN5pQ8sT1v')
    })

    it('should accept an empty list of sets', () => {
      expect(parseMappingManifest(manifestText([]))).toEqual([])
    })
  })

  describe('when it is read from and written to disk', () => {
    let directory: string
    let file: string

    beforeEach(async () => {
      directory = await mkdtemp(path.join(tmpdir(), 'mapping-manifest-'))
      file = manifestPath(path.join(directory, 'components'))
    })

    it('should sit beside the mapping root', () => {
      expect(file).toBe(path.join(directory, 'figma-mapping-manifest.json'))
    })

    it('should name the refresh command when the file is missing', async () => {
      await expect(loadMappingManifest(file)).rejects.toThrow(
        /is missing: run "pnpm figma manifest refresh"/
      )
    })

    it('should read back what it wrote and leave no staging file', async () => {
      await writeMappingManifest(file, [button])

      expect(await loadMappingManifest(file)).toEqual([button])
      expect(await readdir(directory)).toEqual(['figma-mapping-manifest.json'])
    })

    it('should keep the previous file when the new content is refused', async () => {
      await writeMappingManifest(file, [button])
      const before = await readFile(file, 'utf8')

      await expect(writeMappingManifest(file, [button, button])).rejects.toThrow(/twice/)

      expect(await readFile(file, 'utf8')).toBe(before)
      expect(await readdir(directory)).toEqual(['figma-mapping-manifest.json'])
    })

    it('should refuse a file edited into another shape', async () => {
      await writeFile(file, manifestText([{ ...badge, kind: 'frame' }]))

      await expect(loadMappingManifest(file)).rejects.toThrow(/schema at sets\[0\]\.kind/)
    })
  })

  describe('when entries are compared with the mapped nodes', () => {
    it('should name the entries no mapping points at', () => {
      expect(orphanedSets([button, badge], ['20:1']).map((set) => set.setId)).toEqual(['3:40'])
    })

    it('should name none when every entry is mapped', () => {
      expect(orphanedSets([button, badge], ['3:40', '20:1', '9:9'])).toEqual([])
    })
  })
})

describe('check-mappings against a mapping manifest', () => {
  let capture: ResolveContext
  let mapped: FigmaSet[]

  beforeEach(async () => {
    const runtime = await createRuntime({
      cwd: projectRoot,
      env: {},
      configPath: 'test/fixtures/figma.config.json'
    })
    capture = await runtime.resolveContext()
    const source = await loadSource(capture)
    if ('error' in source) throw new Error(source.error.message)
    const nodeIds = source.mappings.flatMap((mapping) =>
      mapping.figma ? [mapping.figma.nodeId] : []
    )
    mapped = capture.figmaSets.filter((set) => nodeIds.includes(set.setId))
  })

  const report = (figmaSets: FigmaSet[]) =>
    checkMappingsReport({ ...capture, figmaSets, identitySource: 'manifest' })
  const errors = async (figmaSets: FigmaSet[]) =>
    (await report(figmaSets)).results.flatMap((result) =>
      result.diagnostics
        .filter((diagnostic) => diagnostic.severity === 'error')
        .map((diagnostic) => `${result.file}: ${diagnostic.code}`)
    )

  describe('when the manifest holds exactly the mapped sets', () => {
    it('should pass and say the identities came from the manifest', async () => {
      expect(mapped.length).toBeGreaterThan(0)
      expect(await report(mapped)).toMatchObject({
        ok: true,
        captureChecked: true,
        identitySource: 'manifest',
        warnings: []
      })
    })
  })

  describe('when a mapped set is absent from the manifest', () => {
    it('should fail with identity-not-found for that mapping', async () => {
      const result = await report(mapped.slice(1))

      expect(result.ok).toBe(false)
      expect(
        result.results.flatMap((entry) => entry.diagnostics.map((diagnostic) => diagnostic.message))
      ).toContainEqual(expect.stringContaining('is not in the mapping manifest'))
    })
  })

  describe('when the manifest is empty', () => {
    it('should fail every mapping instead of passing unchecked', async () => {
      const result = await report([])

      expect(result).toMatchObject({ ok: false, captureChecked: true, warnings: [] })
      expect(
        (await errors([])).filter((error) => error.endsWith('identity-not-found'))
      ).toHaveLength(mapped.length)
    })
  })

  describe('when the manifest holds a set no mapping names', () => {
    it('should fail and name the stale entry', async () => {
      const stale = capture.figmaSets.find((set) => !mapped.includes(set))!

      expect(await errors([...mapped, stale])).toEqual([
        'figma-mapping-manifest.json: identity-not-found'
      ])
    })
  })

  describe('when a mapped property was renamed in the manifest', () => {
    it('should report the drift as an error', async () => {
      const [first, ...rest] = mapped
      const [name, definition] = Object.entries(first!.properties)[0]!
      const renamed = {
        ...first!,
        properties: { ...first!.properties, [`${name} renamed`]: definition }
      }
      delete (renamed.properties as Record<string, unknown>)[name]

      expect(await errors([renamed, ...rest])).toContainEqual(
        expect.stringMatching(/: (contract-incompatible|figma-drift)$/)
      )
    })
  })
})

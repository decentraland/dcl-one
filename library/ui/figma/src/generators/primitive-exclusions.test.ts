import { createHash } from 'node:crypto'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { beforeAll, describe, expect, it } from 'vitest'
import { configSchema, loadConfigFile } from '../config.js'
import type { TokenCollection } from '../ir/schemas.js'
import { SnapshotSource } from '../sources/snapshot-source.js'
import { copySnapshot, snapshotSource, testConfig } from '../../test/snapshot.js'
import { generateAll } from './index.js'
import { excludePrimitives, generateTokens, SUPPORTED_PRIMITIVE_EXCLUSIONS } from './tokens.js'
import type { GeneratedFile } from './types.js'

const context = {}
const EXCLUDED = 'Primitives/Primary/Primary blue TEST'
const TARGETS = ['tokens/tokens.css', 'tokens/tokens.ts', 'tokens/theme.ts']

// The synthetic fixture has no test primitive, so each case adds it the way the capture saved it,
// beside a sibling in the same group, as the capture keeps Primary/Primary there (the synthetic fixture's
// Brand/Primary already takes that CSS name).
function withTestPrimitive(collections: TokenCollection[]): TokenCollection[] {
  return collections.map((collection) =>
    collection.name === 'Primitives'
      ? {
          ...collection,
          variables: [
            ...collection.variables,
            {
              name: 'Primary/Primary red',
              type: 'COLOR' as const,
              values: { Value: { hex: '#ff2d55', alpha: 1 } }
            },
            {
              name: 'Primary/Primary blue TEST',
              type: 'COLOR' as const,
              values: { Value: { hex: '#0400ff', alpha: 1 } }
            }
          ]
        }
      : collection
  )
}

const asMap = (files: GeneratedFile[]) => new Map(files.map((file) => [file.path, file.content]))

describe('excludedPrimitives', () => {
  let collections: TokenCollection[]
  let input: Parameters<typeof generateTokens>[0]

  beforeAll(async () => {
    const source = snapshotSource()
    collections = withTestPrimitive(await source.getTokenCollections())
    input = {
      collections,
      textStyles: await source.getTextStyles(),
      gradients: await source.getGradients()
    }
  })

  describe('when the config does not set it', () => {
    it('should default to an empty list', () => {
      const config = configSchema.parse({ outDir: 'out', pages: {} })

      expect(config.excludedPrimitives).toEqual([])
    })

    it('should generate byte-identical tokens to a run without exclusions', () => {
      const before = asMap(generateTokens(input, context))
      const after = asMap(generateTokens({ ...input, excludedPrimitives: [] }, context))

      expect([...after.entries()]).toEqual([...before.entries()])
    })
  })

  describe('when the supported primitive is excluded', () => {
    let before: Map<string, string>
    let after: Map<string, string>

    beforeAll(() => {
      before = asMap(generateTokens(input, context))
      after = asMap(generateTokens({ ...input, excludedPrimitives: [EXCLUDED] }, context))
    })

    it('should be the only supported exclusion', () => {
      expect(SUPPORTED_PRIMITIVE_EXCLUSIONS).toEqual([EXCLUDED])
    })

    it.each(TARGETS)('should drop it from %s', (target) => {
      expect(before.get(target)).toMatch(/primary-blue-test|primaryBlueTest/)
      expect(after.get(target)).not.toMatch(/primary-blue-test|primaryBlueTest/)
    })

    // The token was the last of its group, so the entry before it loses its trailing comma.
    it.each(TARGETS)('should leave every other line of %s unchanged', (target) => {
      const lines = (text: string) => text.split('\n').map((line) => line.replace(/,$/, ''))
      const removed = lines(before.get(target)!).filter(
        (line) => !/primary-blue-test|primaryBlueTest/.test(line)
      )

      expect(lines(after.get(target)!)).toEqual(removed)
    })

    it('should keep the other generated token files unchanged', () => {
      for (const [file, content] of before)
        if (!TARGETS.includes(file)) expect(after.get(file)).toBe(content)
    })

    it('should not change the collections it was given', () => {
      const copy = structuredClone(collections)

      excludePrimitives(collections, [EXCLUDED])

      expect(collections).toEqual(copy)
    })
  })

  describe('when an entry is not usable', () => {
    it('should reject a name the capture does not have', () => {
      const without = collections.map((collection) => ({
        ...collection,
        variables: collection.variables.filter(
          (variable) => variable.name !== 'Primary/Primary blue TEST'
        )
      }))

      expect(() => excludePrimitives(without, [EXCLUDED])).toThrow(
        `Excluded primitive ${EXCLUDED} is not a saved variable`
      )
    })

    it('should reject a name listed twice', () => {
      expect(() => excludePrimitives(collections, [EXCLUDED, EXCLUDED])).toThrow(
        `Excluded primitive ${EXCLUDED} is listed more than once`
      )
    })

    it.each([
      'Primitives/Primary/Primary',
      'Primitives/Primary/primary blue test',
      'Semantic/bg/page',
      'Primary/Primary blue TEST'
    ])('should reject %s, which is not a supported exclusion', (name) => {
      expect(() => excludePrimitives(collections, [name])).toThrow(
        `Excluded primitive ${name} is not a supported exclusion`
      )
    })

    it('should fail closed through generation too', () => {
      expect(() =>
        generateTokens({ ...input, excludedPrimitives: ['Primitives/Primary/Primary'] }, context)
      ).toThrow('is not a supported exclusion')
    })
  })

  describe('when the config file is loaded', () => {
    const load = async (excludedPrimitives: string[]) => {
      const directory = await mkdtemp(path.join(os.tmpdir(), 'excluded-config-'))
      try {
        const file = path.join(directory, 'figma.config.json')
        await writeFile(file, JSON.stringify({ outDir: 'out', pages: {}, excludedPrimitives }))
        return await loadConfigFile(file)
      } finally {
        await rm(directory, { recursive: true, force: true })
      }
    }

    it('should accept the supported exclusion', async () => {
      expect((await load([EXCLUDED])).excludedPrimitives).toEqual([EXCLUDED])
    })

    it.each([['*'], ['Primitives/Primary/Primary']])(
      'should reject %s before any operation runs',
      async (name) => {
        await expect(load([name])).rejects.toThrow(
          `Excluded primitive ${name} is not a supported exclusion`
        )
      }
    )

    it('should reject the supported exclusion listed twice', async () => {
      await expect(load([EXCLUDED, EXCLUDED])).rejects.toThrow(
        `Excluded primitive ${EXCLUDED} is listed more than once`
      )
    })
  })

  describe('when the saved capture is ambiguous', () => {
    it('should reject a primitive saved twice instead of removing both', () => {
      const twice = collections.map((collection) =>
        collection.name === 'Primitives'
          ? {
              ...collection,
              variables: [
                ...collection.variables,
                {
                  name: 'Primary/Primary blue TEST',
                  type: 'COLOR' as const,
                  values: { Value: { hex: '#00ff00', alpha: 1 } }
                }
              ]
            }
          : collection
      )
      const copy = structuredClone(twice)

      expect(() => excludePrimitives(twice, [EXCLUDED])).toThrow(
        `Excluded primitive ${EXCLUDED} is saved 2 times`
      )
      expect(twice).toEqual(copy)
    })

    it('should reject two Primitives collections', () => {
      const primitives = collections.find((collection) => collection.name === 'Primitives')!

      expect(() => excludePrimitives([...collections, primitives], [EXCLUDED])).toThrow(
        'Excluded primitives are ambiguous: 2 Primitives collections'
      )
    })

    it('should give the same result when applied again to the same input', () => {
      expect(excludePrimitives(collections, [EXCLUDED])).toEqual(
        excludePrimitives(collections, [EXCLUDED])
      )
    })
  })

  describe('when an alias depends on the excluded primitive', () => {
    it('should fail and name every dependent alias', () => {
      const dependent = collections.map((collection) =>
        collection.name === 'Semantic'
          ? {
              ...collection,
              variables: [
                ...collection.variables,
                ...['test/one', 'test/two'].map((name) => ({
                  name,
                  type: 'COLOR' as const,
                  values: {
                    Dark: { alias: 'Primary/Primary blue TEST', collection: 'Primitives' }
                  }
                }))
              ]
            }
          : collection
      )

      expect(() => excludePrimitives(dependent, [EXCLUDED])).toThrow(
        `Excluded primitive ${EXCLUDED} is used by aliases: Semantic/test/one, Semantic/test/two`
      )
    })
  })

  describe('when only a later mode of an alias depends on the excluded primitive', () => {
    it('should still fail', () => {
      const multiMode = [
        ...collections,
        {
          name: 'Themes',
          modes: ['Light', 'Dark'],
          variables: [
            {
              name: 'accent',
              type: 'COLOR' as const,
              values: {
                Light: { alias: 'Primary/Primary', collection: 'Primitives' },
                Dark: { alias: 'Primary/Primary blue TEST', collection: 'Primitives' }
              }
            }
          ]
        }
      ]

      expect(() => excludePrimitives(multiMode, [EXCLUDED])).toThrow(
        `Excluded primitive ${EXCLUDED} is used by aliases: Themes/accent`
      )
    })
  })

  describe('when sync generates from the saved capture', () => {
    // This tree commits no capture: a copy of the synthetic capture saves the test primitive as the
    // real capture does, and generation must leave that file untouched.
    it('should leave variables.json byte for byte as captured', async () => {
      const directory = await mkdtemp(path.join(os.tmpdir(), 'primitive-exclusions-'))
      try {
        await copySnapshot(directory)
        const file = path.join(directory, 'variables.json')
        const saved = JSON.parse(await readFile(file, 'utf8')) as { collections: TokenCollection[] }
        await writeFile(file, JSON.stringify({ collections: withTestPrimitive(saved.collections) }))
        const digest = async () =>
          createHash('sha256')
            .update(await readFile(file))
            .digest('hex')
        const config = await testConfig()
        const before = await digest()

        const result = await generateAll(new SnapshotSource(directory), context, {
          ...config,
          excludedPrimitives: [EXCLUDED]
        })

        expect(await digest()).toBe(before)
        expect(asMap(result.files).get('tokens/tokens.css')).not.toContain('primary-blue-test')
      } finally {
        await rm(directory, { recursive: true, force: true })
      }
    })
  })
})

import { beforeAll, describe, expect, it } from 'vitest'
import { configSchema, type FigmaConfig } from '../config.js'
import { rawComponentPageSchema, type RawComponentPage } from '../ir/raw-components.js'
import type { TokenCollection } from '../ir/schemas.js'
import type { FigmaSource } from '../sources/types.js'
import { loadSpecsFromSource } from '../specs.js'
import { snapshotSource } from '../../test/snapshot.js'
import { generateAll } from './index.js'

const context = {}
const EXCLUDED = 'Primitives/Primary/Primary blue TEST'
const BLUE = '#0400ff'

const card = (paints: Record<string, unknown>) => ({
  kind: 'component',
  id: '1:2',
  name: 'BlueCard',
  section: [],
  x: 0,
  y: 0,
  properties: {},
  base: {
    id: '1:2',
    name: 'BlueCard',
    props: {},
    tree: { type: 'FRAME', name: 'BlueCard', w: 20, h: 20, ...paints }
  },
  others: []
})

const solid = (token?: string) => ({
  type: 'SOLID',
  hex: BLUE,
  opacity: 1,
  ...(token && { token })
})

function source(collections: TokenCollection[], paints: Record<string, unknown>): FigmaSource {
  const page: RawComponentPage = rawComponentPageSchema.parse({
    page: { id: '9:1', name: 'Cards' },
    components: [card(paints)]
  })
  return {
    name: 'snapshot',
    listPages: async () => [],
    getTokenCollections: async () => collections,
    getTextStyles: async () => [],
    getGradients: async () => [],
    getIcons: async () => [],
    getComponentPage: async (name) => (name === 'Cards' ? page : undefined)
  }
}

const config = (excludedPrimitives: string[]): FigmaConfig =>
  configSchema.parse({
    outDir: 'out',
    pages: { '9:1': { id: '9:1', name: 'Cards', kind: 'components' } },
    excludedPrimitives
  })

const css = async (paints: Record<string, unknown>, excluded: string[]) => {
  const { files } = await generateAll(source(collections, paints), context, config(excluded))
  return files
    .filter((file) => file.path.includes('/BlueCard'))
    .map((file) => file.content)
    .join('\n')
}

let collections: TokenCollection[]

beforeAll(async () => {
  collections = (await snapshotSource().getTokenCollections()).map((collection) =>
    collection.name === 'Primitives'
      ? {
          ...collection,
          variables: [
            ...collection.variables,
            {
              name: 'Primary/Primary blue TEST',
              type: 'COLOR' as const,
              values: { Value: { hex: BLUE, alpha: 1 } }
            }
          ]
        }
      : collection
  )
})

describe('when a component is bound to an excluded primitive', () => {
  it.each([
    ['fill', { fills: [solid(EXCLUDED)] }],
    ['stroke', { fills: [], strokes: [solid(EXCLUDED)] }],
    [
      'gradient stop',
      {
        fills: [
          {
            type: 'GRADIENT_LINEAR',
            stops: [
              { position: 0, hex: BLUE, alpha: 1, token: EXCLUDED },
              { position: 1, hex: '#ffffff', alpha: 1 }
            ]
          }
        ]
      }
    ]
  ])('should fail generation for a bound %s and name the component', async (_, paints) => {
    await expect(
      generateAll(source(collections, paints), context, config([EXCLUDED]))
    ).rejects.toThrow(
      `Component BlueCard (1:2) on page Cards is bound to excluded primitive ${EXCLUDED}`
    )
  })

  it('should fail the promotion spec route the same way', async () => {
    await expect(
      loadSpecsFromSource(
        source(collections, { fills: [solid(EXCLUDED)] }),
        config([EXCLUDED]),
        'promotion'
      )
    ).rejects.toThrow('is bound to excluded primitive')
  })

  it('should generate the binding when nothing is excluded', async () => {
    expect(await css({ fills: [solid(EXCLUDED)] }, [])).toContain(
      'var(--dcl-primary-primary-blue-test)'
    )
  })
})

describe('when an unbound colour equals an excluded primitive', () => {
  it('should not value-match the excluded primitive', async () => {
    const output = await css({ fills: [solid()] }, [EXCLUDED])

    expect(output).not.toContain('primary-blue-test')
    expect(output.toLowerCase()).toContain(BLUE)
  })

  it('should value-match it when nothing is excluded', async () => {
    expect(await css({ fills: [solid()] }, [])).toContain('var(--dcl-primary-primary-blue-test)')
  })
})

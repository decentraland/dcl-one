import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'

// The read-only use_figma script of the consumer skill, run against hand-built Plugin API nodes.
const skill = await readFile(
  path.join(import.meta.dirname, 'skills/implement-figma-design/SKILL.md'),
  'utf8'
)
const block = /\n {3}```js\n([\s\S]*?)\n {3}```\n/.exec(skill)![1]!
const source = block
  .split('\n')
  .map((line) => line.replace(/^ {3}/, ''))
  .join('\n')
const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor as new (
  ...args: string[]
) => (figma: unknown) => Promise<unknown>
const script = new AsyncFunction('figma', source)

type Node = {
  type: string
  name: string
  visible?: boolean
  characters?: string
  children?: Node[]
  componentPropertyReferences?: { characters?: string }
}
type Property = { type: string; value: string | boolean }

const text = (name: string, characters: string, extra: Partial<Node> = {}): Node => ({
  type: 'TEXT',
  name,
  characters,
  ...extra
})

function run(
  children: Node[],
  componentProperties: Record<string, Property> = {},
  main: unknown = { name: 'Button', parent: { type: 'COMPONENT_SET', name: 'Button' } }
) {
  const instance = {
    type: 'INSTANCE',
    name: 'Button',
    children,
    componentProperties,
    getMainComponentAsync: async () => main
  }
  return script({ getNodeByIdAsync: async () => instance }) as Promise<Record<string, unknown>>
}

describe('the skill plugin script', () => {
  it('should put VARIANT, BOOLEAN and TEXT properties in separate maps without their #id suffix', async () => {
    const result = await run(
      [text('Label', 'Buy', { componentPropertyReferences: { characters: 'Label#1:2' } })],
      {
        Kind: { type: 'VARIANT', value: 'Primary' },
        'Show icon#3:4': { type: 'BOOLEAN', value: false },
        'Label#1:2': { type: 'TEXT', value: 'Buy' },
        'Icon#5:6': { type: 'INSTANCE_SWAP', value: '7:8' }
      }
    )

    expect(result).toEqual({
      name: 'Button',
      variantProperties: { Kind: 'Primary' },
      booleanProperties: { 'Show icon': 'false' },
      texts: { Label: 'Buy' },
      duplicateTexts: []
    })
  })

  it('should keep the visible text when a hidden layer shows a TEXT property of the same name', async () => {
    const result = await run(
      [
        text('Label', 'Placeholder', {
          visible: false,
          componentPropertyReferences: { characters: 'Label#1:2' }
        }),
        text('Label', 'Buy')
      ],
      { 'Label#1:2': { type: 'TEXT', value: 'Placeholder' } }
    )

    expect(result).toMatchObject({ texts: { Label: 'Buy' }, duplicateTexts: [] })
  })

  it('should leave out a TEXT property that only a hidden layer shows', async () => {
    const result = await run(
      [
        text('Caption', 'Hidden', {
          visible: false,
          componentPropertyReferences: { characters: 'Tip#1:2' }
        })
      ],
      { 'Tip#1:2': { type: 'TEXT', value: 'Hidden' } }
    )

    expect(result).toMatchObject({ texts: {}, duplicateTexts: [] })
  })

  it('should name a TEXT property after a visible layer with another name that shows it', async () => {
    const result = await run(
      [text('Caption', 'Go', { componentPropertyReferences: { characters: 'Label#1:2' } })],
      { 'Label#1:2': { type: 'TEXT', value: 'Go' } }
    )

    expect(result).toMatchObject({ texts: { Caption: 'Go', Label: 'Go' }, duplicateTexts: [] })
  })

  // The nearest false positive of the visibility rule: two visible texts still disagree.
  it('should drop and report a name two visible layers show differently, however often it repeats', async () => {
    const result = await run([text('Label', 'Buy'), text('Label', 'Sell'), text('Label', 'Buy')])

    expect(result).toMatchObject({ texts: {}, duplicateTexts: ['Label'] })
  })

  it('should report a TEXT property name that a visible layer of that name contradicts', async () => {
    const result = await run(
      [
        text('Caption', 'Go', { componentPropertyReferences: { characters: 'Label#1:2' } }),
        text('Label', 'Stop')
      ],
      { 'Label#1:2': { type: 'TEXT', value: 'Go' } }
    )

    expect(result).toMatchObject({ texts: { Caption: 'Go' }, duplicateTexts: ['Label'] })
  })

  it('should accept two visible layers that share a name and a text', async () => {
    const result = await run([text('Label', 'Buy'), text('Label', 'Buy')])

    expect(result).toMatchObject({ texts: { Label: 'Buy' }, duplicateTexts: [] })
  })

  it('should skip text under a hidden frame and inside a nested instance', async () => {
    const result = await run([
      { type: 'FRAME', name: 'Tooltip', visible: false, children: [text('Tip', 'Hidden')] },
      { type: 'INSTANCE', name: 'Badge', children: [text('Label', 'Nested')] },
      { type: 'FRAME', name: 'Body', children: [text('Label', 'Own')] }
    ])

    expect(result).toMatchObject({ texts: { Label: 'Own' }, duplicateTexts: [] })
  })

  it('should keep a layer named like an Object method as plain data', async () => {
    const result = (await run([text('constructor', 'A'), text('toString', 'B')])) as {
      texts: Record<string, string>
      duplicateTexts: string[]
    }

    expect({ ...result.texts }).toEqual({ constructor: 'A', toString: 'B' })
    expect(result.duplicateTexts).toEqual([])
  })

  it('should take the main component name when it is not in a set', async () => {
    expect(await run([], {}, { name: 'Logo', parent: { type: 'PAGE' } })).toMatchObject({
      name: 'Logo'
    })
  })

  it.each([
    ['a node that is not an instance', { type: 'FRAME' }, 'not-an-instance'],
    ['an instance without a main component', undefined, 'no-main-component']
  ])('should refuse %s', async (_label, node, refused) => {
    const instance = node ?? {
      type: 'INSTANCE',
      children: [],
      componentProperties: {},
      getMainComponentAsync: async () => null
    }

    expect(await script({ getNodeByIdAsync: async () => instance })).toEqual({ refused })
  })
})

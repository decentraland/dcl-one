import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'

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
  let children: Node[]
  let properties: Record<string, Property>
  let result: Record<string, unknown>
  beforeEach(() => {
    children = []
    properties = {}
  })

  describe('when the instance has VARIANT, BOOLEAN, TEXT and INSTANCE_SWAP properties', () => {
    beforeEach(async () => {
      children = [
        text('Label', 'Buy', { componentPropertyReferences: { characters: 'Label#1:2' } })
      ]
      properties = {
        Kind: { type: 'VARIANT', value: 'Primary' },
        'Show icon#3:4': { type: 'BOOLEAN', value: false },
        'Label#1:2': { type: 'TEXT', value: 'Buy' },
        'Icon#5:6': { type: 'INSTANCE_SWAP', value: '7:8' }
      }
      result = await run(children, properties)
    })

    it('should put VARIANT, BOOLEAN and TEXT properties in separate maps without their #id suffix', () => {
      expect(result).toEqual({
        name: 'Button',
        variantProperties: { Kind: 'Primary' },
        booleanProperties: { 'Show icon': 'false' },
        texts: { Label: 'Buy' },
        duplicateTexts: []
      })
    })
  })

  describe('when a hidden layer shows a TEXT property named like a visible layer', () => {
    beforeEach(async () => {
      children = [
        text('Label', 'Placeholder', {
          visible: false,
          componentPropertyReferences: { characters: 'Label#1:2' }
        }),
        text('Label', 'Buy')
      ]
      properties = { 'Label#1:2': { type: 'TEXT', value: 'Placeholder' } }
      result = await run(children, properties)
    })

    it('should keep the visible text', () => {
      expect(result).toMatchObject({ texts: { Label: 'Buy' }, duplicateTexts: [] })
    })
  })

  describe('when only a hidden layer shows a TEXT property', () => {
    beforeEach(async () => {
      children = [
        text('Caption', 'Hidden', {
          visible: false,
          componentPropertyReferences: { characters: 'Tip#1:2' }
        })
      ]
      properties = { 'Tip#1:2': { type: 'TEXT', value: 'Hidden' } }
      result = await run(children, properties)
    })

    it('should leave the TEXT property out', () => {
      expect(result).toMatchObject({ texts: {}, duplicateTexts: [] })
    })
  })

  describe('when a visible layer with another name shows a TEXT property', () => {
    beforeEach(async () => {
      children = [
        text('Caption', 'Go', { componentPropertyReferences: { characters: 'Label#1:2' } })
      ]
      properties = { 'Label#1:2': { type: 'TEXT', value: 'Go' } }
      result = await run(children, properties)
    })

    it('should name the TEXT property after both the layer and the property', () => {
      expect(result).toMatchObject({ texts: { Caption: 'Go', Label: 'Go' }, duplicateTexts: [] })
    })
  })

  // The nearest false positive of the visibility rule: two visible texts still disagree.
  describe('when two visible layers show a name differently, however often it repeats', () => {
    beforeEach(async () => {
      children = [text('Label', 'Buy'), text('Label', 'Sell'), text('Label', 'Buy')]
      result = await run(children)
    })

    it('should drop and report the name', () => {
      expect(result).toMatchObject({ texts: {}, duplicateTexts: ['Label'] })
    })
  })

  describe('when a visible layer contradicts a TEXT property of its name', () => {
    beforeEach(async () => {
      children = [
        text('Caption', 'Go', { componentPropertyReferences: { characters: 'Label#1:2' } }),
        text('Label', 'Stop')
      ]
      properties = { 'Label#1:2': { type: 'TEXT', value: 'Go' } }
      result = await run(children, properties)
    })

    it('should report the TEXT property name', () => {
      expect(result).toMatchObject({ texts: { Caption: 'Go' }, duplicateTexts: ['Label'] })
    })
  })

  describe('when two visible layers share a name and a text', () => {
    beforeEach(async () => {
      children = [text('Label', 'Buy'), text('Label', 'Buy')]
      result = await run(children)
    })

    it('should accept the shared name', () => {
      expect(result).toMatchObject({ texts: { Label: 'Buy' }, duplicateTexts: [] })
    })
  })

  describe('when text sits under a hidden frame and inside a nested instance', () => {
    beforeEach(async () => {
      children = [
        { type: 'FRAME', name: 'Tooltip', visible: false, children: [text('Tip', 'Hidden')] },
        { type: 'INSTANCE', name: 'Badge', children: [text('Label', 'Nested')] },
        { type: 'FRAME', name: 'Body', children: [text('Label', 'Own')] }
      ]
      result = await run(children)
    })

    it('should skip that text', () => {
      expect(result).toMatchObject({ texts: { Label: 'Own' }, duplicateTexts: [] })
    })
  })

  describe('when a layer is named like an Object method', () => {
    let texts: { texts: Record<string, string>; duplicateTexts: string[] }
    beforeEach(async () => {
      children = [text('constructor', 'A'), text('toString', 'B')]
      texts = (await run(children)) as typeof texts
    })

    it('should keep the layer as plain data', () => {
      expect({ ...texts.texts }).toEqual({ constructor: 'A', toString: 'B' })
    })

    it('should report no duplicate text', () => {
      expect(texts.duplicateTexts).toEqual([])
    })
  })

  describe('when the main component is not in a set', () => {
    beforeEach(async () => {
      result = await run(children, properties, { name: 'Logo', parent: { type: 'PAGE' } })
    })

    it('should take the main component name', () => {
      expect(result).toMatchObject({ name: 'Logo' })
    })
  })

  describe.each([
    ['a node that is not an instance', { type: 'FRAME' }, 'not-an-instance'],
    ['an instance without a main component', undefined, 'no-main-component']
  ])('when the selection is %s', (_label, node, refused) => {
    let instance: unknown
    beforeEach(() => {
      instance = node ?? {
        type: 'INSTANCE',
        children: [],
        componentProperties: {},
        getMainComponentAsync: async () => null
      }
    })

    it(`should refuse it as ${refused}`, async () => {
      expect(await script({ getNodeByIdAsync: async () => instance })).toEqual({ refused })
    })
  })
})

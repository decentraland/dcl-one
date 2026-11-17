import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { parseFigmaUrl, parseMappingSource, readMappingFile } from './mapping-file.js'
import { mappingSource } from './test-fixtures.js'

const buttonMapping = path.resolve(
  import.meta.dirname,
  '../../test/fixtures/library/components/Button/Button.figma.ts'
)

// Built by concatenation so the tree never holds a literal Figma file URL (figma-key-guard `url`).
const FIGMA_DESIGN_URL = 'https://www.figma' + '.com/design/'

describe('parseFigmaUrl', () => {
  describe('when the url names the file through the placeholder', () => {
    it('should return the placeholder and the colon form of the node id', () => {
      expect(parseFigmaUrl('<DCL_FIGMA_FILE>?node-id=20-1&p=f')).toEqual({
        fileKey: '<DCL_FIGMA_FILE>',
        nodeId: '20:1'
      })
    })
  })

  describe('when the url is a literal Figma URL', () => {
    it('should flag it without keeping the file key', () => {
      expect(parseFigmaUrl(`${FIGMA_DESIGN_URL}AbC123/Some-Name?node-id=20-1&p=f`)).toEqual({
        fileKey: 'literal-figma-url',
        nodeId: '20:1'
      })
    })
  })

  describe('when the url has no node-id', () => {
    it('should return undefined', () => {
      expect(parseFigmaUrl('<DCL_FIGMA_FILE>')).toBeUndefined()
      expect(parseFigmaUrl(`${FIGMA_DESIGN_URL}AbC123/Some-Name`)).toBeUndefined()
    })
  })
})

describe('parseMappingSource', () => {
  describe('when reading the real Button template', () => {
    it('should extract header, locals, example and imports without issues', async () => {
      const mapping = await readMappingFile(buttonMapping)

      expect(mapping.issues).toEqual([])
      expect(mapping.figma).toEqual({ fileKey: '<DCL_FIGMA_FILE>', nodeId: '20:1' })
      expect(mapping.header).toMatchObject({
        component: 'Button',
        contract: '1',
        states: ['Hover']
      })
      expect(mapping.locals.map((l) => l.kind)).toEqual(['enum', 'enum', 'text'])
      expect(mapping.imports).toEqual(["import { Button } from '@dcl/one-ui'"])
      expect(mapping.example).toContain('<Button variant="${variant}"')
    })
  })

  describe('when reading the template id', () => {
    it.each([
      ['single quotes', "id: 'button'", 'button'],
      ['double quotes, as the generator writes it', 'id: "button"', 'button'],
      ['an empty id', "id: ''", undefined]
    ])('should read it written with %s', (_label, line, expected) => {
      const mapping = parseMappingSource(
        mappingSource().replace("id: 'button'", line),
        'B.figma.ts'
      )

      expect(mapping.id).toBe(expected)
    })

    // The nearest false positive: a nodeId property is not the template id.
    it('should not take a nodeId property for the id', () => {
      const source = mappingSource()
        .replace("id: 'button',", '')
        .replace('metadata: {', "metadata: { nodeId: 'x',")

      expect(parseMappingSource(source, 'B.figma.ts').id).toBeUndefined()
    })
  })

  describe('when the template calls an API outside the supported subset', () => {
    it('should report unsupported-construct instead of ignoring it', () => {
      const mapping = parseMappingSource(
        mappingSource({ body: "const icon = instance.getInstanceSwap('Icon')\n" }),
        'Button.figma.ts'
      )

      expect(mapping.issues.map((issue) => issue.code)).toContain('unsupported-construct')
    })
  })

  describe('when getEnum values are not string literals', () => {
    it('should report unsupported-construct', () => {
      const source = mappingSource().replace("Primary: 'primary'", 'Primary: compute()')

      const mapping = parseMappingSource(source, 'Button.figma.ts')

      expect(mapping.issues.map((issue) => issue.code)).toContain('unsupported-construct')
    })
  })

  describe('when the url header is missing', () => {
    it('should report a mapping that cannot be tied to a Figma node', () => {
      const source = mappingSource().replace(/^\/\/ url=.*$/m, '')

      const mapping = parseMappingSource(source, 'Button.figma.ts')

      expect(mapping.figma).toBeUndefined()
      expect(mapping.issues).toHaveLength(1)
    })
  })
})

describe('when a text placeholder is inside a JSX JavaScript expression', () => {
  it('should reject the unsupported context instead of emitting entity text in JavaScript', () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</Button>',
      " aria-label={'${label}'}>Label</Button>"
    )
    expect(
      parseMappingSource(source, 'Button.figma.ts').issues.map((issue) => issue.code)
    ).toContain('unsupported-construct')
  })
})

describe('when a text placeholder follows a literal brace in a quoted JSX attribute', () => {
  it('should retain the supported JSX string context', () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</Button>',
      ' title="{${label}}">Label</Button>'
    )
    expect(parseMappingSource(source, 'Button.figma.ts').issues).toEqual([])
  })
})

import { mkdir, mkdtemp, rename, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { FigmaSet } from './figma-index.js'
import {
  checkMappings,
  checkMappingsReport,
  resolveComponent,
  type ResolveContext
} from './resolve.js'
import { buttonSet, makeLibrary, mappingSource } from './test-fixtures.js'

type Result = Awaited<ReturnType<typeof resolveComponent>>
type Check = Awaited<ReturnType<typeof checkMappings>>

const created: string[] = []

async function library(options: Parameters<typeof makeLibrary>[0] = {}) {
  const made = await makeLibrary(options)
  created.push(made.root)
  return made
}

const context = (mappingRoot: string, extra: Partial<ResolveContext> = {}): ResolveContext => ({
  mappingRoot,
  figmaSets: [buttonSet],
  ...extra
})

const lock = (revision: string) =>
  JSON.stringify({
    sources: [
      {
        repository: 'https://github.com/decentraland/figma-design-system',
        revision: 'a'.repeat(40),
        snapshot: { revision, digests: { 'pages.json': 'b'.repeat(64) } }
      }
    ]
  })

const codes = (result: Result) => (result.ok ? [] : result.errors.map((error) => error.code))

afterEach(async () => {
  await Promise.all(created.splice(0).map((dir) => rm(dir, { recursive: true, force: true })))
})

describe('resolveComponent', () => {
  let result: Result

  describe('when the component is mapped and the variant is supported', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot), {
        nodeId: '1-1',
        variantProperties: { Kind: 'Secondary' }
      })
    })

    it('should resolve', () => {
      expect(result.ok).toBe(true)
    })

    it('should return the translated props in the usage', () => {
      expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
    })

    it('should return the import', () => {
      expect(result.ok && result.resolved.component.importStatements).toEqual([
        "import { Button } from 'fixture-library'"
      ])
    })

    it('should return the provenance', () => {
      expect(result.ok && result.resolved.provenance).toMatchObject({
        figma: { setId: '1:1', identity: 'verified-against-capture' },
        library: { name: 'fixture-library', version: '1.2.0' },
        mapping: { file: 'src/Button.figma.ts', id: 'button' }
      })
    })
  })

  describe('when the node id is the main component of an instance (a variant)', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:3' })
    })

    it('should resolve the owning set and use the variant properties', () => {
      expect(result.ok && result.resolved.usage).toBe(
        '<Button kind="primary" disabled>Label</Button>'
      )
    })
  })

  describe('when no mapping exists for a captured Figma component', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({ mappings: {} })
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
    })

    it('should report mapping-missing and not generate anything', () => {
      expect(codes(result)).toEqual(['mapping-missing'])
    })
  })

  describe('when a mapping only matches the component by name', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '9-9' }) }
      })
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
    })

    it('should still report mapping-missing', () => {
      expect(codes(result)).toEqual(['mapping-missing'])
    })

    it('should list the same-name mapping as a hint', () => {
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        sameNameMappings: [{ targets: '9:9' }]
      })
    })
  })

  describe('when two Figma components share the requested name', () => {
    let twin: FigmaSet

    beforeEach(async () => {
      const { mappingRoot } = await library()
      twin = { ...buttonSet, setId: '2:1', page: 'Other' }
      result = await resolveComponent(context(mappingRoot, { figmaSets: [buttonSet, twin] }), {
        name: 'Button'
      })
    })

    it('should report identity-ambiguous', () => {
      expect(codes(result)).toEqual(['identity-ambiguous'])
    })

    it('should list the candidates', () => {
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        candidates: [{ nodeId: '1:1' }, { nodeId: '2:1' }]
      })
    })
  })

  describe('when two mappings claim the same Figma node', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: {
          'Button.figma.ts': mappingSource(),
          'Other.figma.ts': mappingSource({ id: 'other' })
        }
      })
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
    })

    it('should report identity-ambiguous', () => {
      expect(codes(result)).toEqual(['identity-ambiguous'])
    })
  })

  describe('when the Figma variant exists but the mapping does not cover it', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ kindValues: { Primary: 'primary' } }) }
      })
      result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Secondary' }
      })
    })

    it('should report variant-unsupported', () => {
      expect(codes(result)).toEqual(['variant-unsupported'])
    })

    it('should list the supported values', () => {
      expect(!result.ok && result.errors[0]!.details).toMatchObject({ supported: ['Primary'] })
    })
  })

  describe('when the requested combination is not defined in the design', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Secondary', Blocked: 'Yes' }
      })
    })

    it('should report variant-not-in-figma', () => {
      expect(codes(result)).toEqual(['variant-not-in-figma'])
    })

    it('should list the nearest variants', () => {
      expect(!result.ok && result.errors[0]!.details?.nearest).toBeDefined()
    })
  })

  describe('when a variant value does not exist in Figma at all', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Ghost' }
      })
    })

    it('should report variant-not-in-figma', () => {
      expect(codes(result)).toEqual(['variant-not-in-figma'])
    })
  })

  describe('when the installed library does not satisfy the required range', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot, { libraryRange: '^2.0.0' }), {
        nodeId: '1:1'
      })
    })

    it('should report contract-incompatible', () => {
      expect(codes(result)).toEqual(['contract-incompatible'])
    })

    it('should give the library-version reason', () => {
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        reason: 'library-version',
        installed: '1.2.0'
      })
    })
  })

  describe.each([
    ['an unknown contract version', { contract: '2' }, 'contract-version'],
    ['a literal Figma URL', { literalUrl: true }, 'wrong-figma-file'],
    ['an import from a different package', { importFrom: 'someone-else' }, 'import-path-mismatch']
  ])('when the mapping declares %s', (_label, options, reason) => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource(options) }
      })
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
    })

    it(`should report contract-incompatible (${reason})`, () => {
      expect(!result.ok && result.errors.map((e) => e.details?.reason)).toContain(reason)
    })
  })

  describe('when the component is not exported by the package entry', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({ entry: "export { Other } from './Other'\n" })
      result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
    })

    it('should report contract-incompatible (export-missing)', () => {
      expect(!result.ok && result.errors[0]!.details).toMatchObject({ reason: 'export-missing' })
    })
  })

  describe('when Figma gained an option the mapping does not list', () => {
    let grown: FigmaSet

    beforeEach(async () => {
      const { mappingRoot } = await library()
      grown = {
        ...buttonSet,
        properties: {
          ...buttonSet.properties,
          Kind: { type: 'VARIANT', default: 'Primary', options: ['Primary', 'Secondary', 'Ghost'] }
        }
      }
      result = await resolveComponent(context(mappingRoot, { figmaSets: [grown] }), {
        nodeId: '1:1'
      })
    })

    it('should resolve', () => {
      expect(result.ok).toBe(true)
    })

    it('should warn about the drift', () => {
      expect(result.warnings.map((w) => w.code)).toContain('figma-drift')
    })
  })

  describe('when no capture is available', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1-1',
        variantProperties: { Kind: 'Secondary' }
      })
    })

    it('should resolve from the mapping alone', () => {
      expect(result.ok && result.resolved.provenance.figma.identity).toBe('mapping-only')
    })

    it('should say the identity is unverified', () => {
      expect(result.warnings.map((w) => w.code)).toContain('figma-identity-unverified')
    })
  })

  describe('when identity cannot be established', () => {
    describe('and an unmapped node has no capture', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '7:7'
        })
      })

      it('should report identity-uncertain, not mapping-missing', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })

      it('should tell the caller not to treat it as a missing mapping', () => {
        expect(!result.ok && result.errors[0]!.action).toMatch(
          /do not treat it as a missing mapping/i
        )
      })
    })

    describe('and a name alone matches a mapping', () => {
      beforeEach(async () => {
        const { root } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), lock('e'.repeat(40)))
        result = await resolveComponent(context(path.join(root, 'src'), { figmaSets: [] }), {
          name: 'Button'
        })
      })

      it('should report identity-uncertain', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })

      it('should name the variant properties the mapping reads', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.details).toEqual({
          reason: 'name-unconfirmed',
          reads: ['Kind', 'Blocked'],
          name: 'Button',
          component: 'Button',
          captureRevision: 'e'.repeat(40)
        })
      })

      it('should ask for variant properties', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.action).toMatch(/Resolve again with the instance variantProperties/)
      })

      it('should not ask for a report', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.action).not.toMatch(/Report/)
      })
    })

    describe('and a node id matches no set without a capture', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:2'
        })
      })

      it('should send the caller to the set name', () => {
        expect(!result.ok && result.errors[0]!.action).toMatch(
          /resolve again with the component set name and variantProperties/
        )
      })
    })

    describe('and several mappings claim one node', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: {
            'Button.figma.ts': mappingSource(),
            'Copy.figma.ts': mappingSource({ id: 'copy' })
          }
        })
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should report identity-ambiguous', () => {
        expect(codes(result)).toEqual(['identity-ambiguous'])
      })

      it('should list the claiming files', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.details).toMatchObject({
          reason: 'mappings',
          nodeId: '1:1',
          files: ['src/Button.figma.ts', 'src/Copy.figma.ts']
        })
      })

      it('should report it as a library defect to report', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.action).toMatch(/library defect/)
      })
    })

    describe('and a node outside the capture has no mapping', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot), { nodeId: '7:7' })
      })

      it('should report identity-not-found', () => {
        expect(codes(result)).toEqual(['identity-not-found'])
      })
    })

    describe('and the name is unknown in the capture', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot), { name: 'Nothing' })
      })

      it('should report identity-not-found', () => {
        expect(codes(result)).toEqual(['identity-not-found'])
      })
    })
  })

  describe('when a captured component has no mapping', () => {
    beforeEach(async () => {
      const { root, mappingRoot } = await library({
        mappings: { 'Other.figma.ts': mappingSource({ nodeId: '5-5', component: 'Other' }) }
      })
      await writeFile(path.join(root, 'upstream.lock.json'), lock('c'.repeat(40)))
      result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Primary' }
      })
    })

    it('should report mapping-missing', () => {
      expect(codes(result)).toEqual(['mapping-missing'])
    })

    it('should give the contributor report context', () => {
      const [error] = !result.ok ? result.errors : []
      expect(error!.details).toMatchObject({
        nodeId: '1:1',
        name: 'Button',
        variantProperties: { Kind: 'Primary' },
        library: { name: 'fixture-library', version: '1.2.0' },
        captureRevision: 'c'.repeat(40),
        supported: ['Other']
      })
    })

    it('should tell the caller not to build a replacement', () => {
      const [error] = !result.ok ? result.errors : []
      expect(error!.action).toMatch(/Do not build a replacement/)
    })

    it('should give no Figma URL or key', () => {
      expect(JSON.stringify(result)).not.toMatch(/figma\.com|AAAAAAAAAAAAAAAAAAAAAA/)
    })
  })

  describe('when the library tracks a capture lock', () => {
    describe('and the lock pins a capture revision', () => {
      beforeEach(async () => {
        const { root, mappingRoot } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), lock('d'.repeat(40)))
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1-1'
        })
      })

      it('should name the pinned capture revision in the provenance without a capture', () => {
        expect(result.ok && result.resolved.provenance.capture).toEqual({
          revision: 'd'.repeat(40)
        })
      })
    })

    describe('and the library has no lock', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot), { nodeId: '1-1' })
      })

      it('should omit the capture', () => {
        expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
      })
    })

    describe.each([
      ['a short revision', lock('abc')],
      ['no figma-design-system source', JSON.stringify({ sources: [] })],
      ['text that is not JSON', '{not json']
    ])('and the lock has %s', (_label, text) => {
      beforeEach(async () => {
        const { root, mappingRoot } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), text)
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1-1'
        })
      })

      it('should resolve', () => {
        expect(result.ok).toBe(true)
      })

      it('should warn with capture-revision-unknown', () => {
        expect(result.warnings.map((w) => w.code)).toContain('capture-revision-unknown')
      })

      it('should omit the capture', () => {
        expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
      })
    })
  })

  describe('when a component is identified by its set name without a capture', () => {
    // A mapping that reads no variant property: nothing but the name can confirm it.
    const noVariants = () =>
      mappingSource()
        .replace(/const kind = instance\.getEnum\('Kind', \{[\s\S]*?\}\)\n/, '')
        .replace(/const disabled = [^\n]*\n/, '')
        .replace('kind="${kind}"${disabled}', '')
    const byName = async (name: string, componentNames?: Record<string, string>) => {
      const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': noVariants() } })
      return resolveComponent(context(mappingRoot, { figmaSets: [], componentNames }), { name })
    }
    const named = (name: string, variantProperties?: Record<string, string>, extra = {}) =>
      library().then(({ mappingRoot }) =>
        resolveComponent(context(mappingRoot, { figmaSets: [], ...extra }), {
          name,
          variantProperties
        })
      )
    const withBooleans = (
      variantProperties: Record<string, string>,
      booleanProperties: Record<string, string>
    ) =>
      library().then(({ mappingRoot }) =>
        resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          name: 'Button',
          variantProperties,
          booleanProperties
        })
      )

    describe('and the instance leaves out a mapped variant property', () => {
      beforeEach(async () => {
        result = await named('Button', { Kind: 'Secondary' })
      })

      it('should name the mapped variant property the instance did not give', () => {
        expect(
          result.warnings.find((w) => w.code === 'figma-identity-unverified')?.details
        ).toEqual({
          combinationChecked: false,
          notGiven: ['Blocked']
        })
      })
    })

    // The nearest false positive: every mapped property given leaves nothing to name.
    describe('and the instance gives every mapped variant property', () => {
      beforeEach(async () => {
        result = await named('Button', { Kind: 'Primary', Blocked: 'No' })
      })

      it('should name nothing', () => {
        expect(
          result.warnings.find((w) => w.code === 'figma-identity-unverified')?.details
        ).toEqual({
          combinationChecked: false,
          notGiven: []
        })
      })
    })

    describe('and an alias names a mapping that reads no variant property', () => {
      beforeEach(async () => {
        result = await byName('Component 1', { 'Component 1': 'Button' })
      })

      it('should refuse it as identity-uncertain', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })

      it('should report the alias as unconfirmed', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-unconfirmed',
          name: 'Component 1'
        })
      })
    })

    // The nearest false positive: the mapping's own component name still resolves it.
    describe('and the component name of a mapping that reads no variant property is given', () => {
      beforeEach(async () => {
        result = await byName('Button', { 'Component 1': 'Button' })
      })

      it('should resolve it', () => {
        expect(result.ok && result.resolved.usage).toBe('<Button >Label</Button>')
      })
    })

    describe.each([
      ['the exported component name', 'Button', {}],
      ['the template id', 'button', {}],
      [
        'a componentNames alias',
        'Button-Contained-Text',
        { componentNames: { 'Button-Contained-Text': 'Button' } }
      ]
    ])('and %s comes with variant properties that fit', (_label, name, extra) => {
      beforeEach(async () => {
        result = await named(name, { Kind: 'Secondary', Hover: 'No' }, extra)
      })

      it('should resolve the usage', () => {
        expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
      })

      it('should record a mapping-only identity', () => {
        expect(result.ok && result.resolved.provenance.figma.identity).toBe('mapping-only')
      })

      it('should warn that the identity is an unverified name match', () => {
        expect(result.warnings.map((w) => w.code)).toEqual(
          expect.arrayContaining(['figma-identity-unverified', 'identity-name-match'])
        )
      })
    })

    describe('and a property the mapping does not read is given', () => {
      beforeEach(async () => {
        result = await named('Button', { Kind: 'Primary', Size: 'Large' })
      })

      it('should refuse it as identity-uncertain', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })

      it('should report it as incompatible', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-incompatible',
          unknown: ['Size'],
          unsupported: []
        })
      })
    })

    describe('and an unread boolean property is given', () => {
      beforeEach(async () => {
        result = await withBooleans({ Kind: 'Secondary' }, { 'Show icon': 'false' })
      })

      it('should resolve', () => {
        expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
      })

      it('should warn that the mapping does not read it', () => {
        expect(result.warnings).toContainEqual(
          expect.objectContaining({
            code: 'unmapped-figma-property',
            details: expect.objectContaining({ property: 'Show icon' })
          })
        )
      })
    })

    // The nearest false positive of the boolean rule: a two-state VARIANT the mapping does not read.
    describe.each([['true'], ['false'], ['True']])(
      'and an unread variant is valued %s',
      (value) => {
        beforeEach(async () => {
          result = await named('Button', { Kind: 'Primary', Destructive: value })
        })

        it('should still refuse it as identity-uncertain', () => {
          expect(codes(result)).toEqual(['identity-uncertain'])
        })

        it('should report it as incompatible', () => {
          expect(!result.ok && result.errors[0]!.details).toMatchObject({
            reason: 'name-incompatible',
            unknown: ['Destructive']
          })
        })
      }
    )

    describe('and only a boolean is given', () => {
      beforeEach(async () => {
        result = await withBooleans({}, { 'Show icon': 'true' })
      })

      it('should refuse it, since a boolean does not confirm the name', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-unconfirmed'
        })
      })
    })

    describe('and a boolean the mapping reads as a variant is given', () => {
      beforeEach(async () => {
        result = await withBooleans({ Kind: 'Primary' }, { Blocked: 'true' })
      })

      it('should refuse it as incompatible', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-incompatible',
          unknown: ['Blocked']
        })
      })
    })

    it.each([['True'], ['yes'], ['']])(
      'should refuse the boolean value %j as invalid input',
      async (value) => {
        expect(codes(await withBooleans({ Kind: 'Primary' }, { 'Show icon': value }))).toEqual([
          'invalid-input'
        ])
      }
    )

    describe('and a value the mapping does not support is given', () => {
      beforeEach(async () => {
        result = await named('Button', { Kind: 'Tertiary' })
      })

      it('should refuse it as incompatible, not as a missing variant', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-incompatible',
          unsupported: [
            { property: 'Kind', value: 'Tertiary', supported: ['Primary', 'Secondary'] }
          ]
        })
      })
    })

    describe('and the name is unknown', () => {
      beforeEach(async () => {
        result = await named('Card', { Kind: 'Primary' })
      })

      it('should report uncertain identity, never a missing mapping', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })

      it('should report the name as unmatched', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({ reason: 'name-unmatched' })
      })
    })

    describe('and two mappings declare the name', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: {
            'Button.figma.ts': mappingSource(),
            'Other.figma.ts': mappingSource({ nodeId: '2-2', id: 'Button', component: 'Other' })
          }
        })
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          name: 'Button',
          variantProperties: { Kind: 'Primary' }
        })
      })

      it('should refuse it as identity-ambiguous', () => {
        expect(codes(result)).toEqual(['identity-ambiguous'])
      })

      it('should list both mapping files', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-mappings',
          files: ['src/Button.figma.ts', 'src/Other.figma.ts']
        })
      })

      it('should report it as a library defect', () => {
        expect(!result.ok && result.errors[0]!.action).toMatch(/library defect/)
      })
    })

    describe('and one mapping declares the name while an alias gives it to another', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: {
            'Button.figma.ts': mappingSource(),
            'Other.figma.ts': mappingSource({ nodeId: '2-2', id: 'other', component: 'Other' })
          }
        })
        result = await resolveComponent(
          context(mappingRoot, { figmaSets: [], componentNames: { Button: 'Other' } }),
          { name: 'Button', variantProperties: { Kind: 'Primary' } }
        )
      })

      it('should refuse the name', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({ reason: 'name-mappings' })
      })
    })

    describe.each([
      ['a declared state only', { Hover: 'No' }],
      ['a property the mapping does not read only', { Size: 'L' }]
    ])('and the instance gives %s', (_label, variantProperties) => {
      beforeEach(async () => {
        result = await named('Button', variantProperties)
      })

      it('should not confirm the name', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'name-unconfirmed'
        })
      })
    })

    describe('and the mapping reads no variant property at all', () => {
      beforeEach(async () => {
        const source = mappingSource({ kindValues: {} })
          .replace(/const kind = [\s\S]*?\}\)\n/, '')
          .replace(/const disabled = [\s\S]*?\}\)\n/, '')
          .replace(' kind="${kind}"${disabled}', '')
        const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          name: 'Button'
        })
      })

      it('should resolve it by name alone', () => {
        expect(result.ok && result.resolved.usage).toBe('<Button>Label</Button>')
      })

      it('should warn that it was identified by name', () => {
        expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
      })
    })
  })

  describe('when two captured sets share a name', () => {
    let other: FigmaSet
    let mappingRoot: string

    beforeEach(async () => {
      other = {
        ...buttonSet,
        setId: '8:8',
        page: 'Chat',
        properties: { Size: { type: 'VARIANT', default: 'S', options: ['S', 'L'] } },
        variants: [{ id: '8:9', name: 'Size=S', props: { Size: 'S' } }]
      }
      ;({ mappingRoot } = await library())
    })

    describe('and the variant properties fit one set', () => {
      beforeEach(async () => {
        result = await resolveComponent(context(mappingRoot, { figmaSets: [buttonSet, other] }), {
          name: 'Button',
          variantProperties: { Kind: 'Secondary' }
        })
      })

      it('should pick the one set the variant properties fit', () => {
        expect(result.ok && result.resolved.provenance.figma.setId).toBe('1:1')
      })

      it('should say it was identified by name', () => {
        expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
      })
    })

    describe.each([
      ['no variant properties', undefined],
      ['properties neither set defines', { Theme: 'dark' }],
      ['a value neither set allows', { Kind: 'Tertiary' }]
    ])('and the request has %s', (_label, variantProperties) => {
      beforeEach(async () => {
        result = await resolveComponent(context(mappingRoot, { figmaSets: [buttonSet, other] }), {
          name: 'Button',
          variantProperties
        })
      })

      it('should stay ambiguous', () => {
        expect(codes(result)).toEqual(['identity-ambiguous'])
      })
    })
  })

  describe('when a set is captured on two pages', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(
        context(mappingRoot, { figmaSets: [buttonSet, { ...buttonSet, page: 'Other page' }] }),
        { name: 'Button', variantProperties: { Kind: 'Secondary' } }
      )
    })

    it('should resolve its name as one component', () => {
      expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
    })

    it('should say it was identified by name', () => {
      expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
    })
  })

  describe('when a request names an Object member', () => {
    let mappingRoot: string

    beforeEach(async () => {
      ;({ mappingRoot } = await library())
    })

    it.each(['toString', '__proto__', 'constructor'])(
      'should refuse the variant value %s instead of rendering an inherited member',
      async (value) => {
        expect(
          codes(
            await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
              nodeId: '1:1',
              variantProperties: JSON.parse(`{"Kind":${JSON.stringify(value)}}`)
            })
          )
        ).toEqual(['variant-unsupported'])
      }
    )

    describe('and a Figma property is named __proto__', () => {
      beforeEach(async () => {
        result = await resolveComponent(context(mappingRoot), {
          nodeId: '1:1',
          variantProperties: JSON.parse('{"__proto__":"x"}')
        })
      })

      it('should refuse it against the capture', () => {
        expect(codes(result)).toEqual(['variant-not-in-figma'])
      })
    })

    describe('and a text layer is named after an inherited member', () => {
      beforeEach(async () => {
        const { mappingRoot: textRoot } = await library({
          mappings: {
            'Button.figma.ts': mappingSource({
              body: "const labelLayer = instance.findText('toString')\nconst label = labelLayer.type === 'TEXT' ? labelLayer.textContent : ''"
            }).replace('>Label</', '>${label}</')
          }
        })
        result = await resolveComponent(context(textRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          texts: {}
        })
      })

      it('should not take the inherited member as text', () => {
        expect(result.ok && result.resolved.usage).toBe('<Button kind="">toString</Button>')
      })
    })
  })

  describe('when the review follow-up checks apply', () => {
    const booleanMapping = () =>
      library({
        mappings: {
          'Button.figma.ts': mappingSource({
            body: "const blocked = instance.getBoolean('Blocked', { true: ' disabled', false: '' })"
          })
            .replace(/const disabled = [^\n]*\n/, '')
            .replace('${disabled}', '${blocked}')
        }
      })

    describe('and a stale mapping targets a node outside a loaded capture', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
        })
        result = await resolveComponent(context(mappingRoot), { nodeId: '7:7' })
      })

      it('should refuse the node as identity-not-found', () => {
        expect(codes(result)).toEqual(['identity-not-found'])
      })

      it('should name the stale mapping', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.details).toEqual({
          reason: 'stale-mapping',
          nodeId: '7:7',
          files: ['src/Button.figma.ts']
        })
      })

      it('should report it as a library defect', () => {
        const [error] = !result.ok ? result.errors : []
        expect(error!.action).toMatch(/library defect/)
      })
    })

    describe('and a resolve fails with a pinned capture', () => {
      beforeEach(async () => {
        const { root, mappingRoot } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), lock('f'.repeat(40)))
        result = await resolveComponent(context(mappingRoot), {
          nodeId: '1:1',
          variantProperties: { Kind: 'Secondary', Blocked: 'Yes' }
        })
      })

      it('should give the failure the library and capture context a report needs', () => {
        expect(result).toMatchObject({
          ok: false,
          context: {
            library: { name: 'fixture-library', version: '1.2.0' },
            capture: { revision: 'f'.repeat(40) }
          }
        })
      })
    })

    describe('and the lock is unreadable', () => {
      beforeEach(async () => {
        const { root, mappingRoot } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), '{}')
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should name the unreadable lock in capture-revision-unknown', () => {
        const warning = result.warnings.find((w) => w.code === 'capture-revision-unknown')!
        expect(warning.details).toEqual({
          lock: 'fixture-library/upstream.lock.json'
        })
      })
    })

    describe('and the pin reader refuses the lock', () => {
      beforeEach(async () => {
        const { root, mappingRoot } = await library()
        await writeFile(
          path.join(root, 'upstream.lock.json'),
          JSON.stringify({
            sources: [
              {
                repository: 'https://github.com/decentraland/figma-design-system',
                snapshot: { revision: 'f'.repeat(40) }
              }
            ]
          })
        )
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1'
        })
      })

      it('should warn with capture-revision-unknown', () => {
        expect(result.warnings.map((w) => w.code)).toContain('capture-revision-unknown')
      })

      it('should not take a revision from the lock', () => {
        expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
      })
    })

    describe('and the mapping reads a boolean from booleanProperties', () => {
      beforeEach(async () => {
        const { mappingRoot } = await booleanMapping()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          variantProperties: { Kind: 'Primary' },
          booleanProperties: { Blocked: 'true' }
        })
      })

      it('should render the boolean', () => {
        expect(result.ok && result.resolved.usage).toBe(
          '<Button kind="primary" disabled>Label</Button>'
        )
      })

      it('should not warn that the property is unmapped', () => {
        expect(result.warnings.map((w) => w.code)).not.toContain('unmapped-figma-property')
      })
    })

    describe('and a node id comes with a boolean the mapping reads as a variant', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          variantProperties: { Kind: 'Primary' },
          booleanProperties: { Blocked: 'true' }
        })
      })

      it('should refuse the boolean', () => {
        expect(codes(result)).toEqual(['variant-unsupported'])
      })
    })

    describe('and a property the mapping does not read is given without a capture', () => {
      beforeEach(async () => {
        const { mappingRoot } = await booleanMapping()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          variantProperties: { Kind: 'Primary', Size: 'M' }
        })
      })

      it('should resolve', () => {
        expect(result.ok).toBe(true)
      })

      it('should warn, as the capture path does, about the property', () => {
        expect(
          result.warnings.find((w) => w.code === 'unmapped-figma-property')?.details
        ).toMatchObject({ property: 'Size' })
      })
    })

    describe.each([
      ['without a capture', []],
      [
        'with a capture',
        [{ ...buttonSet, properties: { ...buttonSet.properties, Blocked: { type: 'BOOLEAN' } } }]
      ]
    ])('and a boolean value is outside the accepted words %s', (_label, figmaSets) => {
      beforeEach(async () => {
        const { mappingRoot } = await booleanMapping()
        result = await resolveComponent(
          context(mappingRoot, { figmaSets: figmaSets as FigmaSet[] }),
          {
            nodeId: '1:1',
            variantProperties: { Blocked: 'Maybe' }
          }
        )
      })

      it('should refuse the value', () => {
        expect(codes(result)).toEqual(['variant-unsupported'])
      })
    })

    describe('and neither a capture nor variant properties are given', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1'
        })
      })

      it('should say no selection was applied', () => {
        expect(result.ok && result.resolved.usageFor.basis).toBe('unspecified')
      })
    })

    describe('and booleans are requested', () => {
      beforeEach(async () => {
        const { mappingRoot } = await booleanMapping()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          booleanProperties: { Blocked: 'true', 'Show icon': 'false' }
        })
      })

      it('should report the booleans as requested', () => {
        expect(result.ok && result.resolved.usageFor).toEqual({
          variantProperties: {},
          booleanProperties: { Blocked: 'true', 'Show icon': 'false' },
          basis: 'requested'
        })
      })

      it('should report the boolean the mapping reads as selected', () => {
        expect(
          result.ok &&
            result.resolved.propTranslation.find((entry) => entry.figmaType === 'BOOLEAN')
        ).toMatchObject({ figmaProperty: 'Blocked', selected: { figma: 'true', code: 'disabled' } })
      })
    })

    // The nearest false positive: a boolean local with no value given stays unselected.
    describe('and the instance does not give the boolean the mapping reads', () => {
      beforeEach(async () => {
        const { mappingRoot } = await booleanMapping()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          booleanProperties: { 'Show icon': 'true' }
        })
      })

      it('should leave the boolean unselected', () => {
        const entry =
          result.ok && result.resolved.propTranslation.find((item) => item.figmaType === 'BOOLEAN')
        expect(entry && 'selected' in entry).toBe(false)
      })

      it('should report the selection as requested', () => {
        expect(result.ok && result.resolved.usageFor.basis).toBe('requested')
      })
    })

    describe('and a declared state comes with a text the mapping does not render', () => {
      beforeEach(async () => {
        const { mappingRoot } = await library()
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          variantProperties: { Kind: 'Primary', Hover: 'Yes' },
          texts: { Caption: 'x' }
        })
      })

      it('should accept the declared state', () => {
        expect(result.ok).toBe(true)
      })

      it('should warn about the text', () => {
        expect(
          result.warnings.find((w) => w.code === 'unmapped-figma-property')?.details
        ).toMatchObject({
          text: 'Caption'
        })
      })
    })
  })

  describe.each([
    [
      'a directory that does not exist inside the package',
      (root: string) => path.join(root, 'src/missing')
    ],
    ['a file', (root: string) => path.join(root, 'src/Button.tsx')]
  ])('when the mapping root is %s', (_label, rootOf) => {
    let ctx: ResolveContext

    beforeEach(async () => {
      const { root } = await library()
      ctx = context(rootOf(root))
    })

    it('should report mapping-root-not-configured from resolve, never a raw error', async () => {
      expect(codes(await resolveComponent(ctx, { nodeId: '1:1' }))).toEqual([
        'mapping-root-not-configured'
      ])
    })

    it('should report mapping-root-not-configured from the check-mappings report', async () => {
      expect(await checkMappingsReport(ctx)).toMatchObject({
        ok: false,
        errors: [expect.objectContaining({ code: 'mapping-root-not-configured' })]
      })
    })

    it('should return no mapping results from check-mappings', async () => {
      expect(await checkMappings(ctx)).toEqual([])
    })
  })

  describe('when the component registry records accepted limits', () => {
    let limits: { id: string; text: string }[]
    let mappingRoot: string

    beforeEach(() => {
      limits = [
        { id: 'logo-size', text: 'The logo is pending.' },
        { id: 'mock-data', text: 'Views show mock data.' }
      ]
    })

    describe('and the registry lists limits for the component', () => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        await writeFile(
          path.join(mappingRoot, 'promotion.json'),
          JSON.stringify({ components: [{ name: 'Button', limits }] })
        )
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should return every limit', () => {
        expect(result.ok && result.resolved.limits).toEqual(limits)
      })

      it('should warn about the accepted limits', () => {
        expect(result.warnings.find((w) => w.code === 'accepted-limits')?.details).toEqual({
          ids: ['logo-size', 'mock-data']
        })
      })
    })

    describe.each([
      ['a registry entry without limits', JSON.stringify({ components: ['Button'] })],
      ['no registry', undefined]
    ])('and there is %s', (_label, registry) => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        if (registry) await writeFile(path.join(mappingRoot, 'promotion.json'), registry)
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should return no limits', () => {
        expect(result.ok && result.resolved.limits).toEqual([])
      })

      it('should not warn about accepted limits', () => {
        expect(result.warnings.map((w) => w.code)).not.toContain('accepted-limits')
      })
    })

    describe.each([
      [
        'invalid limits',
        () => JSON.stringify({ components: [{ name: 'Button', limits: [{ id: 'x', text: '' }] }] })
      ],
      ['malformed JSON', () => '{"components": ['],
      ['an entry object without a name', () => JSON.stringify({ components: [{ limits }] })]
    ])('and the registry has %s', (_label, text) => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        await writeFile(path.join(mappingRoot, 'promotion.json'), text())
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should fail closed', () => {
        expect(codes(result)).toEqual(['contract-incompatible'])
      })

      it('should give a coded diagnostic', () => {
        expect(!result.ok && result.errors[0]!.details).toMatchObject({
          reason: 'component-registry'
        })
      })
    })

    describe('and the registry is invalid but the node is not identified', () => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        await writeFile(path.join(mappingRoot, 'promotion.json'), '{')
        result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '8:8'
        })
      })

      it('should keep the identity code', () => {
        expect(codes(result)).toEqual(['identity-uncertain'])
      })
    })

    describe('and the mapping root points at a subdirectory', () => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library({ mappings: {} }))
        const sub = path.join(mappingRoot, 'Button')
        await mkdir(sub)
        await writeFile(path.join(sub, 'Button.figma.ts'), mappingSource())
        await writeFile(
          path.join(mappingRoot, 'promotion.json'),
          JSON.stringify({ components: [{ name: 'Button', limits }] })
        )
        result = await resolveComponent(context(sub, { figmaSets: [] }), { nodeId: '1:1' })
      })

      it('should find the registry above it', () => {
        expect(result.ok && result.resolved.limits).toEqual(limits)
      })
    })

    // The nearest false positive: a registry outside the library package is not the library's.
    describe('and a registry lies outside the library package', () => {
      beforeEach(async () => {
        // The package sits inside its own temporary directory, which holds the outside registry.
        const outer = await mkdtemp(path.join(tmpdir(), 'dcl-figma-outer-'))
        created.push(outer)
        const { root } = await library()
        const pkg = path.join(outer, 'pkg')
        await rename(root, pkg)
        await writeFile(path.join(outer, 'promotion.json'), '{')
        result = await resolveComponent(context(path.join(pkg, 'src')), { nodeId: '1:1' })
      })

      it('should not read the registry', () => {
        expect(result.ok && result.resolved.limits).toEqual([])
      })
    })

    // The nearest false positive: the same outside registry is read once it is inside the package.
    describe('and the same registry lies inside the package', () => {
      beforeEach(async () => {
        const { root, mappingRoot: inside } = await library()
        await writeFile(path.join(root, 'promotion.json'), '{')
        result = await resolveComponent(context(inside), { nodeId: '1:1' })
      })

      it('should read that registry', () => {
        expect(!result.ok && result.errors[0]!.message).toContain('promotion.json is not JSON')
      })
    })

    describe('and the registry is not JSON', () => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        await writeFile(path.join(mappingRoot, 'promotion.json'), '{"components": [ see https://x')
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should report that the registry is not JSON', () => {
        expect(!result.ok && result.errors[0]!.message).toMatch(/promotion\.json is not JSON\.$/)
      })

      it('should not quote the input', () => {
        expect(JSON.stringify(result)).not.toContain('https://x')
      })
    })

    describe('and the registry names a component twice', () => {
      beforeEach(async () => {
        ;({ mappingRoot } = await library())
        await writeFile(
          path.join(mappingRoot, 'promotion.json'),
          JSON.stringify({
            components: [
              { name: 'Button', limits: [{ id: 'a', text: 'First.' }] },
              { name: 'Button', limits: [{ id: 'b', text: 'Second.' }] }
            ]
          })
        )
        result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })
      })

      it('should refuse the registry instead of keeping the later limits', () => {
        expect(codes(result)).toEqual(['contract-incompatible'])
      })

      it('should name the duplicate component', () => {
        expect(!result.ok && result.errors[0]!.message).toContain(
          'Duplicate component registry name: Button'
        )
      })
    })
  })

  describe('when no mapping source is configured', () => {
    beforeEach(async () => {
      result = await resolveComponent({ figmaSets: [] }, { nodeId: '1:1' })
    })

    it('should report mapping-root-not-configured', () => {
      expect(codes(result)).toEqual(['mapping-root-not-configured'])
    })
  })

  describe('when neither a node id nor a name is given', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      result = await resolveComponent(context(mappingRoot), {})
    })

    it('should report invalid-input', () => {
      expect(codes(result)).toEqual(['invalid-input'])
    })
  })
})

describe('checkMappings', () => {
  let results: Check

  describe('when every mapping matches the library and the capture', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library()
      results = await checkMappings(context(mappingRoot))
    })

    it('should return no errors', () => {
      expect(results.flatMap((r) => r.diagnostics.filter((d) => d.severity === 'error'))).toEqual(
        []
      )
    })
  })

  describe('when a mapping points at a node missing from the capture', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })
      results = await checkMappings(context(mappingRoot))
    })

    it('should report identity-not-found', () => {
      expect(results[0]!.diagnostics.map((d) => d.code)).toContain('identity-not-found')
    })
  })

  describe('when two mappings collide', () => {
    describe.each([
      [
        'on the Figma node',
        () => ({ 'Copy.figma.ts': mappingSource({ id: 'copy' }) }),
        { reason: 'duplicate-node', nodeId: '1:1' },
        /one mapping per component set/
      ],
      [
        'on the template id',
        () => ({ 'Copy.figma.ts': mappingSource({ nodeId: '1-9' }) }),
        { reason: 'duplicate-id', id: 'button' },
        /its own id/
      ]
    ])('and they collide %s', (_l, extra, details, action) => {
      let found: Check[number]['diagnostics'][number]

      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: { 'Button.figma.ts': mappingSource(), ...extra() }
        })
        results = await checkMappings(context(mappingRoot, { figmaSets: [] }))
        found = results.flatMap((r) => r.diagnostics).find((d) => d.code === 'identity-ambiguous')!
      })

      it('should report both files', () => {
        expect(found.details).toMatchObject({
          ...details,
          files: ['src/Button.figma.ts', 'src/Copy.figma.ts']
        })
      })

      it('should give a maintainer action', () => {
        expect(found.action).toMatch(action)
      })
    })

    describe('and a mapping lies outside the capture', () => {
      let found: Check[number]['diagnostics'][number]

      beforeEach(async () => {
        const { mappingRoot } = await library({
          mappings: { 'Button.figma.ts': mappingSource({ nodeId: '9-9' }) }
        })
        const [result] = await checkMappings(context(mappingRoot))
        found = result!.diagnostics.find((d) => d.code === 'identity-not-found')!
      })

      it('should name the mapping file', () => {
        expect(found.details).toMatchObject({ reason: 'mapping-node', file: 'src/Button.figma.ts' })
      })

      it('should give the maintainer action', () => {
        expect(found.action).toMatch(/Point the mapping at the captured component set/)
      })
    })
  })

  describe('when a ready page under sync generates a component with the hand-written name', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })
      results = await checkMappings(
        context(mappingRoot, { loadSyncedComponents: async () => new Map([['Button', []]]) })
      )
    })

    it('should report the hand-written mapping as superseded instead of failing on its old node', () => {
      expect(results[0]!.diagnostics).toEqual([
        expect.objectContaining({ code: 'superseded-by-sync', severity: 'warning' })
      ])
    })
  })

  describe('when the synced replacement is excluded from promotion', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })
      results = await checkMappings(
        context(mappingRoot, {
          loadSyncedComponents: async () => new Map([['Button', ['unresolved-instance:Icon']]])
        })
      )
    })

    it('should say why the hand-written mapping stays', () => {
      expect(results[0]!.diagnostics).toEqual([
        expect.objectContaining({
          code: 'superseded-pending',
          severity: 'warning',
          details: { reasons: ['unresolved-instance:Icon'] }
        })
      ])
    })
  })

  describe('when sync generates other components only', () => {
    beforeEach(async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })
      results = await checkMappings(
        context(mappingRoot, { loadSyncedComponents: async () => new Map([['Modal', []]]) })
      )
    })

    it('should keep validating the hand-written mapping', () => {
      expect(results[0]!.diagnostics.map((d) => d.code)).toContain('identity-not-found')
    })
  })
})

describe('when an example places a text local inside a JS expression', () => {
  let result: Result

  beforeEach(async () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</',
      ">{'${label}'}</"
    )
    const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })
    result = await resolveComponent(
      context(mappingRoot, {
        figmaSets: [
          { ...buttonSet, properties: { ...buttonSet.properties, Label: { type: 'TEXT' } } }
        ]
      }),
      { nodeId: '1:1', texts: { Label: "Don't" } }
    )
  })

  it('should reject it', () => {
    expect(result.ok).toBe(false)
  })

  it('should say text placeholders are not supported in JavaScript expressions', () => {
    expect(JSON.stringify(result.ok ? [] : result.errors)).toContain(
      'text placeholders are supported only in JSX text or quoted attributes, not JavaScript expressions'
    )
  })
})

describe('when captured text contains JSX syntax', () => {
  let result: Result

  beforeEach(async () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</',
      '>${label}</'
    )
    const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })
    result = await resolveComponent(
      context(mappingRoot, {
        figmaSets: [
          { ...buttonSet, properties: { ...buttonSet.properties, Label: { type: 'TEXT' } } }
        ]
      }),
      {
        nodeId: '1:1',
        texts: { Label: 'Pay {total} < b "quoted"' }
      }
    )
  })

  it('should resolve without errors', () => {
    expect(result.ok ? [] : result.errors).toEqual([])
  })

  it('should emit text as a string expression instead of executable JSX', () => {
    expect(result.ok && result.resolved.usage).toContain(
      'Pay &#123;total&#125; &lt; b &quot;quoted&quot;'
    )
  })
})

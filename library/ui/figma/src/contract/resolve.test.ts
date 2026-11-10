import { rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { checkMappings, resolveComponent, type ResolveContext } from './resolve.js'
import { buttonSet, makeLibrary, mappingSource } from './test-fixtures.js'

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

const codes = (result: Awaited<ReturnType<typeof resolveComponent>>) =>
  result.ok ? [] : result.errors.map((error) => error.code)

afterEach(async () => {
  await Promise.all(created.splice(0).map((dir) => rm(dir, { recursive: true, force: true })))
})

describe('resolveComponent', () => {
  describe('when the component is mapped and the variant is supported', () => {
    it('should return the import, the translated props and the provenance', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1-1',
        variantProperties: { Kind: 'Secondary' }
      })

      expect(result.ok).toBe(true)
      if (!result.ok) return
      expect(result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
      expect(result.resolved.component.importStatements).toEqual([
        "import { Button } from 'fixture-library'"
      ])
      expect(result.resolved.provenance).toMatchObject({
        figma: { setId: '1:1', identity: 'verified-against-capture' },
        library: { name: 'fixture-library', version: '1.2.0' },
        mapping: { file: 'src/Button.figma.ts', id: 'button' }
      })
    })
  })

  describe('when the node id is the main component of an instance (a variant)', () => {
    it('should resolve the owning set and use the variant properties', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:3' })

      expect(result.ok && result.resolved.usage).toBe(
        '<Button kind="primary" disabled>Label</Button>'
      )
    })
  })

  describe('when no mapping exists for a captured Figma component', () => {
    it('should report mapping-missing and not generate anything', async () => {
      const { mappingRoot } = await library({ mappings: {} })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(codes(result)).toEqual(['mapping-missing'])
    })
  })

  describe('when a mapping only matches the component by name', () => {
    it('should still report mapping-missing and list the same-name mapping as a hint', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '9-9' }) }
      })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(codes(result)).toEqual(['mapping-missing'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        sameNameMappings: [{ targets: '9:9' }]
      })
    })
  })

  describe('when two Figma components share the requested name', () => {
    it('should report identity-ambiguous with the candidates', async () => {
      const { mappingRoot } = await library()
      const twin = { ...buttonSet, setId: '2:1', page: 'Other' }

      const result = await resolveComponent(
        context(mappingRoot, { figmaSets: [buttonSet, twin] }),
        { name: 'Button' }
      )

      expect(codes(result)).toEqual(['identity-ambiguous'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        candidates: [{ nodeId: '1:1' }, { nodeId: '2:1' }]
      })
    })
  })

  describe('when two mappings claim the same Figma node', () => {
    it('should report identity-ambiguous', async () => {
      const { mappingRoot } = await library({
        mappings: {
          'Button.figma.ts': mappingSource(),
          'Other.figma.ts': mappingSource({ id: 'other' })
        }
      })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(codes(result)).toEqual(['identity-ambiguous'])
    })
  })

  describe('when the Figma variant exists but the mapping does not cover it', () => {
    it('should report variant-unsupported with the supported values', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ kindValues: { Primary: 'primary' } }) }
      })

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Secondary' }
      })

      expect(codes(result)).toEqual(['variant-unsupported'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({ supported: ['Primary'] })
    })
  })

  describe('when the requested combination is not defined in the design', () => {
    it('should report variant-not-in-figma with the nearest variants', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Secondary', Blocked: 'Yes' }
      })

      expect(codes(result)).toEqual(['variant-not-in-figma'])
      expect(!result.ok && result.errors[0]!.details?.nearest).toBeDefined()
    })
  })

  describe('when a variant value does not exist in Figma at all', () => {
    it('should report variant-not-in-figma', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Ghost' }
      })

      expect(codes(result)).toEqual(['variant-not-in-figma'])
    })
  })

  describe('when the installed library does not satisfy the required range', () => {
    it('should report contract-incompatible with the library-version reason', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { libraryRange: '^2.0.0' }), {
        nodeId: '1:1'
      })

      expect(codes(result)).toEqual(['contract-incompatible'])
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
    it(`should report contract-incompatible (${reason})`, async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource(options) }
      })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(!result.ok && result.errors.map((e) => e.details?.reason)).toContain(reason)
    })
  })

  describe('when the component is not exported by the package entry', () => {
    it('should report contract-incompatible (export-missing)', async () => {
      const { mappingRoot } = await library({ entry: "export { Other } from './Other'\n" })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(!result.ok && result.errors[0]!.details).toMatchObject({ reason: 'export-missing' })
    })
  })

  describe('when Figma gained an option the mapping does not list', () => {
    it('should resolve but warn about the drift', async () => {
      const { mappingRoot } = await library()
      const grown = {
        ...buttonSet,
        properties: {
          ...buttonSet.properties,
          Kind: { type: 'VARIANT', default: 'Primary', options: ['Primary', 'Secondary', 'Ghost'] }
        }
      }

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [grown] }), {
        nodeId: '1:1'
      })

      expect(result.ok).toBe(true)
      expect(result.warnings.map((w) => w.code)).toContain('figma-drift')
    })
  })

  describe('when no capture is available', () => {
    it('should resolve from the mapping alone and say the identity is unverified', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1-1',
        variantProperties: { Kind: 'Secondary' }
      })

      expect(result.ok && result.resolved.provenance.figma.identity).toBe('mapping-only')
      expect(result.warnings.map((w) => w.code)).toContain('figma-identity-unverified')
    })
  })

  describe('when identity cannot be established', () => {
    it('should report identity-uncertain, not mapping-missing, for an unmapped node without a capture', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '7:7'
      })

      expect(codes(result)).toEqual(['identity-uncertain'])
      expect(!result.ok && result.errors[0]!.action).toMatch(
        /do not treat it as a missing mapping/i
      )
    })

    it('should ask for variant properties when a name alone matches a mapping', async () => {
      const { root } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), lock('e'.repeat(40)))
      const result = await resolveComponent(context(path.join(root, 'src'), { figmaSets: [] }), {
        name: 'Button'
      })

      expect(codes(result)).toEqual(['identity-uncertain'])
      const [error] = !result.ok ? result.errors : []
      expect(error!.details).toEqual({
        reason: 'name-unconfirmed',
        reads: ['Kind', 'Blocked'],
        name: 'Button',
        component: 'Button',
        captureRevision: 'e'.repeat(40)
      })
      expect(error!.action).toMatch(/Resolve again with the instance variantProperties/)
      expect(error!.action).not.toMatch(/Report/)
    })

    it('should send the caller to the set name for an unmatched node id without a capture', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:2'
      })

      expect(!result.ok && result.errors[0]!.action).toMatch(
        /resolve again with the component set name and variantProperties/
      )
    })

    it('should report several mappings claiming one node as a library defect to report', async () => {
      const { mappingRoot } = await library({
        mappings: {
          'Button.figma.ts': mappingSource(),
          'Copy.figma.ts': mappingSource({ id: 'copy' })
        }
      })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      expect(codes(result)).toEqual(['identity-ambiguous'])
      const [error] = !result.ok ? result.errors : []
      expect(error!.details).toMatchObject({
        reason: 'mappings',
        nodeId: '1:1',
        files: ['src/Button.figma.ts', 'src/Copy.figma.ts']
      })
      expect(error!.action).toMatch(/library defect/)
    })

    it('should report identity-not-found for a node outside the capture with no mapping', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), { nodeId: '7:7' })

      expect(codes(result)).toEqual(['identity-not-found'])
    })

    it('should report identity-not-found for an unknown name in the capture', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), { name: 'Nothing' })

      expect(codes(result)).toEqual(['identity-not-found'])
    })
  })

  describe('when a captured component has no mapping', () => {
    it('should give the contributor report context and no Figma URL or key', async () => {
      const { root, mappingRoot } = await library({
        mappings: { 'Other.figma.ts': mappingSource({ nodeId: '5-5', component: 'Other' }) }
      })
      await writeFile(path.join(root, 'upstream.lock.json'), lock('c'.repeat(40)))

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Primary' }
      })

      expect(codes(result)).toEqual(['mapping-missing'])
      const [error] = !result.ok ? result.errors : []
      expect(error!.details).toMatchObject({
        nodeId: '1:1',
        name: 'Button',
        variantProperties: { Kind: 'Primary' },
        library: { name: 'fixture-library', version: '1.2.0' },
        captureRevision: 'c'.repeat(40),
        supported: ['Other']
      })
      expect(error!.action).toMatch(/Do not build a replacement/)
      expect(JSON.stringify(result)).not.toMatch(/figma\.com|AAAAAAAAAAAAAAAAAAAAAA/)
    })
  })

  describe('when the library tracks a capture lock', () => {
    it('should name the pinned capture revision in the provenance without a capture', async () => {
      const { root, mappingRoot } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), lock('d'.repeat(40)))

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1-1'
      })

      expect(result.ok && result.resolved.provenance.capture).toEqual({ revision: 'd'.repeat(40) })
    })

    it('should omit the capture when the library has no lock', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1-1' })

      expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
    })

    it.each([
      ['a short revision', lock('abc')],
      ['no figma-design-system source', JSON.stringify({ sources: [] })],
      ['text that is not JSON', '{not json']
    ])(
      'should resolve with a capture-revision-unknown warning on a lock with %s',
      async (_label, text) => {
        const { root, mappingRoot } = await library()
        await writeFile(path.join(root, 'upstream.lock.json'), text)

        const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1-1'
        })

        expect(result.ok).toBe(true)
        expect(result.warnings.map((w) => w.code)).toContain('capture-revision-unknown')
        expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
      }
    )
  })

  describe('when a component is identified by its set name without a capture', () => {
    const named = (name: string, variantProperties?: Record<string, string>, extra = {}) =>
      library().then(({ mappingRoot }) =>
        resolveComponent(context(mappingRoot, { figmaSets: [], ...extra }), {
          name,
          variantProperties
        })
      )

    it.each([
      ['the exported component name', 'Button', {}],
      ['the template id', 'button', {}],
      [
        'a componentNames alias',
        'Button-Contained-Text',
        { componentNames: { 'Button-Contained-Text': 'Button' } }
      ]
    ])('should resolve %s once the variant properties fit', async (_label, name, extra) => {
      const result = await named(name, { Kind: 'Secondary', Hover: 'No' }, extra)

      expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
      expect(result.ok && result.resolved.provenance.figma.identity).toBe('mapping-only')
      expect(result.warnings.map((w) => w.code)).toEqual(
        expect.arrayContaining(['figma-identity-unverified', 'identity-name-match'])
      )
    })

    it('should refuse a property the mapping does not read as incompatible', async () => {
      const result = await named('Button', { Kind: 'Primary', Size: 'Large' })

      expect(codes(result)).toEqual(['identity-uncertain'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        reason: 'name-incompatible',
        unknown: ['Size'],
        unsupported: []
      })
    })

    it('should refuse a value the mapping does not support as incompatible, not as a missing variant', async () => {
      const result = await named('Button', { Kind: 'Tertiary' })

      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        reason: 'name-incompatible',
        unsupported: [{ property: 'Kind', value: 'Tertiary', supported: ['Primary', 'Secondary'] }]
      })
    })

    it('should report an unknown name as uncertain identity, never as a missing mapping', async () => {
      const result = await named('Card', { Kind: 'Primary' })

      expect(codes(result)).toEqual(['identity-uncertain'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({ reason: 'name-unmatched' })
    })

    it('should refuse a name two mappings declare', async () => {
      const { mappingRoot } = await library({
        mappings: {
          'Button.figma.ts': mappingSource(),
          'Other.figma.ts': mappingSource({ nodeId: '2-2', id: 'Button', component: 'Other' })
        }
      })

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        name: 'Button',
        variantProperties: { Kind: 'Primary' }
      })

      expect(codes(result)).toEqual(['identity-ambiguous'])
      expect(!result.ok && result.errors[0]!.details).toMatchObject({
        reason: 'name',
        candidates: [
          { nodeId: '1:1', file: 'src/Button.figma.ts' },
          { nodeId: '2:2', file: 'src/Other.figma.ts' }
        ]
      })
    })

    it('should resolve a mapping that reads no variant property by name alone', async () => {
      const source = mappingSource({ kindValues: {} })
        .replace(/const kind = [\s\S]*?\}\)\n/, '')
        .replace(/const disabled = [\s\S]*?\}\)\n/, '')
        .replace(' kind="${kind}"${disabled}', '')
      const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        name: 'Button'
      })

      expect(result.ok && result.resolved.usage).toBe('<Button>Label</Button>')
      expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
    })
  })

  describe('when two captured sets share a name', () => {
    const other = {
      ...buttonSet,
      setId: '8:8',
      page: 'Chat',
      properties: { Size: { type: 'VARIANT', default: 'S', options: ['S', 'L'] } },
      variants: [{ id: '8:9', name: 'Size=S', props: { Size: 'S' } }]
    }

    it('should pick the one set the variant properties fit', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(
        context(mappingRoot, { figmaSets: [buttonSet, other] }),
        {
          name: 'Button',
          variantProperties: { Kind: 'Secondary' }
        }
      )

      expect(result.ok && result.resolved.provenance.figma.setId).toBe('1:1')
      expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
    })

    it.each([
      ['no variant properties', undefined],
      ['properties neither set defines', { Theme: 'dark' }],
      ['a value neither set allows', { Kind: 'Tertiary' }]
    ])('should stay ambiguous with %s', async (_label, variantProperties) => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(
        context(mappingRoot, { figmaSets: [buttonSet, other] }),
        {
          name: 'Button',
          variantProperties
        }
      )

      expect(codes(result)).toEqual(['identity-ambiguous'])
    })
  })

  describe('when a set is captured on two pages', () => {
    it('should resolve its name as one component and say it was identified by name', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(
        context(mappingRoot, { figmaSets: [buttonSet, { ...buttonSet, page: 'Other page' }] }),
        { name: 'Button', variantProperties: { Kind: 'Secondary' } }
      )

      expect(result.ok && result.resolved.usage).toBe('<Button kind="secondary">Label</Button>')
      expect(result.warnings.map((w) => w.code)).toContain('identity-name-match')
    })
  })

  describe('when a request names an Object member', () => {
    it.each(['toString', '__proto__', 'constructor'])(
      'should refuse the variant value %s instead of rendering an inherited member',
      async (value) => {
        const { mappingRoot } = await library()

        const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
          nodeId: '1:1',
          variantProperties: JSON.parse(`{"Kind":${JSON.stringify(value)}}`)
        })

        expect(codes(result)).toEqual(['variant-unsupported'])
      }
    )

    it('should refuse a Figma property named __proto__ against the capture', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: JSON.parse('{"__proto__":"x"}')
      })

      expect(codes(result)).toEqual(['variant-not-in-figma'])
    })

    it('should not take an inherited member as text for a text layer', async () => {
      const { mappingRoot } = await library({
        mappings: {
          'Button.figma.ts': mappingSource({
            body: "const labelLayer = instance.findText('toString')\nconst label = labelLayer.type === 'TEXT' ? labelLayer.textContent : ''"
          }).replace('>Label</', '>${label}</')
        }
      })

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:1',
        texts: {}
      })

      expect(result.ok && result.resolved.usage).toBe('<Button kind="">toString</Button>')
    })
  })

  describe('when the review follow-up checks apply', () => {
    it('should refuse a node outside a loaded capture even when a stale mapping targets it', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })

      const result = await resolveComponent(context(mappingRoot), { nodeId: '7:7' })

      expect(codes(result)).toEqual(['identity-not-found'])
      const [error] = !result.ok ? result.errors : []
      expect(error!.details).toEqual({
        reason: 'stale-mapping',
        nodeId: '7:7',
        files: ['src/Button.figma.ts']
      })
      expect(error!.action).toMatch(/library defect/)
    })

    it('should give every failure the library and capture context a report needs', async () => {
      const { root, mappingRoot } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), lock('f'.repeat(40)))

      const result = await resolveComponent(context(mappingRoot), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Secondary', Blocked: 'Yes' }
      })

      expect(result).toMatchObject({
        ok: false,
        context: {
          library: { name: 'fixture-library', version: '1.2.0' },
          capture: { revision: 'f'.repeat(40) }
        }
      })
    })

    it('should name the unreadable lock in capture-revision-unknown', async () => {
      const { root, mappingRoot } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), '{}')

      const result = await resolveComponent(context(mappingRoot), { nodeId: '1:1' })

      const warning = result.warnings.find((w) => w.code === 'capture-revision-unknown')!
      expect(warning.details).toEqual({
        lock: 'fixture-library/upstream.lock.json'
      })
    })

    it('should not take a revision from a lock the pin reader refuses', async () => {
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

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:1'
      })

      expect(result.warnings.map((w) => w.code)).toContain('capture-revision-unknown')
      expect(result.ok && 'capture' in result.resolved.provenance).toBe(false)
    })

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

    it('should warn, as the capture path does, about a property the mapping does not read', async () => {
      const { mappingRoot } = await booleanMapping()

      const withoutCapture = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Primary', Size: 'M' }
      })

      expect(withoutCapture.ok).toBe(true)
      expect(
        withoutCapture.warnings.find((w) => w.code === 'unmapped-figma-property')?.details
      ).toMatchObject({ property: 'Size' })
    })

    it.each([
      ['without a capture', []],
      [
        'with a capture',
        [{ ...buttonSet, properties: { ...buttonSet.properties, Blocked: { type: 'BOOLEAN' } } }]
      ]
    ])('should refuse a boolean value outside the accepted words %s', async (_label, figmaSets) => {
      const { mappingRoot } = await booleanMapping()

      const result = await resolveComponent(context(mappingRoot, { figmaSets }), {
        nodeId: '1:1',
        variantProperties: { Blocked: 'Maybe' }
      })

      expect(codes(result)).toEqual(['variant-unsupported'])
    })

    it('should say no selection was applied without a capture or variant properties', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:1'
      })

      expect(result.ok && result.resolved.usageFor.basis).toBe('unspecified')
    })

    it('should accept a declared state and warn about a text the mapping does not render', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        nodeId: '1:1',
        variantProperties: { Kind: 'Primary', Hover: 'Yes' },
        texts: { Caption: 'x' }
      })

      expect(result.ok).toBe(true)
      expect(
        result.warnings.find((w) => w.code === 'unmapped-figma-property')?.details
      ).toMatchObject({
        text: 'Caption'
      })
    })
  })

  describe('when no mapping source is configured', () => {
    it('should report mapping-root-not-configured', async () => {
      const result = await resolveComponent({ figmaSets: [] }, { nodeId: '1:1' })

      expect(codes(result)).toEqual(['mapping-root-not-configured'])
    })
  })

  describe('when neither a node id nor a name is given', () => {
    it('should report invalid-input', async () => {
      const { mappingRoot } = await library()

      expect(codes(await resolveComponent(context(mappingRoot), {}))).toEqual(['invalid-input'])
    })
  })
})

describe('checkMappings', () => {
  describe('when every mapping matches the library and the capture', () => {
    it('should return no errors', async () => {
      const { mappingRoot } = await library()

      const results = await checkMappings(context(mappingRoot))

      expect(results.flatMap((r) => r.diagnostics.filter((d) => d.severity === 'error'))).toEqual(
        []
      )
    })
  })

  describe('when a mapping points at a node missing from the capture', () => {
    it('should report identity-not-found', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })

      const results = await checkMappings(context(mappingRoot))

      expect(results[0]!.diagnostics.map((d) => d.code)).toContain('identity-not-found')
    })
  })
  describe('when two mappings collide', () => {
    it.each([
      [
        'on the Figma node',
        { 'Copy.figma.ts': mappingSource({ id: 'copy' }) },
        { reason: 'duplicate-node', nodeId: '1:1' },
        /one mapping per component set/
      ],
      [
        'on the template id',
        { 'Copy.figma.ts': mappingSource({ nodeId: '1-9' }) },
        { reason: 'duplicate-id', id: 'button' },
        /its own id/
      ]
    ])(
      'should report %s with both files and a maintainer action',
      async (_l, extra, details, action) => {
        const { mappingRoot } = await library({
          mappings: { 'Button.figma.ts': mappingSource(), ...extra }
        })

        const results = await checkMappings(context(mappingRoot, { figmaSets: [] }))
        const found = results
          .flatMap((r) => r.diagnostics)
          .find((d) => d.code === 'identity-ambiguous')!

        expect(found.details).toMatchObject({
          ...details,
          files: ['src/Button.figma.ts', 'src/Copy.figma.ts']
        })
        expect(found.action).toMatch(action)
      }
    )

    it('should give a mapping outside the capture the maintainer action', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '9-9' }) }
      })

      const [result] = await checkMappings(context(mappingRoot))
      const found = result!.diagnostics.find((d) => d.code === 'identity-not-found')!

      expect(found.details).toMatchObject({ reason: 'mapping-node', file: 'src/Button.figma.ts' })
      expect(found.action).toMatch(/Point the mapping at the captured component set/)
    })
  })

  describe('when a ready page under sync generates a component with the hand-written name', () => {
    it('should report the hand-written mapping as superseded instead of failing on its old node', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })

      const results = await checkMappings(
        context(mappingRoot, { loadSyncedComponents: async () => new Map([['Button', []]]) })
      )

      expect(results[0]!.diagnostics).toEqual([
        expect.objectContaining({ code: 'superseded-by-sync', severity: 'warning' })
      ])
    })
  })

  describe('when the synced replacement is excluded from promotion', () => {
    it('should say why the hand-written mapping stays', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })

      const results = await checkMappings(
        context(mappingRoot, {
          loadSyncedComponents: async () => new Map([['Button', ['unresolved-instance:Icon']]])
        })
      )

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
    it('should keep validating the hand-written mapping', async () => {
      const { mappingRoot } = await library({
        mappings: { 'Button.figma.ts': mappingSource({ nodeId: '7-7' }) }
      })

      const results = await checkMappings(
        context(mappingRoot, { loadSyncedComponents: async () => new Map([['Modal', []]]) })
      )

      expect(results[0]!.diagnostics.map((d) => d.code)).toContain('identity-not-found')
    })
  })
})

describe('when an example places a text local inside a JS expression', () => {
  it('should reject it instead of rendering entity-escaped text as code', async () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</',
      ">{'${label}'}</"
    )
    const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })
    const result = await resolveComponent(
      context(mappingRoot, {
        figmaSets: [
          { ...buttonSet, properties: { ...buttonSet.properties, Label: { type: 'TEXT' } } }
        ]
      }),
      { nodeId: '1:1', texts: { Label: "Don't" } }
    )
    expect(result.ok).toBe(false)
    expect(JSON.stringify(result.ok ? [] : result.errors)).toContain(
      'text placeholders are supported only in JSX text or quoted attributes, not JavaScript expressions'
    )
  })
})

describe('when captured text contains JSX syntax', () => {
  it('should emit text as a string expression instead of executable JSX', async () => {
    const source = mappingSource({ body: "const label = instance.getString('Label')" }).replace(
      '>Label</',
      '>${label}</'
    )
    const { mappingRoot } = await library({ mappings: { 'Button.figma.ts': source } })
    const text = 'Pay {total} < b "quoted"'
    const result = await resolveComponent(
      context(mappingRoot, {
        figmaSets: [
          { ...buttonSet, properties: { ...buttonSet.properties, Label: { type: 'TEXT' } } }
        ]
      }),
      {
        nodeId: '1:1',
        texts: { Label: text }
      }
    )
    expect(result.ok ? [] : result.errors).toEqual([])
    expect(result.ok && result.resolved.usage).toContain(
      'Pay &#123;total&#125; &lt; b &quot;quoted&quot;'
    )
  })
})

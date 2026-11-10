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
        snapshot: { revision }
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
      expect(!result.ok && result.errors[0]!.action).toMatch(/Do not treat it as a missing mapping/)
    })

    it('should report identity-uncertain for a name lookup without a capture', async () => {
      const { mappingRoot } = await library()

      const result = await resolveComponent(context(mappingRoot, { figmaSets: [] }), {
        name: 'Button'
      })

      expect(codes(result)).toEqual(['identity-uncertain'])
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
      ['no figma-design-system source', JSON.stringify({ sources: [] })]
    ])('should fail closed on a lock with %s', async (_label, text) => {
      const { root, mappingRoot } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), text)

      await expect(resolveComponent(context(mappingRoot), { nodeId: '1-1' })).rejects.toThrow(
        'pins no full figma-design-system capture revision'
      )
    })

    it('should fail closed on a lock that is not JSON', async () => {
      const { root, mappingRoot } = await library()
      await writeFile(path.join(root, 'upstream.lock.json'), '{not json')

      await expect(resolveComponent(context(mappingRoot), { nodeId: '1-1' })).rejects.toThrow()
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

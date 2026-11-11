import { Buffer } from 'node:buffer'
import { createHash } from 'node:crypto'
import { beforeEach, describe, expect, it } from 'vitest'
import {
  componentConfigPath,
  componentDependencyPaths,
  componentGenerationFile,
  componentModulePaths,
  componentOutputKind,
  isComponentModulePath,
  createComponentGeneration,
  validateComponentGeneration
} from '../component-generation.mjs'
import { componentSourceIdentity, digestTableHash } from '../source-identity.mjs'

type Identity = { path: string; sha256: string; indexBlobSha: string }
type Source = {
  repository: string
  revision: string
  snapshot: { revision: string; digests: Record<string, string> }
  files: Record<string, { sha256: string }>
}

describe('when validating component generation as an offline consistency record', () => {
  const prefix = 'library/ui/figma/'
  const runtime = [
    'figma-module-runtime.tsx',
    'figma-module-runtime.module.css',
    'figma-view-state.ts',
    'figma-interactions.tsx'
  ]
  const hash = (bytes: string) => createHash('sha256').update(bytes).digest('hex')
  const oid = (bytes: string) => createHash('sha1').update(bytes).digest('hex')
  const encode = (value: unknown) => Buffer.from(JSON.stringify(value, null, 2) + '\n')
  let source: Source
  let modules: Identity[]
  let dependencies: Identity[]
  let config: Identity
  let files: Map<string, Buffer>
  let bytes: Buffer
  let parsed: ReturnType<typeof validateComponentGeneration>
  let generate: () => Buffer

  beforeEach(() => {
    source = {
      repository: 'https://github.com/decentraland/figma-design-system',
      revision: 'a'.repeat(40),
      snapshot: {
        revision: 'b'.repeat(40),
        digests: { 'z.json': 'f'.repeat(64), 'a.json': '1'.repeat(64) }
      },
      files: Object.fromEntries(
        [
          'component-generation.mjs',
          'source-identity.mjs',
          'ledger/retirements.json',
          'tsconfig.json',
          'src/install.ts',
          'src/install-components.ts',
          'src/generators/components.ts',
          'src/generators/tokens.ts',
          'src/extra-production.ts',
          ...runtime.map((file) => 'review/src/components/' + file),
          'figma.config.json'
        ].map((file) => [prefix + file, { sha256: hash(file) }])
      )
    }
    modules = componentModulePaths(source).map((path) => ({
      path,
      sha256: source.files[path]!.sha256,
      indexBlobSha: oid(path)
    }))
    dependencies = componentDependencyPaths.map((path) => ({
      path,
      sha256: hash(path),
      indexBlobSha: oid(path)
    }))
    config = {
      path: componentConfigPath,
      sha256: source.files[componentConfigPath]!.sha256,
      indexBlobSha: oid('config')
    }
    files = new Map([
      ...runtime.map((file) => ['runtime/' + file, Buffer.from('// ' + file)] as const),
      ['runtime/figma-module-runtime.module.css.d.ts', Buffer.from('declare const styles: {}')],
      ...[
        'Button/Button.tsx',
        'Button/Button.module.css',
        'Button/Button.module.css.d.ts',
        'Button/Button.figma.ts',
        'NavBar/NavBar.tsx',
        'NavBar/NavBar.module.css',
        'NavBar/NavBar.module.css.d.ts'
      ].map((file) => [file, Buffer.from('// ' + file)] as const),
      ['promotion.json', encode({ components: ['Button'], internal: ['NavBar'] })]
    ])
    generate = () => createComponentGeneration({ source, modules, dependencies, config, files })
    bytes = generate()
    parsed = validateComponentGeneration(bytes, source)
  })

  it('should preserve the historical marker digest byte protocol and key-order independence', () => {
    expect(digestTableHash({ 'z.json': 'f'.repeat(64), 'a.json': '1'.repeat(64) })).toBe(
      '95be3c2f8b13b9259551a323fca6376c3efcd63395c0355f68a3552233f4f828'
    )
    expect(digestTableHash({ 'a.json': '1'.repeat(64), 'z.json': 'f'.repeat(64) })).toBe(
      '95be3c2f8b13b9259551a323fca6376c3efcd63395c0355f68a3552233f4f828'
    )
  })

  it('should retain distinct source, output digest and index object identities', () => {
    expect(parsed.source).toEqual({
      repository: source.repository,
      codeRevision: source.revision,
      snapshotRevision: source.snapshot.revision,
      digestTableHash: '95be3c2f8b13b9259551a323fca6376c3efcd63395c0355f68a3552233f4f828'
    })
    expect(parsed.inputs.config).toEqual(config)
    expect(parsed.inputs.dependencies).toEqual(dependencies)
    expect(parsed.modules).toEqual(modules)
    expect(parsed.registry).toMatchObject({ components: ['Button'], internal: ['NavBar'] })
    expect(parsed.output).toHaveLength(files.size)
    expect(
      parsed.output.some((entry: { path: string }) => entry.path === componentGenerationFile)
    ).toBe(false)
  })

  it('should serialize deterministically regardless of map and input identity order', () => {
    expect(
      createComponentGeneration({
        source,
        modules: [...modules].reverse(),
        dependencies: [...dependencies].reverse(),
        config,
        files: new Map([...files].reverse())
      })
    ).toEqual(bytes)
  })

  it('should accept exact independently inspected identities', () => {
    expect(validateComponentGeneration(bytes, source, { modules, config, dependencies })).toEqual(
      parsed
    )
  })

  it('should require an explicit expected path for a pinned custom configuration', () => {
    config = { ...config, path: prefix + 'custom.config.json' }
    source.files[config.path] = { sha256: config.sha256 }
    bytes = generate()
    expect(() => validateComponentGeneration(bytes, source)).toThrow('config identity')
    expect(
      validateComponentGeneration(bytes, source, { configPath: config.path, config }).inputs.config
    ).toEqual(config)
  })

  it('should represent complete retirement with only registry output and generation metadata', () => {
    files = new Map([['promotion.json', encode({ components: [] })]])
    bytes = generate()
    parsed = validateComponentGeneration(bytes, source)
    expect(parsed.registry).toMatchObject({ components: [], internal: [] })
    expect(parsed.output).toEqual([
      { path: 'promotion.json', sha256: hash(files.get('promotion.json')!.toString()) }
    ])
    expect(parsed.modules).toEqual(modules)
    expect(parsed.inputs.dependencies).toEqual(dependencies)
    expect(componentOutputKind(componentGenerationFile, parsed.registry)).toBe('generated')
    expect(componentOutputKind('promotion.json', parsed.registry)).toBe('source')
  })

  it.each([
    'runtime/figma-module-runtime.tsx',
    'runtime/figma-module-runtime.module.css.d.ts',
    'Button/Button.tsx',
    'assets/0123456789abcdef.svg'
  ])('should reject stale output %s after complete retirement', (output) => {
    files = new Map([['promotion.json', encode({ components: [] })]])
    files.set(output, Buffer.from('stale'))
    expect(generate).toThrow('Unexpected component output for empty registry')
  })

  it('should reject stale output claims in an otherwise canonical empty record', () => {
    files = new Map([['promotion.json', encode({ components: [] })]])
    parsed = validateComponentGeneration(generate(), source)
    parsed.output.push({ path: 'runtime/figma-module-runtime.tsx', sha256: 'a'.repeat(64) })
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Unexpected component output for empty registry'
    )
  })

  it('should classify only public mappings and independently fixed runtime paths', () => {
    expect(componentOutputKind('Button/Button.figma.ts', parsed.registry)).toBe('generated')
    expect(componentOutputKind('runtime/figma-module-runtime.tsx', parsed.registry)).toBe('runtime')
    expect(componentOutputKind('promotion.json', parsed.registry)).toBe('source')
    expect(() => componentOutputKind('NavBar/NavBar.figma.ts', parsed.registry)).toThrow(
      'internal mapping'
    )
  })

  it('should include newly locked production modules without trusting the manifest closure', () => {
    source.files[prefix + 'src/new-production.ts'] = { sha256: hash('new code') }
    expect(() => validateComponentGeneration(bytes, source)).toThrow(
      'Incomplete component module identities'
    )
  })

  it('should select the effective retirement ledger independently of a claimed module table', () => {
    expect(isComponentModulePath(prefix + 'ledger/retirements.json')).toBe(true)
    expect(isComponentModulePath(prefix + 'src/specs.ts')).toBe(true)
    expect(isComponentModulePath(prefix + 'src/specs.test.ts')).toBe(false)
    expect(isComponentModulePath(prefix + 'ledger/unrelated.json')).toBe(false)
    expect(modules.some((module) => module.path === prefix + 'ledger/retirements.json')).toBe(true)
  })

  it('should bind both operational resolver configuration files', () => {
    expect(isComponentModulePath(prefix + 'tsconfig.json')).toBe(true)
    expect(modules.some((module) => module.path === prefix + 'tsconfig.json')).toBe(true)
    expect(componentDependencyPaths).toEqual([
      'library/ui/package.json',
      'library/ui/pnpm-lock.yaml',
      'library/ui/tsconfig.json'
    ])
    expect(
      parsed.inputs.dependencies.some(
        (dependency: Identity) => dependency.path === 'library/ui/tsconfig.json'
      )
    ).toBe(true)
  })

  it('should reject an omitted Figma resolver configuration pin', () => {
    delete source.files[prefix + 'tsconfig.json']
    expect(() => validateComponentGeneration(bytes, source)).toThrow(
      'Missing locked component implementation: ' + prefix + 'tsconfig.json'
    )
  })

  it('should reject an omitted retirement ledger pin', () => {
    delete source.files[prefix + 'ledger/retirements.json']
    expect(() => validateComponentGeneration(bytes, source)).toThrow(
      'Missing locked component implementation: ' + prefix + 'ledger/retirements.json'
    )
  })

  it('should exclude test and fixture modules from the independently selected closure', () => {
    for (const file of [
      'src/example.test.ts',
      'src/example.spec.ts',
      'src/__tests__/example.ts',
      'src/fixtures/example.ts'
    ])
      source.files[prefix + file] = { sha256: 'not-a-production-pin' }
    expect(componentModulePaths(source)).toEqual(modules.map((module) => module.path))
  })

  it('should reject an absent mandatory implementation even if the manifest omits it too', () => {
    delete source.files[prefix + 'source-identity.mjs']
    parsed.modules = parsed.modules.filter(
      (module: Identity) => module.path !== prefix + 'source-identity.mjs'
    )
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Missing locked component implementation'
    )
  })

  it.each(['js', 'jsx', 'ts', 'tsx', 'mjs', 'mts', 'cjs', 'cts'])(
    'should independently include tracked top-level .%s implementations',
    (extension) => {
      const file = prefix + 'extra-helper.' + extension
      source.files[file] = { sha256: hash(file) }
      expect(isComponentModulePath(file)).toBe(true)
      expect(componentModulePaths(source)).toContain(file)
      expect(() => validateComponentGeneration(bytes, source)).toThrow(
        'Incomplete component module identities'
      )
    }
  )

  it('should exclude top-level tests and unrelated nested helper folders', () => {
    expect(isComponentModulePath(prefix + 'helper.test.mts')).toBe(false)
    expect(isComponentModulePath(prefix + 'other/helper.mts')).toBe(false)
  })

  it('should reject colliding locked module paths', () => {
    source.files[prefix + 'src/EXTRA-PRODUCTION.ts'] = { sha256: hash('case') }
    expect(() => componentModulePaths(source)).toThrow('Colliding locked component module paths')
  })

  it('should reject unsupported schemas and unknown top-level claims', () => {
    parsed.schema = 2
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Unsupported component generation schema'
    )
    parsed.schema = 1
    Object.assign(parsed, { liveAttestation: true })
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow('generation record')
  })

  it.each(['source', 'registry', 'inputs', 'transform', 'modules', 'output'])(
    'should reject a missing %s section',
    (section) => {
      Reflect.deleteProperty(parsed, section)
      expect(() => validateComponentGeneration(encode(parsed), source)).toThrow('generation record')
    }
  )

  it.each(['repository', 'codeRevision', 'snapshotRevision', 'digestTableHash'])(
    'should reject changed source identity field %s',
    (field) => {
      Object.assign(parsed.source, { [field]: 'changed' })
      expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
        'source differs from lock'
      )
    }
  )

  it('should reject a changed pinned digest table even if source revisions are unchanged', () => {
    source.snapshot.digests['a.json'] = '2'.repeat(64)
    expect(() => validateComponentGeneration(bytes, source)).toThrow('source differs from lock')
  })

  it('should reject an invalid source identity before trusting claimed provenance', () => {
    source.repository = 'https://example.invalid/fork'
    expect(() => componentSourceIdentity(source)).toThrow(
      'Invalid verified component source identity'
    )
    expect(() => validateComponentGeneration(bytes, source)).toThrow(
      'Invalid verified component source identity'
    )
  })

  it.each(['missing', 'extra', 'duplicate', 'unsafe', 'digest', 'oid', 'fields'])(
    'should reject %s module identities',
    (kind) => {
      if (kind === 'missing') parsed.modules.pop()
      if (kind === 'extra')
        parsed.modules.push({ ...parsed.modules[0], path: prefix + 'src/not-locked.ts' })
      if (kind === 'duplicate') parsed.modules[1] = { ...parsed.modules[0] }
      if (kind === 'unsafe') parsed.modules[0].path = '../outside'
      if (kind === 'digest') parsed.modules[0].sha256 = '0'.repeat(64)
      if (kind === 'oid') parsed.modules[0].indexBlobSha = 'invalid'
      if (kind === 'fields') Object.assign(parsed.modules[0], { origin: 'trusted' })
      expect(() => validateComponentGeneration(encode(parsed), source)).toThrow()
    }
  )

  it('should reject a syntactically valid forged module object ID against inspected identities', () => {
    parsed.modules[0].indexBlobSha = 'f'.repeat(40)
    expect(() => validateComponentGeneration(encode(parsed), source, { modules })).toThrow(
      'module differs from verified inputs'
    )
  })

  it.each(['path', 'digest', 'oid', 'missing-pin'])('should reject invalid config %s', (kind) => {
    if (kind === 'path') parsed.inputs.config.path = prefix + 'unapproved.json'
    if (kind === 'digest') parsed.inputs.config.sha256 = '0'.repeat(64)
    if (kind === 'oid') parsed.inputs.config.indexBlobSha = 'invalid'
    if (kind === 'missing-pin') delete source.files[componentConfigPath]
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow()
  })

  it('should reject forged config object IDs when independently inspected', () => {
    parsed.inputs.config.indexBlobSha = 'f'.repeat(40)
    expect(() => validateComponentGeneration(encode(parsed), source, { config })).toThrow(
      'config differs from verified inputs'
    )
  })

  it.each(['missing', 'extra', 'duplicate', 'unsafe', 'digest', 'oid'])(
    'should reject %s dependency identities',
    (kind) => {
      if (kind === 'missing') parsed.inputs.dependencies.pop()
      if (kind === 'extra')
        parsed.inputs.dependencies.push({ ...dependencies[0], path: 'library/ui/extra-lock.json' })
      if (kind === 'duplicate') parsed.inputs.dependencies[1] = { ...dependencies[0] }
      if (kind === 'unsafe') parsed.inputs.dependencies[0].path = '../package.json'
      if (kind === 'digest') parsed.inputs.dependencies[0].sha256 = 'invalid'
      if (kind === 'oid') parsed.inputs.dependencies[0].indexBlobSha = 'invalid'
      expect(() => validateComponentGeneration(encode(parsed), source)).toThrow()
    }
  )

  it('should reject dependency hashes inconsistent with independently inspected package inputs', () => {
    parsed.inputs.dependencies[0].sha256 = 'f'.repeat(64)
    expect(() => validateComponentGeneration(encode(parsed), source, { dependencies })).toThrow(
      'dependency differs from verified inputs'
    )
  })

  it.each(['duplicate', 'both', 'case', 'invalid', 'extra-field'])(
    'should reject %s visibility names',
    (kind) => {
      if (kind === 'duplicate') parsed.registry.components.push('Button')
      if (kind === 'both') parsed.registry.internal.push('Button')
      if (kind === 'case') parsed.registry.internal.push('button')
      if (kind === 'invalid') parsed.registry.internal.push('../Unsafe')
      if (kind === 'extra-field') Object.assign(parsed.registry, { captured: true })
      expect(() => validateComponentGeneration(encode(parsed), source)).toThrow()
    }
  )

  // The record names the registry's components and pins its bytes; the accepted limits stay in promotion.json.
  it('should pin a registry with accepted limits by digest without copying the limits', () => {
    const limited = encode({
      components: [{ name: 'Button', limits: [{ id: 'logo-size', text: 'Pending.' }] }],
      internal: ['NavBar']
    })
    files.set('promotion.json', limited)

    const record = validateComponentGeneration(generate(), source)

    expect(record.registry).toEqual({
      path: 'promotion.json',
      sha256: hash(limited.toString()),
      components: ['Button'],
      internal: ['NavBar']
    })
  })

  it('should reject a record that still copies the limits into its registry', () => {
    ;(parsed.registry as Record<string, unknown>).limits = {}
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Invalid component generation registry'
    )
  })

  it('should reject registry names that are not plain names', () => {
    ;(parsed.registry as { components: unknown[] }).components = [{ name: 'Button', limits: [] }]
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Invalid component generation registry names'
    )
  })

  it('should reject registry/output digest disagreement', () => {
    parsed.registry.sha256 = '0'.repeat(64)
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'registry digest differs from output'
    )
  })

  it.each([
    'missing',
    'duplicate',
    'unsafe',
    'internal-mapping',
    'self',
    'unexpected',
    'digest',
    'fields'
  ])('should reject %s output identities', (kind) => {
    if (kind === 'missing') parsed.output.pop()
    if (kind === 'duplicate') parsed.output.push({ ...parsed.output[0] })
    if (kind === 'unsafe') parsed.output[0].path = '../outside'
    if (kind === 'internal-mapping')
      parsed.output.push({ path: 'NavBar/NavBar.figma.ts', sha256: 'a'.repeat(64) })
    if (kind === 'self')
      parsed.output.push({ path: componentGenerationFile, sha256: 'a'.repeat(64) })
    if (kind === 'unexpected') parsed.output.push({ path: 'unrelated.ts', sha256: 'a'.repeat(64) })
    if (kind === 'digest') parsed.output[0].sha256 = 'invalid'
    if (kind === 'fields') Object.assign(parsed.output[0], { kind: 'source' })
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow()
  })

  it('should reject missing public mappings while preserving the internal mapping hold', () => {
    parsed.output = parsed.output.filter(
      (output: { path: string }) => output.path !== 'Button/Button.figma.ts'
    )
    expect(() => validateComponentGeneration(encode(parsed), source)).toThrow(
      'Missing emitted component output: Button/Button.figma.ts'
    )
  })

  it.each([
    'compact',
    'no-newline',
    'modules-order',
    'dependencies-order',
    'output-order',
    'duplicate-json-key'
  ])('should reject noncanonical serialization: %s', (kind) => {
    if (kind === 'compact') bytes = Buffer.from(JSON.stringify(parsed))
    if (kind === 'no-newline') bytes = Buffer.from(bytes.toString().trimEnd())
    if (kind === 'modules-order') {
      parsed.modules.reverse()
      bytes = encode(parsed)
    }
    if (kind === 'dependencies-order') {
      parsed.inputs.dependencies.reverse()
      bytes = encode(parsed)
    }
    if (kind === 'output-order') {
      parsed.output.reverse()
      bytes = encode(parsed)
    }
    if (kind === 'duplicate-json-key')
      bytes = Buffer.from(bytes.toString().replace('"schema": 1,', '"schema": 1,\n  "schema": 1,'))
    expect(() => validateComponentGeneration(bytes, source)).toThrow(
      'Noncanonical component generation record'
    )
  })

  it('should refuse malformed JSON and non-buffer input', () => {
    expect(() => validateComponentGeneration(Buffer.from('{'), source)).toThrow('generation JSON')
    expect(() => validateComponentGeneration('not-bytes' as unknown as Buffer, source)).toThrow(
      'must be bytes'
    )
  })

  it('should leave actual output-byte verification to the common inspector', () => {
    parsed.output[0].sha256 = 'f'.repeat(64)
    expect(validateComponentGeneration(encode(parsed), source).output[0].sha256).toBe(
      'f'.repeat(64)
    )
  })

  it('should reject a self-listed generation record during creation too', () => {
    files.set(componentGenerationFile, Buffer.from('{}'))
    expect(generate).toThrow('self-listed component plan output')
  })
})

import { Buffer } from 'node:buffer'
import { createHash } from 'node:crypto'
import { beforeEach, describe, expect, it } from 'vitest'
import {
  componentConfigPath,
  componentDependencyPaths,
  componentGenerationFile,
  componentModuleCandidates,
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
  let contents: Map<string, string>
  // Pins one synthetic file: its bytes are its text, so the lock and the reader agree.
  const lock = (file: string, text = '// ' + file) => {
    contents.set(prefix + file, text)
    source.files[prefix + file] = { sha256: hash(text) }
  }
  const read = (file: string) => {
    const text = contents.get(file)
    if (text === undefined) throw new Error('Unreadable synthetic module: ' + file)
    return text
  }

  beforeEach(() => {
    contents = new Map()
    source = {
      repository: 'https://github.com/decentraland/figma-design-system',
      revision: 'a'.repeat(40),
      snapshot: {
        revision: 'b'.repeat(40),
        digests: { 'z.json': 'f'.repeat(64), 'a.json': '1'.repeat(64) }
      },
      files: {}
    }
    for (const file of [
      'component-generation.mjs',
      'module-graph.mjs',
      'source-identity.mjs',
      'ledger/retirements.json',
      'src/install.ts',
      'src/install-components.ts',
      'src/generators/components.ts',
      'src/generators/tokens.ts',
      'src/unrelated-tool.ts',
      ...runtime.map((name) => 'review/src/components/' + name),
      'figma.config.json'
    ])
      lock(file)
    lock('tsconfig.json', '{"compilerOptions":{}}')
    lock('src/install-main.ts', "import { runInstall } from './install.js'")
    lock('src/extra-production.ts', "import './cycle.js'")
    lock('src/cycle.ts', "export * from './extra-production.js'")
    lock('src/install.ts', "import { extra } from './extra-production.js'")
    modules = componentModulePaths(source, read).map((path) => ({
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
    generate = () =>
      createComponentGeneration({ source, read, modules, dependencies, config, files })
    bytes = generate()
    parsed = validateComponentGeneration(bytes, source, read)
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
        read,
        modules: [...modules].reverse(),
        dependencies: [...dependencies].reverse(),
        config,
        files: new Map([...files].reverse())
      })
    ).toEqual(bytes)
  })

  it('should accept exact independently inspected identities', () => {
    expect(
      validateComponentGeneration(bytes, source, read, { modules, config, dependencies })
    ).toEqual(parsed)
  })

  it('should require an explicit expected path for a pinned custom configuration', () => {
    config = { ...config, path: prefix + 'custom.config.json' }
    source.files[config.path] = { sha256: config.sha256 }
    bytes = generate()
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow('config identity')
    expect(
      validateComponentGeneration(bytes, source, read, { configPath: config.path, config }).inputs
        .config
    ).toEqual(config)
  })

  it('should represent complete retirement with only registry output and generation metadata', () => {
    files = new Map([['promotion.json', encode({ components: [] })]])
    bytes = generate()
    parsed = validateComponentGeneration(bytes, source, read)
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
    parsed = validateComponentGeneration(generate(), source, read)
    parsed.output.push({ path: 'runtime/figma-module-runtime.tsx', sha256: 'a'.repeat(64) })
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
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

  it('should bind the modules the installer reaches and leave unrelated tooling out', () => {
    const paths = modules.map((module) => module.path)
    expect(paths).toContain(prefix + 'src/extra-production.ts')
    expect(paths).toContain(prefix + 'src/cycle.ts')
    expect(paths).not.toContain(prefix + 'src/unrelated-tool.ts')
    expect(isComponentModulePath(prefix + 'src/unrelated-tool.ts')).toBe(true)
  })

  it('should accept the record unchanged when unrelated tooling is edited or added', () => {
    lock('src/unrelated-tool.ts', 'export const edited = true')
    lock('src/contract/resolve.ts', "import '../port.js'")
    lock('src/port.ts', 'export const port = true')
    expect(validateComponentGeneration(bytes, source, read)).toEqual(parsed)
  })

  it('should bind the target of an import added to a bound module', () => {
    lock(
      'src/extra-production.ts',
      "import './cycle.js'\nimport { tool } from './unrelated-tool.js'"
    )
    expect(componentModulePaths(source, read)).toContain(prefix + 'src/unrelated-tool.ts')
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'Incomplete component module identities'
    )
  })

  it('should bind a literal dynamic import and a re-export of a bound module', () => {
    lock('src/extra-production.ts', "export { tool } from './unrelated-tool.js'")
    lock('src/unrelated-tool.ts', "const later = () => import('./lazy.js')")
    lock('src/lazy.ts')
    expect(componentModulePaths(source, read)).toEqual(
      expect.arrayContaining([prefix + 'src/unrelated-tool.ts', prefix + 'src/lazy.ts'])
    )
  })

  it('should leave out a module that only a type import names', () => {
    lock('src/extra-production.ts', "import type { Tool } from './unrelated-tool.js'")
    expect(componentModulePaths(source, read)).not.toContain(prefix + 'src/unrelated-tool.ts')
  })

  it.each([
    ['a dynamic import of a variable', 'const load = (name) => import(name)', 'dynamic import'],
    ['a dynamic import of a template', 'const load = () => import(`./lazy.js`)', 'dynamic import'],
    ['require', "const tool = require('./unrelated-tool.js')", 'require()'],
    ['an import assignment', "import tool = require('./unrelated-tool.js')", 'require()'],
    [
      'an extensionless import',
      "import './unrelated-tool'",
      'needs a .js, .mjs or .json extension'
    ],
    ['an import outside the tooling', "import '../../index.js'", 'outside the generation scope'],
    ['an import into the Review', "import '../review/src/main.js'", 'outside the generation scope'],
    ['an untracked import', "import './absent.js'", 'untracked file'],
    ['a test import', "import './helper.test.js'", 'test, fixture or document'],
    ['a fixture import', "import './fixtures/data.json'", 'test, fixture or document'],
    ['a query import', "import './unrelated-tool.js?raw'", 'query or fragment'],
    ['a subpath import', "import '#internal'", 'outside the sanctioned packages'],
    ['the package itself', "import '@dcl/one-ui'", 'outside the sanctioned packages'],
    ['an unsanctioned package', "import 'left-pad'", 'outside the sanctioned packages'],
    ['an unprefixed built-in', "import 'fs'", 'outside the sanctioned packages'],
    ['an unterminated literal', "const text = 'open", 'Unterminated string literal'],
    ['an unbalanced bracket', 'const value = (1', 'Unbalanced bracket']
  ])('should fail closed when a bound module has %s', (_case, text, message) => {
    lock('src/helper.test.ts')
    lock('src/fixtures/data.json', '{}')
    lock('review/src/main.ts')
    contents.set('library/ui/index.ts', '// index')
    source.files['library/ui/index.ts'] = { sha256: hash('// index') }
    lock('src/extra-production.ts', text)
    expect(() => componentModulePaths(source, read)).toThrow(message)
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(message)
  })

  it('should fail closed when two tracked files could answer one import', () => {
    lock('src/extra-production.js')
    expect(() => componentModulePaths(source, read)).toThrow('ambiguous import')
  })

  it('should allow sanctioned packages, their subpaths and node built-ins', () => {
    lock(
      'src/extra-production.ts',
      "import { z } from 'zod'\nimport 'zod/v4'\nimport path from 'node:path'\nimport fs from 'node:fs/promises'"
    )
    expect(componentModulePaths(source, read)).toContain(prefix + 'src/extra-production.ts')
  })

  it('should ignore import text inside strings, templates, comments and regular expressions', () => {
    lock(
      'src/extra-production.ts',
      [
        'const a = "import \'./unrelated-tool.js\'"',
        "const b = `import x from './unrelated-tool.js' ${1 + 1} require('y')`",
        "// import './unrelated-tool.js'",
        "/* export * from './unrelated-tool.js' */",
        "const c = /import\\('x'\\)/.test(a)",
        'const d = { import: 1, require: 2 }.import',
        'const e = import.meta.url'
      ].join('\n')
    )
    expect(componentModulePaths(source, read)).not.toContain(prefix + 'src/unrelated-tool.ts')
  })

  it('should read an import inside a template substitution', () => {
    lock('src/extra-production.ts', "const a = `${await import('./unrelated-tool.js')}`")
    expect(componentModulePaths(source, read)).toContain(prefix + 'src/unrelated-tool.ts')
  })

  it('should detect a deleted bound module', () => {
    contents.delete(prefix + 'src/cycle.ts')
    Reflect.deleteProperty(source.files, prefix + 'src/cycle.ts')
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow('untracked file')
  })

  it('should detect a renamed bound module', () => {
    contents.delete(prefix + 'src/cycle.ts')
    Reflect.deleteProperty(source.files, prefix + 'src/cycle.ts')
    lock('src/loop.ts', "export * from './extra-production.js'")
    lock('src/extra-production.ts', "import './loop.js'")
    expect(componentModulePaths(source, read)).toContain(prefix + 'src/loop.ts')
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'Invalid component module identity'
    )
  })

  it('should refuse module bytes that differ from the lock', () => {
    contents.set(prefix + 'src/cycle.ts', "import './unrelated-tool.js'")
    expect(() => componentModulePaths(source, read)).toThrow(
      'Component module differs from source lock: ' + prefix + 'src/cycle.ts'
    )
  })

  it('should require a module reader', () => {
    expect(() => componentModulePaths(source, undefined as never)).toThrow('module reader')
  })

  it.each([
    ['a path alias', '{"compilerOptions":{"paths":{"x":["./src/unrelated-tool.ts"]}}}'],
    ['a base URL', '{"compilerOptions":{"baseUrl":"."}}'],
    ['an inherited configuration', '{"extends":"../tsconfig.json"}'],
    ['comments', '// aliases\n{}']
  ])('should refuse a resolver configuration with %s', (_case, text) => {
    lock('tsconfig.json', text)
    expect(() => componentModulePaths(source, read)).toThrow('component resolver configuration')
  })

  it('should keep a non-code file under src bound because code may read it', () => {
    lock('src/data/table.json', '{}')
    expect(componentModulePaths(source, read)).toContain(prefix + 'src/data/table.json')
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
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'Missing locked component implementation: ' + prefix + 'tsconfig.json'
    )
  })

  it('should reject an omitted retirement ledger pin', () => {
    delete source.files[prefix + 'ledger/retirements.json']
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'Missing locked component implementation: ' + prefix + 'ledger/retirements.json'
    )
  })

  it('should leave unreachable test and fixture modules out of the read-set', () => {
    for (const file of [
      'src/example.test.ts',
      'src/example.spec.ts',
      'src/__tests__/example.ts',
      'src/fixtures/example.ts'
    ])
      source.files[prefix + file] = { sha256: 'not-a-production-pin' }
    expect(componentModulePaths(source, read)).toEqual(modules.map((module) => module.path))
  })

  it('should reject an absent mandatory implementation even if the manifest omits it too', () => {
    delete source.files[prefix + 'source-identity.mjs']
    parsed.modules = parsed.modules.filter(
      (module: Identity) => module.path !== prefix + 'source-identity.mjs'
    )
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
      'Missing locked component implementation'
    )
  })

  it.each(['js', 'jsx', 'ts', 'tsx', 'mjs', 'mts', 'cjs', 'cts'])(
    'should keep an unreachable top-level .%s file a candidate without binding it',
    (extension) => {
      const file = prefix + 'extra-helper.' + extension
      source.files[file] = { sha256: hash(file) }
      expect(isComponentModulePath(file)).toBe(true)
      expect(componentModuleCandidates(source)).toContain(file)
      expect(componentModulePaths(source, read)).not.toContain(file)
      expect(validateComponentGeneration(bytes, source, read)).toEqual(parsed)
    }
  )

  it('should exclude top-level tests and unrelated nested helper folders', () => {
    expect(isComponentModulePath(prefix + 'helper.test.mts')).toBe(false)
    expect(isComponentModulePath(prefix + 'other/helper.mts')).toBe(false)
  })

  it('should reject colliding locked module paths', () => {
    lock('src/CYCLE.ts')
    lock('src/extra-production.ts', "import './cycle.js'\nimport './CYCLE.js'")
    expect(() => componentModulePaths(source, read)).toThrow(
      'Colliding locked component module paths'
    )
  })

  it('should reject unsupported schemas and unknown top-level claims', () => {
    parsed.schema = 1
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
      'Unsupported component generation schema'
    )
    parsed.schema = 2
    Object.assign(parsed, { liveAttestation: true })
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
      'generation record'
    )
  })

  it.each(['source', 'registry', 'inputs', 'transform', 'modules', 'output'])(
    'should reject a missing %s section',
    (section) => {
      Reflect.deleteProperty(parsed, section)
      expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
        'generation record'
      )
    }
  )

  it.each(['repository', 'codeRevision', 'snapshotRevision', 'digestTableHash'])(
    'should reject changed source identity field %s',
    (field) => {
      Object.assign(parsed.source, { [field]: 'changed' })
      expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
        'source differs from lock'
      )
    }
  )

  it('should reject a changed pinned digest table even if source revisions are unchanged', () => {
    source.snapshot.digests['a.json'] = '2'.repeat(64)
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'source differs from lock'
    )
  })

  it('should reject an invalid source identity before trusting claimed provenance', () => {
    source.repository = 'https://example.invalid/fork'
    expect(() => componentSourceIdentity(source)).toThrow(
      'Invalid verified component source identity'
    )
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
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
      expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow()
    }
  )

  it('should reject a syntactically valid forged module object ID against inspected identities', () => {
    parsed.modules[0].indexBlobSha = 'f'.repeat(40)
    expect(() => validateComponentGeneration(encode(parsed), source, read, { modules })).toThrow(
      'module differs from verified inputs'
    )
  })

  it.each(['path', 'digest', 'oid', 'missing-pin'])('should reject invalid config %s', (kind) => {
    if (kind === 'path') parsed.inputs.config.path = prefix + 'unapproved.json'
    if (kind === 'digest') parsed.inputs.config.sha256 = '0'.repeat(64)
    if (kind === 'oid') parsed.inputs.config.indexBlobSha = 'invalid'
    if (kind === 'missing-pin') delete source.files[componentConfigPath]
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow()
  })

  it('should reject forged config object IDs when independently inspected', () => {
    parsed.inputs.config.indexBlobSha = 'f'.repeat(40)
    expect(() => validateComponentGeneration(encode(parsed), source, read, { config })).toThrow(
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
      expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow()
    }
  )

  it('should reject dependency hashes inconsistent with independently inspected package inputs', () => {
    parsed.inputs.dependencies[0].sha256 = 'f'.repeat(64)
    expect(() =>
      validateComponentGeneration(encode(parsed), source, read, { dependencies })
    ).toThrow('dependency differs from verified inputs')
  })

  it.each(['duplicate', 'both', 'case', 'invalid', 'extra-field'])(
    'should reject %s visibility names',
    (kind) => {
      if (kind === 'duplicate') parsed.registry.components.push('Button')
      if (kind === 'both') parsed.registry.internal.push('Button')
      if (kind === 'case') parsed.registry.internal.push('button')
      if (kind === 'invalid') parsed.registry.internal.push('../Unsafe')
      if (kind === 'extra-field') Object.assign(parsed.registry, { captured: true })
      expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow()
    }
  )

  // The record names the registry's components and pins its bytes; the accepted limits stay in promotion.json.
  it('should pin a registry with accepted limits by digest without copying the limits', () => {
    const limited = encode({
      components: [{ name: 'Button', limits: [{ id: 'logo-size', text: 'Pending.' }] }],
      internal: ['NavBar']
    })
    files.set('promotion.json', limited)

    const record = validateComponentGeneration(generate(), source, read)

    expect(record.registry).toEqual({
      path: 'promotion.json',
      sha256: hash(limited.toString()),
      components: ['Button'],
      internal: ['NavBar']
    })
  })

  it('should reject a record that still copies the limits into its registry', () => {
    ;(parsed.registry as Record<string, unknown>).limits = {}
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
      'Invalid component generation registry'
    )
  })

  it('should reject registry names that are not plain names', () => {
    ;(parsed.registry as { components: unknown[] }).components = [{ name: 'Button', limits: [] }]
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
      'Invalid component generation registry names'
    )
  })

  it('should reject registry/output digest disagreement', () => {
    parsed.registry.sha256 = '0'.repeat(64)
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
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
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow()
  })

  it('should reject missing public mappings while preserving the internal mapping hold', () => {
    parsed.output = parsed.output.filter(
      (output: { path: string }) => output.path !== 'Button/Button.figma.ts'
    )
    expect(() => validateComponentGeneration(encode(parsed), source, read)).toThrow(
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
      bytes = Buffer.from(bytes.toString().replace('"schema": 2,', '"schema": 2,\n  "schema": 2,'))
    expect(() => validateComponentGeneration(bytes, source, read)).toThrow(
      'Noncanonical component generation record'
    )
  })

  it('should refuse malformed JSON and non-buffer input', () => {
    expect(() => validateComponentGeneration(Buffer.from('{'), source, read)).toThrow(
      'generation JSON'
    )
    expect(() =>
      validateComponentGeneration('not-bytes' as unknown as Buffer, source, read)
    ).toThrow('must be bytes')
  })

  it('should leave actual output-byte verification to the common inspector', () => {
    parsed.output[0].sha256 = 'f'.repeat(64)
    expect(validateComponentGeneration(encode(parsed), source, read).output[0].sha256).toBe(
      'f'.repeat(64)
    )
  })

  it('should reject a self-listed generation record during creation too', () => {
    files.set(componentGenerationFile, Buffer.from('{}'))
    expect(generate).toThrow('self-listed component plan output')
  })
})

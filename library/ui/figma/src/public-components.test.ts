import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  componentConfigPath,
  componentDependencyPaths,
  componentModulePaths,
  createComponentGeneration
} from '../component-generation.mjs'
import { componentExpectedFiles, publicComponentsProblems } from '../public-components.mjs'

type Source = {
  repository: string
  revision: string
  snapshot: { revision: string; digests: Record<string, string> }
  files: Record<string, { sha256: string }>
}
type Table = Record<string, { sha256: string; origin: Record<string, unknown> }>
type Manifest = {
  source: { codeRevision: string; snapshotRevision: string; digestTableHash: string }
  transform: { id: string }
  modules: { path: string; sha256: string; indexBlobSha: string }[]
  output: { path: string; sha256: string }[]
  inputs: { dependencies: { indexBlobSha: string }[]; config: { indexBlobSha: string } }
}
const scope = 'library/ui/components/'
const installer = 'library/ui/figma/src/install-components.ts'
const ledger = 'library/ui/figma/ledger/retirements.json'
const hash = (bytes: Buffer | string) => createHash('sha256').update(bytes).digest('hex')

// These are synthetic verified identities, not a source capture or a claim about Figma fidelity.
describe('when the offline component guard checks a staged internal plan', () => {
  let root: string
  let source: Source
  let lock: {
    sources: Source[]
    publicComponents: { files: Table }
    publicFoundations?: { files: Table }
  }
  let files: Map<string, Buffer>
  let metadata: Buffer
  const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args], { stdio: 'pipe' })
  const put = (file: string, content: Buffer | string) => {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true })
    writeFileSync(path.join(root, file), content)
  }
  const identity = (file: string) => {
    const bytes = readFileSync(path.join(root, file))
    return {
      path: file,
      sha256: hash(bytes),
      indexBlobSha: git('hash-object', file).toString().trim()
    }
  }
  const reseal = (file: string, bytes: Buffer | string) => {
    put(scope + file, bytes)
    lock.publicComponents.files[file]!.sha256 = hash(bytes)
    git('add', '--', scope + file)
  }
  const editMetadata = (edit: (value: Manifest) => void) => {
    const value = JSON.parse(metadata.toString()) as Manifest
    edit(value)
    reseal('generation.json', JSON.stringify(value, null, 2) + '\n')
  }

  beforeEach(() => {
    root = mkdtempSync(path.join(tmpdir(), 'dcl-component-guard-'))
    git('init', '--quiet')
    source = {
      repository: 'https://github.com/decentraland/figma-design-system',
      revision: 'a'.repeat(40),
      snapshot: { revision: 'b'.repeat(40), digests: { 'pages.json': 'c'.repeat(64) } },
      files: {}
    }
    for (const file of [
      'component-generation.mjs',
      'source-identity.mjs',
      'tsconfig.json',
      'src/install.ts',
      'src/install-components.ts',
      'src/generators/components.ts',
      'src/generators/tokens.ts',
      'src/extra-generator.ts',
      'ledger/retirements.json',
      'review/src/components/figma-module-runtime.tsx',
      'review/src/components/figma-module-runtime.module.css',
      'review/src/components/figma-view-state.ts',
      'review/src/components/figma-interactions.tsx',
      'figma.config.json'
    ]) {
      const full = 'library/ui/figma/' + file
      const bytes =
        file === 'figma.config.json'
          ? '{"imageFills":"placeholder"}\n'
          : file === 'ledger/retirements.json'
            ? '{"schema":1,"retired":[]}\n'
            : '// synthetic input: ' + file + '\n'
      put(full, bytes)
      source.files[full] = { sha256: hash(bytes) }
    }
    put(
      componentDependencyPaths[0]!,
      '{"name":"synthetic-ui","private":true,"devDependencies":{"postcss":"8.5.32","postcss-modules":"6.0.1"}}\n'
    )
    put(componentDependencyPaths[1]!, 'lockfileVersion: 9.0\nimporters: {}\n')
    put('library/ui/tsconfig.json', '{"compilerOptions":{}}\n')
    files = new Map([
      ['NavBar/NavBar.tsx', Buffer.from('export const NavBar = () => null\n')],
      ['NavBar/NavBar.module.css', Buffer.from('.root {}\n')],
      [
        'NavBar/NavBar.module.css.d.ts',
        Buffer.from('declare const styles: {readonly root: string}; export default styles\n')
      ],
      ['runtime/figma-module-runtime.tsx', Buffer.from('export const runtime = true\n')],
      ['runtime/figma-module-runtime.module.css', Buffer.from('.node {}\n')],
      [
        'runtime/figma-module-runtime.module.css.d.ts',
        Buffer.from('declare const styles: {readonly node: string}; export default styles\n')
      ],
      ['runtime/figma-view-state.ts', Buffer.from('export const state = true\n')],
      ['runtime/figma-interactions.tsx', Buffer.from('export const interactions = true\n')],
      ['promotion.json', Buffer.from('{"components":[],"internal":["NavBar"]}\n')]
    ])
    metadata = createComponentGeneration({
      source,
      files,
      config: identity(componentConfigPath),
      modules: componentModulePaths(source).map(identity),
      dependencies: componentDependencyPaths.map(identity)
    })
    files.set('generation.json', metadata)
    lock = {
      sources: [source],
      publicComponents: {
        files: Object.fromEntries(
          componentExpectedFiles(JSON.parse(metadata.toString()), hash(metadata))
        )
      }
    }
    for (const [file, bytes] of files) put(scope + file, bytes)
    git('add', '.')
  })
  afterEach(() => {
    rmSync(root, { recursive: true, force: true })
  })

  describe('and a clean checkout runs the real offline lock entrypoint before dependency installation', () => {
    let checkOutput: string
    let foundationTable: Table
    let fontFiles: string[]
    beforeEach(() => {
      // Copy only the dependency-free checker implementation into the isolated repository.
      for (const name of [
        'lock-digests.mjs',
        'public-components.mjs',
        'public-foundations.mjs',
        'public-output.mjs',
        'public-components-policy.mjs',
        'component-generation.mjs',
        'source-identity.mjs'
      ]) {
        const file = 'library/ui/figma/' + name
        const bytes = readFileSync(fileURLToPath(new URL('../' + name, import.meta.url)))
        put(file, bytes)
        source.files[file] = { sha256: hash(bytes) }
      }
      Object.assign(source, {
        deviationCodes: { fixture: 'Synthetic dependency-free checker fixture.' }
      })
      for (const entry of Object.values(source.files))
        Object.assign(entry, { fds: null, deviation: 'fixture' })
      foundationTable = JSON.parse(readFileSync(path.resolve('upstream.lock.json'), 'utf8'))
        .publicFoundations.files as Table
      fontFiles = Object.keys(foundationTable).filter((file) => file.endsWith('.woff2'))
      for (const [file, entry] of Object.entries(foundationTable)) {
        if (entry.origin.kind === 'generated') entry.origin.revision = source.snapshot.revision
        const bytes = readFileSync(
          file.endsWith('.woff2')
            ? path.resolve('node_modules/@fontsource-variable/inter/files', path.basename(file))
            : path.resolve('foundations', file)
        )
        expect(hash(bytes)).toBe(entry.sha256)
        put('library/ui/foundations/' + file, bytes)
      }
      lock.publicFoundations = { files: foundationTable }
      metadata = createComponentGeneration({
        source,
        files: new Map([...files].filter(([file]) => file !== 'generation.json')),
        config: identity(componentConfigPath),
        modules: componentModulePaths(source).map(identity),
        dependencies: componentDependencyPaths.map(identity)
      })
      files.set('generation.json', metadata)
      put(scope + 'generation.json', metadata)
      lock.publicComponents.files = Object.fromEntries(
        componentExpectedFiles(JSON.parse(metadata.toString()), hash(metadata))
      )
      put('library/ui/upstream.lock.json', JSON.stringify(lock))
      git('add', '.')
      for (const file of fontFiles) {
        const relative = 'library/ui/foundations/' + file
        const bytes = readFileSync(path.join(root, relative))
        const pointer = `version https://git-lfs.github.com/spec/v1\noid sha256:${hash(bytes)}\nsize ${bytes.length}\n`
        const oid = execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: pointer
        })
          .toString()
          .trim()
        git('update-index', '--cacheinfo', `100644,${oid},${relative}`)
      }
      checkOutput = execFileSync(process.execPath, ['library/ui/figma/lock-digests.mjs'], {
        cwd: root,
        encoding: 'utf8'
      })
    })
    it('should accept all 39 foundations and 10 component files with seven materialized fonts and no node_modules', () => {
      expect(Object.keys(foundationTable)).toHaveLength(39)
      expect(fontFiles).toHaveLength(7)
      expect(publicComponentsProblems(root, lock)).toEqual([])
      expect(checkOutput).toContain('files match upstream.lock.json')
      expect(existsSync(path.join(root, 'library/ui/node_modules'))).toBe(false)
      expect(existsSync(path.join(root, 'node_modules'))).toBe(false)
    })
    it('should still refuse a package self-reference in the real checker with no dependencies installed', () => {
      put(
        'library/ui/figma/package.json',
        '{"name":"postcss","exports":{".":"./audit-dependency/postcss.mjs"}}'
      )
      put('library/ui/figma/audit-dependency/postcss.mjs', 'throw new Error("must not evaluate")')
      expect(publicComponentsProblems(root, lock)).toEqual([
        'unexpected component resolver control: library/ui/figma/package.json'
      ])
      expect(() =>
        execFileSync(process.execPath, ['library/ui/figma/lock-digests.mjs'], {
          cwd: root,
          encoding: 'utf8'
        })
      ).toThrow(
        expect.objectContaining({
          status: 1,
          stdout: expect.stringContaining(
            'PUBLIC unexpected component resolver control: library/ui/figma/package.json'
          )
        })
      )
    })
  })

  it('should accept the canonical staged plan, bindings and independently classified origins', () => {
    expect(publicComponentsProblems(root, lock)).toEqual([])
  })

  describe('and a public-only registry omits the optional internal list', () => {
    beforeEach(() => {
      files.delete('generation.json')
      files.set('promotion.json', Buffer.from('{"components":["NavBar"]}\n'))
      files.set('NavBar/NavBar.figma.ts', Buffer.from('export const mapping = true\n'))
      metadata = createComponentGeneration({
        source,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source).map(identity),
        dependencies: componentDependencyPaths.map(identity)
      })
      files.set('generation.json', metadata)
      lock.publicComponents.files = Object.fromEntries(
        componentExpectedFiles(JSON.parse(metadata.toString()), hash(metadata))
      )
      for (const [file, bytes] of files) put(scope + file, bytes)
      git('add', '.')
    })
    it('should use the producer registry normalization and retain public mapping integrity', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([])
    })
  })

  describe.each([
    'library/ui/figma/component-generation.mts',
    'library/ui/figma/src/generators/components.tsx'
  ])('and TSX could discover an untracked implementation at %s', (shadow) => {
    beforeEach(() => {
      put(shadow, 'export const override = true')
    })
    it('should refuse the otherwise valid manifest and output table', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'untracked component implementation: ' + shadow
      ])
    })
  })

  describe('and a root implementation is tracked without a source pin', () => {
    beforeEach(() => {
      put('library/ui/figma/component-generation.mts', 'export const override = true')
      git('add', '.')
    })
    it('should require the independently inventoried alternative in the locked read-set', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'component implementation inventory differs from source lock'
      ])
    })
  })

  describe('and the last component has been retired', () => {
    let previousRuntime: Table[string]
    beforeEach(() => {
      previousRuntime = lock.publicComponents.files['runtime/figma-view-state.ts']!
      files.clear()
      files.set('promotion.json', Buffer.from('{"components":[],"internal":[]}\n'))
      metadata = createComponentGeneration({
        source,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source).map(identity),
        dependencies: componentDependencyPaths.map(identity)
      })
      files.set('generation.json', metadata)
      lock.publicComponents.files = Object.fromEntries(
        componentExpectedFiles(JSON.parse(metadata.toString()), hash(metadata))
      )
      rmSync(path.join(root, scope), { recursive: true })
      for (const [file, bytes] of files) put(scope + file, bytes)
      git('add', '-A')
    })
    it('should verify the registry and generation record without retaining unused runtime', () => {
      expect(Object.keys(lock.publicComponents.files).sort()).toEqual([
        'generation.json',
        'promotion.json'
      ])
      expect(publicComponentsProblems(root, lock)).toEqual([])
    })
    it('should reject a stale runtime even when its previous origin and bytes are pinned', () => {
      put(scope + 'runtime/figma-view-state.ts', 'export const state = true\n')
      lock.publicComponents.files['runtime/figma-view-state.ts'] = previousRuntime
      git('add', '.')
      expect(publicComponentsProblems(root, lock)).toEqual([
        'unexpected public component: runtime/figma-view-state.ts'
      ])
    })
  })

  describe('and output bytes change without being staged', () => {
    beforeEach(() => {
      put(scope + 'NavBar/NavBar.tsx', 'changed\n')
    })
    it('should identify changed materialized and index bytes', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'changed public file: NavBar/NavBar.tsx',
        'public index bytes differ: NavBar/NavBar.tsx'
      ])
    })
  })
  describe('and changed output is staged and the outer table is resealed', () => {
    beforeEach(() => {
      reseal('NavBar/NavBar.tsx', 'changed\n')
    })
    it('should reject the mismatch against the independently emitted manifest hash', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'emitted component digest differs: NavBar/NavBar.tsx'
      ])
    })
  })
  describe('and generated output claims maintained provenance', () => {
    beforeEach(() => {
      lock.publicComponents.files['NavBar/NavBar.tsx']!.origin = {
        kind: 'source',
        repository: 'https://github.com/decentraland/dcl-one'
      }
    })
    it('should classify from the emitted plan rather than the claimed origin', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'component origin differs: NavBar/NavBar.tsx'
      ])
    })
  })
  describe.each([
    [
      'code revision',
      (value: Manifest) => {
        value.source.codeRevision = 'd'.repeat(40)
      },
      'Component generation source differs from lock'
    ],
    [
      'snapshot identity',
      (value: Manifest) => {
        value.source.snapshotRevision = 'd'.repeat(40)
      },
      'Component generation source differs from lock'
    ],
    [
      'digest table identity',
      (value: Manifest) => {
        value.source.digestTableHash = 'd'.repeat(64)
      },
      'Component generation source differs from lock'
    ],
    [
      'transform',
      (value: Manifest) => {
        value.transform.id = 'other'
      },
      'Invalid component generation transform'
    ],
    [
      'module hash',
      (value: Manifest) => {
        value.modules[0]!.sha256 = 'd'.repeat(64)
      },
      'Component module differs from source lock: library/ui/figma/component-generation.mjs'
    ],
    [
      'self-listed metadata',
      (value: Manifest) => {
        value.output.push({ path: 'generation.json', sha256: 'd'.repeat(64) })
      },
      'Invalid or self-listed component plan output'
    ],
    [
      'internal mapping',
      (value: Manifest) => {
        value.output.push({ path: 'NavBar/NavBar.figma.ts', sha256: 'd'.repeat(64) })
      },
      'Unexpected or internal mapping component output: NavBar/NavBar.figma.ts'
    ]
  ])('and metadata alters %s despite an outer reseal', (_label, edit, error) => {
    beforeEach(() => {
      editMetadata(edit)
    })
    it('should reject the exact invalid binding', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: ' + error
      ])
    })
  })
  describe('and the module map is pinned once by the generation record', () => {
    it('should emit origins without a per-output module map', () => {
      for (const entry of Object.values(lock.publicComponents.files))
        expect(Object.hasOwn(entry.origin, 'modules')).toBe(false)
    })
    it('should reject a table that still repeats the module map in an output origin', () => {
      const modules = Object.fromEntries(
        (JSON.parse(metadata.toString()) as Manifest).modules.map((m) => [m.path, m.sha256])
      )
      const entry = lock.publicComponents.files['NavBar/NavBar.tsx']!
      entry.origin = { ...entry.origin, modules }
      expect(publicComponentsProblems(root, lock)).toEqual([
        'component origin differs: NavBar/NavBar.tsx'
      ])
    })
    it('should reject tampered generation metadata whose digest the table does not pin', () => {
      const value = JSON.parse(metadata.toString()) as Manifest
      value.modules[0]!.sha256 = 'd'.repeat(64)
      put(scope + 'generation.json', JSON.stringify(value, null, 2) + '\n')
      git('add', '--', scope + 'generation.json')
      expect(publicComponentsProblems(root, lock)).toEqual(['changed public file: generation.json'])
    })
    // Stops at the table digest, like the tamper case above: a real module change still needs the table resealed.
    it('should refuse a module change whose regenerated generation.json the table does not pin', () => {
      put(installer, '// changed implementation\n')
      source.files[installer] = { sha256: hash('// changed implementation\n') }
      git('add', '--', installer)
      const value = JSON.parse(metadata.toString()) as Manifest
      const module = value.modules.find((entry) => entry.path === installer)!
      Object.assign(module, identity(installer))
      put(scope + 'generation.json', JSON.stringify(value, null, 2) + '\n')
      git('add', '--', scope + 'generation.json')
      expect(publicComponentsProblems(root, lock)).toEqual(['changed public file: generation.json'])
    })
    it('should reject metadata that drops a module despite an outer reseal', () => {
      editMetadata((value) => {
        value.modules = value.modules.filter((entry) => entry.path !== installer)
      })
      expect(publicComponentsProblems(root, lock)).toEqual([
        expect.stringMatching(/^invalid component generation metadata: /)
      ])
    })
    it('should reject a module digest that the source lock and metadata agree on but the file does not have', () => {
      const forged = 'd'.repeat(64)
      source.files[installer] = { sha256: forged }
      editMetadata((value) => {
        value.modules.find((entry) => entry.path === installer)!.sha256 = forged
      })
      expect(publicComponentsProblems(root, lock)).toEqual(['changed public input: ' + installer])
    })
  })
  describe('and a mandatory module is omitted from the source lock', () => {
    beforeEach(() => {
      delete source.files[installer]
    })
    it('should reject the missing independently chosen implementation path', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: Missing locked component implementation: ' +
          installer
      ])
    })
  })
  describe('and the effective retirement ledger pin is omitted', () => {
    beforeEach(() => {
      delete source.files[ledger]
    })
    it('should require the committed ledger independently of the cached snapshot copy', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: Missing locked component implementation: ' + ledger
      ])
    })
  })
  describe('and the effective retirement ledger is missing from the materialized tree', () => {
    beforeEach(() => {
      rmSync(path.join(root, ledger))
    })
    it('should reject the exact missing input before validating manifest claims', () => {
      expect(publicComponentsProblems(root, lock)).toEqual(['missing public input: ' + ledger])
    })
  })
  describe('and a tracked transitive module is omitted while metadata and origins are resealed', () => {
    beforeEach(() => {
      delete source.files['library/ui/figma/src/extra-generator.ts']
      files.delete('generation.json')
      metadata = createComponentGeneration({
        source,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source).map(identity),
        dependencies: componentDependencyPaths.map(identity)
      })
      lock.publicComponents.files = Object.fromEntries(
        componentExpectedFiles(JSON.parse(metadata.toString()), hash(metadata))
      )
      put(scope + 'generation.json', metadata)
      git('add', '--', scope + 'generation.json')
    })
    it('should require complete source pins from the independently inspected tracked inventory', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'component implementation inventory differs from source lock'
      ])
    })
  })
  describe('and a locked module is missing from the materialized tree', () => {
    beforeEach(() => {
      rmSync(path.join(root, installer))
    })
    it('should preserve the exact inspector error instead of masking it as metadata failure', () => {
      expect(publicComponentsProblems(root, lock)).toEqual(['missing public input: ' + installer])
    })
  })
  describe('and a locked module changes without staging', () => {
    beforeEach(() => {
      put(installer, '// tampered implementation\n')
    })
    it('should report both materialized and index identity violations', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'changed public input: ' + installer,
        'input index bytes differ: ' + installer
      ])
    })
  })
  describe.each([
    installer,
    ledger,
    componentConfigPath,
    'library/ui/figma/tsconfig.json',
    ...componentDependencyPaths
  ])('and the pinned input %s changes in both the index and tree', (file) => {
    beforeEach(() => {
      put(file, 'changed input\n')
      git('add', '--', file)
    })
    it('should reject changed input bytes despite a matching staged identity', () => {
      expect(publicComponentsProblems(root, lock)).toEqual(['changed public input: ' + file])
    })
  })
  describe.each([
    [
      'module',
      (value: Manifest) => {
        value.modules[0]!.indexBlobSha = 'd'.repeat(40)
      },
      'Component generation module differs from verified inputs'
    ],
    [
      'dependency',
      (value: Manifest) => {
        value.inputs.dependencies[0]!.indexBlobSha = 'd'.repeat(40)
      },
      'Component generation dependency differs from verified inputs'
    ],
    [
      'configuration',
      (value: Manifest) => {
        value.inputs.config.indexBlobSha = 'd'.repeat(40)
      },
      'Component generation config differs from verified inputs'
    ]
  ])('and metadata forges the %s index identity', (_label, edit, error) => {
    beforeEach(() => {
      editMetadata(edit)
    })
    it('should compare the claimed blob against independently inspected inputs', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: ' + error
      ])
    })
  })
  describe('and generation metadata is malformed JSON despite an outer reseal', () => {
    beforeEach(() => {
      reseal('generation.json', '{invalid JSON')
    })
    it('should reject before trusting any manifest output identity', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: Invalid component generation JSON'
      ])
    })
  })
  describe('and the emitted registry changes visibility', () => {
    beforeEach(() => {
      reseal('promotion.json', '{"components":["NavBar"],"internal":[]}\n')
    })
    it('should reject the registry digest independently of its claimed maintained origin', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'component registry differs from emitted plan'
      ])
    })
  })
  describe.each(['NavBar/NavBar.tsx', 'generation.json'])('and %s is missing', (file) => {
    beforeEach(() => {
      rmSync(path.join(root, scope, file))
    })
    it('should report the exact missing output', () => {
      expect(publicComponentsProblems(root, lock)).toEqual(['missing public file: ' + file])
    })
  })
  describe('and an internal mapping appears outside the emitted output set', () => {
    beforeEach(() => {
      put(scope + 'NavBar/NavBar.figma.ts', 'export default {}\n')
    })
    it('should reject the extra file before consuming metadata', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'extra public file: NavBar/NavBar.figma.ts'
      ])
    })
  })
  describe('and a component contains an LFS pointer', () => {
    beforeEach(() => {
      reseal(
        'NavBar/NavBar.tsx',
        'version https://git-lfs.github.com/spec/v1\noid sha256:' + 'a'.repeat(64) + '\nsize 1\n'
      )
    })
    it('should reject the pointer even when it is staged and pinned', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'unmaterialized public file: NavBar/NavBar.tsx'
      ])
    })
  })
  describe('and an output becomes a symlink', () => {
    beforeEach(() => {
      rmSync(path.join(root, scope, 'NavBar/NavBar.tsx'))
      symlinkSync('../promotion.json', path.join(root, scope, 'NavBar/NavBar.tsx'))
    })
    it('should reject the symlink without following it', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'public output contains a symlink: NavBar/NavBar.tsx',
        'missing public file: NavBar/NavBar.tsx'
      ])
    })
  })
  describe('and the components directory has no output table', () => {
    beforeEach(() => {
      delete (lock as Partial<typeof lock>).publicComponents
    })
    it('should reject a present directory without independent pins', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'public output has no pinned file table'
      ])
    })
  })
  describe('and neither a components directory nor output table exists', () => {
    beforeEach(() => {
      rmSync(path.join(root, scope), { recursive: true })
      delete (lock as Partial<typeof lock>).publicComponents
    })
    it('should preserve optional components behavior for a foundations-only tree', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([])
    })
  })
  describe('and a components directory has no generation metadata or pin', () => {
    beforeEach(() => {
      rmSync(path.join(root, scope, 'generation.json'))
      delete lock.publicComponents.files['generation.json']
      git('rm', '--cached', '--', scope + 'generation.json')
    })
    it('should refuse to treat the incomplete directory as absent', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'missing component generation metadata or registry'
      ])
    })
  })
})

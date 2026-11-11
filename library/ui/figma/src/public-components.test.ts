import { execFileSync, spawnSync } from 'node:child_process'
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
  const read = (file: string) => readFileSync(path.join(root, file))
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
      'module-graph.mjs',
      'source-identity.mjs',
      'tsconfig.json',
      'src/install-main.ts',
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
            : file === 'tsconfig.json'
              ? '{"compilerOptions":{}}\n'
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
      read,
      files,
      config: identity(componentConfigPath),
      modules: componentModulePaths(source, read).map(identity),
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
        'key-guard.mjs',
        'component-generation.mjs',
        'module-graph.mjs',
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
        read,
        files: new Map([...files].filter(([file]) => file !== 'generation.json')),
        config: identity(componentConfigPath),
        modules: componentModulePaths(source, read).map(identity),
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

    describe('and a hand-written component is copied from the template', () => {
      const lockFile = 'library/ui/upstream.lock.json'
      const run = (...args: string[]) => {
        const { status, stdout, stderr } = spawnSync(
          process.execPath,
          ['library/ui/figma/lock-digests.mjs', ...args],
          { cwd: root, encoding: 'utf8' }
        )
        return { status, stdout, stderr }
      }
      const writtenLock = () => JSON.parse(readFileSync(path.join(root, lockFile), 'utf8'))
      beforeEach(() => {
        for (const file of [
          'Component.tsx',
          'Component.module.css',
          'Component.test.tsx',
          'Component.figma.ts'
        ])
          put(
            scope + 'Badge/' + file.replace('Component', 'Badge'),
            readFileSync(path.resolve('templates/component', file), 'utf8').replaceAll(
              'Component',
              'Badge'
            )
          )
        git(
          'add',
          '--',
          'library/ui/components',
          'library/ui/figma',
          lockFile,
          'library/ui/foundations/index.ts'
        )
      })

      it('should pass the inspector and the lock check with no lock entry for it', () => {
        expect(publicComponentsProblems(root, lock)).toEqual([])
        expect(run()).toMatchObject({ status: 0 })
        expect(
          Object.keys(lock.publicComponents.files).some((file) => file.startsWith('Badge/'))
        ).toBe(false)
      })

      it('should accept a hand-written mapping that follows the mapping contract', () => {
        put(
          scope + 'Badge/Badge.figma.ts',
          [
            '// url=<DCL_FIGMA_FILE>?node-id=1-2',
            '// source=components/Badge/Badge.tsx',
            '// component=Badge',
            '// contract=1',
            '',
            "import figma from 'figma'",
            'export default {',
            '  example: figma.tsx`<Badge>Example</Badge>`,',
            '  imports: ["import { Badge } from \'@dcl/one-ui\'"],',
            "  id: 'Badge',",
            '  metadata: { nestable: true }',
            '}',
            ''
          ].join('\n')
        )
        git('add', '--', 'library/ui/components')
        expect(publicComponentsProblems(root, lock)).toEqual([])
        expect(run()).toMatchObject({ status: 0 })
      })

      it.each([
        [
          'a Figma-key-shaped value',
          'Badge/Badge.tsx',
          "export const node = 'Ab3dEf6hIj9kLm2nOp5qRs'\n",
          'Figma file key or URL in hand-written component file: Badge/Badge.tsx'
        ],
        [
          'a Figma file URL',
          'Badge/Badge.figma.ts',
          '// url=https://www.figma' + '.com/design/abc123/Library?node-id=1-2\nexport {}\n',
          'Figma file key or URL in hand-written component file: Badge/Badge.figma.ts'
        ],
        [
          'the generator header',
          'Badge/Badge.tsx',
          '// Generated by dcl-figma from the Figma design system snapshot. Do not edit.\nexport {}\n',
          'generated or captured output in hand-written component file: Badge/Badge.tsx'
        ],
        [
          'the generated runtime',
          'Badge/Badge.tsx',
          "import { FigmaView } from '../runtime/figma-module-runtime'\nexport { FigmaView }\n",
          'generated or captured output in hand-written component file: Badge/Badge.tsx'
        ],
        [
          'a captured Figma node identity',
          'Badge/Badge.tsx',
          '// Figma: Badge / Badge (15599:30721)\nexport {}\n',
          'generated or captured output in hand-written component file: Badge/Badge.tsx'
        ],
        [
          'a nested import of the generated runtime',
          'Badge/parts/Inner.tsx',
          "import { FigmaView } from '../../runtime/figma-module-runtime'\nexport { FigmaView }\n",
          'generated or captured output in hand-written component file: Badge/parts/Inner.tsx'
        ],
        [
          'an import of the generated runtime directory',
          'Badge/Badge.tsx',
          "import * as runtime from './../runtime'\nexport { runtime }\n",
          'generated or captured output in hand-written component file: Badge/Badge.tsx'
        ],
        [
          'a capture file',
          'Badge/capture.json',
          '{"nodes":[]}\n',
          'unsupported hand-written component file: Badge/capture.json'
        ]
      ])('should refuse a hand-written file with %s', (_, file, content, problem) => {
        put(scope + file, content)
        git(
          'add',
          '--',
          'library/ui/components',
          'library/ui/figma',
          lockFile,
          'library/ui/foundations/index.ts'
        )
        expect(publicComponentsProblems(root, lock)).toEqual([problem])
        expect(run()).toMatchObject({
          status: 1,
          stdout: expect.stringContaining('PUBLIC ' + problem)
        })
      })

      it('should refuse a hand-written file committed as an LFS pointer', () => {
        const bytes = readFileSync(path.join(root, scope + 'Badge/Badge.module.css'))
        const pointer = `version https://git-lfs.github.com/spec/v1\noid sha256:${hash(bytes)}\nsize ${bytes.length}\n`
        const oid = execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: pointer
        })
          .toString()
          .trim()
        git('update-index', '--cacheinfo', `100644,${oid},${scope}Badge/Badge.module.css`)
        expect(publicComponentsProblems(root, lock)).toEqual([
          'LFS pointer in hand-written component file: Badge/Badge.module.css'
        ])
      })

      it('should check the whole directory as installer output when the lock claims a generated origin for one of its files', () => {
        const file = 'Badge/Badge.tsx'
        lock.publicComponents.files[file] = {
          sha256: hash(readFileSync(path.join(root, scope + file))),
          origin: { ...lock.publicComponents.files['NavBar/NavBar.tsx']!.origin }
        }
        const problems = publicComponentsProblems(root, lock)
        // Pinning one file hands the directory to the installer, so its other files are extra outputs.
        expect(problems).toContain('extra public file: Badge/Badge.module.css')
        expect(problems).not.toEqual([])
      })

      it('should refuse a lock entry that claims a generated origin for a one-file hand-written directory', () => {
        put(scope + 'Tag/Tag.tsx', 'export const Tag = () => null\n')
        git('add', '--', 'library/ui/components')
        lock.publicComponents.files['Tag/Tag.tsx'] = {
          sha256: hash(readFileSync(path.join(root, scope + 'Tag/Tag.tsx'))),
          origin: { ...lock.publicComponents.files['NavBar/NavBar.tsx']!.origin }
        }
        expect(publicComponentsProblems(root, lock)).toContain(
          'unexpected public component: Tag/Tag.tsx'
        )
      })

      it('should refuse an untracked hand-written file', () => {
        put(scope + 'Badge/Badge.extra.ts', 'export const extra = true\n')
        expect(publicComponentsProblems(root, lock)).toEqual([
          'untracked hand-written component file: Badge/Badge.extra.ts'
        ])
      })

      it('should refuse a directory that differs from an installed component only by case', () => {
        // Index only: a case-insensitive file system would write it over NavBar/NavBar.tsx.
        const oid = execFileSync('git', ['-C', root, 'hash-object', '-w', '--stdin'], {
          input: 'export const NAVBAR = () => null\n'
        })
          .toString()
          .trim()
        git('update-index', '--add', '--cacheinfo', `100644,${oid},${scope}NAVBAR/NAVBAR.tsx`)
        expect(publicComponentsProblems(root, lock)).toContain(
          'hand-written component collides with an installed component: NAVBAR'
        )
      })

      it('should refuse a hand-written directory that is not a component name', () => {
        put(scope + 'badge-v2/Badge.tsx', 'export const Badge = () => null\n')
        git(
          'add',
          '--',
          'library/ui/components',
          'library/ui/figma',
          lockFile,
          'library/ui/foundations/index.ts'
        )
        expect(publicComponentsProblems(root, lock)).toEqual([
          'unsafe hand-written component directory: badge-v2'
        ])
      })

      describe('and lock-digests --write reseals the lock', () => {
        it('should rewrite stale public tables into a lock the check accepts, without pinning it', () => {
          const stale = writtenLock()
          stale.publicComponents.files['generation.json'].sha256 = 'f'.repeat(64)
          delete stale.publicFoundations
          put(lockFile, JSON.stringify(stale))
          git(
            'add',
            '--',
            'library/ui/components',
            'library/ui/figma',
            lockFile,
            'library/ui/foundations/index.ts'
          )
          expect(run()).toMatchObject({
            status: 1,
            stdout: expect.stringContaining('PUBLIC changed public file: generation.json')
          })
          expect(run('--write')).toMatchObject({ status: 0, stderr: '' })
          const written = writtenLock()
          expect(written.publicComponents.files).toEqual(lock.publicComponents.files)
          expect(written.publicFoundations.files).toEqual(foundationTable)
          expect(run()).toMatchObject({ status: 0 })
        })

        it('should pin a changed foundation source file at its staged bytes', () => {
          const edited = '// edited foundation entrypoint\n'
          put('library/ui/foundations/index.ts', edited)
          git(
            'add',
            '--',
            'library/ui/components',
            'library/ui/figma',
            lockFile,
            'library/ui/foundations/index.ts'
          )
          expect(run()).toMatchObject({ status: 1 })
          expect(run('--write')).toMatchObject({ status: 0 })
          expect(writtenLock().publicFoundations.files['index.ts']).toEqual({
            sha256: hash(edited),
            origin: { kind: 'source', repository: 'https://github.com/decentraland/dcl-one' }
          })
          expect(run()).toMatchObject({ status: 0 })
        })

        it('should keep a generated foundation pin the manifest no longer lists, so the check still fails', () => {
          const sourceJson = JSON.parse(
            readFileSync(path.join(root, 'library/ui/foundations/source.json'), 'utf8')
          )
          sourceJson.output = sourceJson.output.filter(
            (entry: { path: string }) => entry.path !== 'theme.ts'
          )
          put('library/ui/foundations/source.json', JSON.stringify(sourceJson, null, 2) + '\n')
          git('add', '--', 'library/ui/foundations/source.json')
          expect(run('--write')).toMatchObject({ status: 0 })
          expect(writtenLock().publicFoundations.files['theme.ts']).toEqual(
            foundationTable['theme.ts']
          )
          expect(run()).toMatchObject({
            status: 1,
            stdout: expect.stringContaining(
              'PUBLIC generated origin classification differs: theme.ts'
            )
          })
        })

        it('should keep the component table, exit 1 and fail the check when the generation record is stale', () => {
          put('library/ui/figma/src/install.ts', '// a changed installer\n')
          git(
            'add',
            '--',
            'library/ui/components',
            'library/ui/figma',
            lockFile,
            'library/ui/foundations/index.ts'
          )
          const written = run('--write')
          expect(written.status).toBe(1)
          expect(written.stderr).toContain(
            'kept the publicComponents table: Component module differs from source lock: library/ui/figma/src/install.ts'
          )
          expect(writtenLock().publicComponents.files).toEqual(lock.publicComponents.files)
          expect(run()).toMatchObject({ status: 1 })
        })
      })
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
        read,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source, read).map(identity),
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
        read,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source, read).map(identity),
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
  describe('and a locked tooling module outside the read-set is edited and resealed', () => {
    const tool = 'library/ui/figma/src/extra-generator.ts'
    beforeEach(() => {
      put(tool, '// edited tooling\n')
      source.files[tool] = { sha256: hash('// edited tooling\n') }
      git('add', '--', tool)
    })
    it('should accept the unchanged generation record', () => {
      expect(
        JSON.parse(metadata.toString()).modules.map((entry: { path: string }) => entry.path)
      ).not.toContain(tool)
      expect(publicComponentsProblems(root, lock)).toEqual([])
    })
  })
  describe('and a bound module gains an import of a locked tooling module', () => {
    beforeEach(() => {
      const bytes = "import './extra-generator.js'\n"
      put(installer, bytes)
      source.files[installer] = { sha256: hash(bytes) }
      git('add', '--', installer)
    })
    it('should refuse the record until the newly reached module is bound', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: Incomplete component module identities'
      ])
    })
  })
  describe('and a bound module loads code the closure cannot follow', () => {
    beforeEach(() => {
      const bytes = 'export const load = (name) => import(name)\n'
      put(installer, bytes)
      source.files[installer] = { sha256: hash(bytes) }
      git('add', '--', installer)
    })
    it('should fail closed', () => {
      expect(publicComponentsProblems(root, lock)).toEqual([
        'invalid component generation metadata: A dynamic import needs one string literal in ' +
          installer
      ])
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
        read,
        files,
        config: identity(componentConfigPath),
        modules: componentModulePaths(source, read).map(identity),
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

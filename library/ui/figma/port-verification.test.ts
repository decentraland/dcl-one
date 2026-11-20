import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { blobDigests } from './lock-digests.mjs'
import {
  REQUIRED_CHECKS,
  exemptReferences,
  isRecordExempt,
  main,
  portInputHash,
  recordProblems,
  scanProblems,
  trackedBlobsAt,
  trackedFilesAt
} from './port-verification.mjs'

const LOCK = 'library/ui/upstream.lock.json'
const RECORD = 'library/ui/port-verification.json'
const REVISION = 'a'.repeat(40)
const SNAPSHOT_REVISION = 'c'.repeat(40)
let repo: string
let out: string

const git = (...args: string[]) =>
  execFileSync(
    'git',
    ['-C', repo, '-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args],
    { stdio: 'pipe' }
  )
    .toString()
    .trim()
const put = async (file: string, content: string) => {
  await mkdir(path.dirname(path.join(repo, file)), { recursive: true })
  await writeFile(path.join(repo, file), content)
}
const commit = (message: string) => {
  git('add', '-A')
  git('commit', '--quiet', '--no-gpg-sign', '-m', message)
  return git('rev-parse', 'HEAD')
}
type Pin = {
  sha256: string
  fds: { path: string; sha256: string } | null
  deviation?: string[]
  note?: string
}
type LockValue = {
  sources: ({ files: Record<string, Pin>; deviationCodes: Record<string, string | null> } & Record<
    string,
    unknown
  >)[]
} & Record<string, unknown>
type LockOptions = {
  revision?: string
  note?: string
  digest?: string
  components?: string
  edit?: (lock: LockValue) => void
}
const lockValue = ({
  revision = REVISION,
  note = 'v1',
  digest = 'd',
  components = 'e',
  edit
}: LockOptions = {}) => {
  const value: LockValue = {
    sources: [
      {
        repository: 'https://github.com/decentraland/figma-design-system',
        revision,
        note,
        changes: ['A narrative entry.'],
        deviationCodes: { 'dcl-one-only': 'Owned by this tree.', note: null },
        snapshot: { revision: SNAPSHOT_REVISION, digests: { 'pages.json': digest.repeat(64) } },
        files: {
          'library/ui/figma/a.ts': {
            sha256: '1'.repeat(64),
            fds: null,
            deviation: ['dcl-one-only']
          },
          'library/ui/figma/test/a.test.ts': {
            sha256: '2'.repeat(64),
            fds: { path: 'upstream/test/a.test.ts', sha256: '3'.repeat(64) },
            deviation: ['dcl-one-only']
          }
        }
      }
    ],
    publicFoundations: { files: { 'foundations.css': { sha256: 'f'.repeat(64) } } },
    publicComponents: { files: { 'promotion.json': { sha256: components.repeat(64) } } }
  }
  edit?.(value)
  return value
}
const passing = () => Object.fromEntries(REQUIRED_CHECKS.map((name) => [name, { result: 'pass' }]))
// A record that reports passing checks for whatever the working tree will commit next.
const recordFor = (overrides: Record<string, unknown> = {}) => {
  git('add', '-A')
  const tree = git('write-tree')
  const head = git('commit-tree', tree, '-m', 'probe')
  const lock = JSON.parse(git('show', `${head}:${LOCK}`))
  return {
    schema: 2,
    portInputHash: portInputHash(lock, trackedFilesAt(repo, head)),
    fdsRevision: REVISION,
    snapshotRevision: SNAPSHOT_REVISION,
    checks: passing(),
    ...overrides
  }
}
const run = (base: string) => main(['--base', base], repo)
let base: string
beforeEach(async () => {
  repo = await mkdtemp(path.join(tmpdir(), 'port-verification-'))
  git('init', '--quiet')
  await put('library/ui/figma/a.ts', 'export const a = 1\n')
  await put('library/ui/figma/test/a.test.ts', 'it("a", () => {})\n')
  await put('library/ui/figma/MAINTENANCE.md', 'v1\n')
  await put(LOCK, JSON.stringify(lockValue()))
  base = commit('base')
  out = ''
  vi.spyOn(process.stdout, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
  vi.spyOn(process.stderr, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
})

afterEach(async () => {
  vi.restoreAllMocks()
  await rm(repo, { recursive: true, force: true })
})

describe('when the pull request leaves the port inputs alone', () => {
  describe('and the change is outside library/ui/figma', () => {
    beforeEach(async () => {
      await put('library/ui/README.md', 'unrelated\n')
      commit('docs')
    })

    it('should pass with no record', () => {
      expect(run(base)).toBe(0)
      expect(out).toContain('unchanged')
    })
  })

  describe('and only tests, fixtures and Markdown change', () => {
    beforeEach(async () => {
      await put('library/ui/figma/test/a.test.ts', 'it("a", () => { /* edited */ })\n')
      await put('library/ui/figma/test/fixtures/data.json', '{}\n')
      await put('library/ui/figma/MAINTENANCE.md', 'v2\n')
      await put('library/ui/figma/agent-kit/skills/x/SKILL.md', 'skill\n')
      commit('tests and docs')
    })

    it('should pass with no record', () => {
      expect(run(base)).toBe(0)
      expect(out).toContain('unchanged')
    })
  })

  describe.each([
    ['license', (lock: LockValue) => void (lock.sources[0]!.license = { spdx: 'MIT' })],
    ['imported paths', (lock: LockValue) => void (lock.sources[0]!.imported = { a: 'b' })],
    ['narrative changes', (lock: LockValue) => void (lock.sources[0]!.changes = ['Edited.'])],
    [
      'a deviation reason or a new code',
      (lock: LockValue) =>
        void (lock.sources[0]!.deviationCodes = {
          added: 'New.',
          'dcl-one-only': 'Edited.',
          note: null
        })
    ],
    [
      'a per-file note',
      (lock: LockValue) => void (lock.sources[0]!.files['library/ui/figma/a.ts']!.note = 'x')
    ],
    [
      "an exempt file's deviation codes",
      (lock: LockValue) =>
        void (lock.sources[0]!.files['library/ui/figma/test/a.test.ts']!.deviation = ['added'])
    ]
  ])('and only %s changes', (_, edit) => {
    beforeEach(async () => {
      await put(LOCK, JSON.stringify(lockValue({ edit })))
      commit('provenance')
    })

    it('should pass with no record', () => {
      expect(run(base)).toBe(0)
      expect(out).toContain('unchanged')
    })
  })
})

describe('when the head has no lock', () => {
  describe('and the head branched before the library existed', () => {
    let head: string

    beforeEach(() => {
      const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
      head = git('commit-tree', emptyTree, '-m', 'old branch')
    })

    it('should pass and say the head has no lock', () => {
      expect(main(['--base', base, '--head', head], repo)).toBe(0)
      expect(out).toContain('no upstream.lock.json at this head')
    })
  })

  describe('and the head keeps library/ui', () => {
    let head: string

    beforeEach(async () => {
      await rm(path.join(repo, LOCK))
      head = commit('no lock')
    })

    it('should pass', () => {
      expect(main(['--base', base, '--head', head], repo)).toBe(0)
    })
  })

  describe('and the head is not a commit', () => {
    it('should fail, naming the head', () => {
      expect(main(['--base', base, '--head', 'f'.repeat(40)], repo)).toBe(1)
      expect(out).toContain('the head')
      expect(out).toContain('not a commit')
    })
  })
})

describe('when the pull request changes a port input', () => {
  beforeEach(async () => {
    await put('library/ui/figma/a.ts', 'export const a = 2\n')
  })

  describe('and no record is committed', () => {
    beforeEach(() => {
      commit('port without record')
    })

    it('should fail, saying the record is not committed', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('not committed')
    })
  })

  describe('and a record for exactly these port inputs is committed', () => {
    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      commit('port')
    })

    it('should pass, reporting the passing checks', () => {
      expect(run(base)).toBe(0)
      expect(out).toContain('reports passing checks')
    })
  })

  describe('and only exempt files change after the record', () => {
    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      await put('library/ui/figma/test/a.test.ts', 'it("b", () => {})\n')
      await put('library/ui/figma/MAINTENANCE.md', 'v3\n')
      commit('port, then a test and a doc edit')
    })

    it('should keep the record valid', () => {
      expect(run(base)).toBe(0)
    })
  })

  describe('and a non-exempt file changed after the record was written', () => {
    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      await put('library/ui/figma/a.ts', 'export const a = 3\n')
      commit('edited after verification')
    })

    it('should fail, saying the record is for other port inputs', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('the record is for other port inputs')
    })
  })

  describe('and a new untracked-by-the-lock file appears after the record', () => {
    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      await put('library/ui/figma/src/new.ts', 'export {}\n')
      commit('new module')
    })

    it('should fail, saying the record is for other port inputs', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('other port inputs')
    })
  })

  describe.each([
    ['the capture digest table', { digest: 'f' }],
    ['the public component table', { components: 'f' }]
  ])('and %s changed after the record was written', (_, options) => {
    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      await put(LOCK, JSON.stringify(lockValue(options)))
      commit('pin changed after verification')
    })

    it('should fail, saying the record is for other port inputs', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('other port inputs')
    })
  })

  describe.each([
    [
      'a port input counterpart',
      (lock: LockValue) =>
        void (lock.sources[0]!.files['library/ui/figma/a.ts']!.fds = {
          path: 'upstream/a.ts',
          sha256: '1'.repeat(64)
        })
    ],
    [
      'an exempt file counterpart',
      (lock: LockValue) =>
        void (lock.sources[0]!.files['library/ui/figma/test/a.test.ts']!.fds = null)
    ],
    [
      "a port input's deviation codes",
      (lock: LockValue) => void (lock.sources[0]!.files['library/ui/figma/a.ts']!.deviation = [])
    ],
    [
      'a new exempt file entry',
      (lock: LockValue) =>
        void (lock.sources[0]!.files['library/ui/figma/test/b.test.ts'] = {
          sha256: '4'.repeat(64),
          fds: null,
          deviation: ['dcl-one-only']
        })
    ],
    ['the FDS code commit', (lock: LockValue) => void (lock.sources[0]!.revision = 'b'.repeat(40))],
    [
      'the capture commit',
      (lock: LockValue) =>
        void ((lock.sources[0]!.snapshot as { revision: string }).revision = 'd'.repeat(40))
    ],
    [
      'the public foundation table',
      (lock: LockValue) => void (lock.publicFoundations = { files: {} })
    ],
    ['a lock field added later', (lock: LockValue) => void (lock.sources[0]!.policy = 'strict')]
  ])('and only %s changes in the lock', (_, edit) => {
    beforeEach(async () => {
      await put('library/ui/figma/a.ts', 'export const a = 1\n')
      await put(LOCK, JSON.stringify(lockValue({ edit })))
      commit('lock-only change')
    })

    it('should require a new record', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('not committed')
    })
  })

  describe('and the committed record uses schema 1', () => {
    beforeEach(async () => {
      const rest: Record<string, unknown> = recordFor()
      delete rest.portInputHash
      await put(RECORD, JSON.stringify({ ...rest, schema: 1, lockHash: 'x', figmaTree: 'y' }))
      commit('old record')
    })

    it('should fail on the unsupported schema', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('unsupported record schema')
    })
  })

  describe('and the committed record is malformed', () => {
    beforeEach(async () => {
      await put(RECORD, '{ not json')
      commit('broken record')
    })

    it('should fail, saying the record is not valid JSON', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('not committed valid JSON')
    })
  })

  describe('and a required check did not pass', () => {
    beforeEach(async () => {
      const checks = { ...passing(), diff: { result: 'fail', detail: '3 changed' } }
      await put(RECORD, JSON.stringify(recordFor({ checks })))
      commit('failing check')
    })

    it('should fail, naming the check', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('check diff did not pass')
    })
  })

  describe('and the base has no library', () => {
    let empty: string

    beforeEach(() => {
      const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
      empty = git('commit-tree', emptyTree, '-m', 'empty')
      commit('port')
    })

    it('should treat the base as a change', () => {
      expect(run(empty)).toBe(1)
    })
  })

  describe('and the base lock cannot be hashed', () => {
    let broken: string

    beforeEach(async () => {
      await put(LOCK, JSON.stringify({ sources: [] }))
      broken = commit('broken base')
      await put(LOCK, JSON.stringify(lockValue()))
      commit('port')
    })

    it('should treat the base as a change', () => {
      expect(run(broken)).toBe(1)
      expect(out).toContain('not committed')
    })
  })

  describe('and the head lock is not valid JSON', () => {
    beforeEach(async () => {
      await put(LOCK, '{ not json')
      commit('broken lock')
    })

    it('should fail, saying the lock is not valid JSON', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('is not valid JSON')
    })
  })

  describe('and the checkout moved past the given head', () => {
    let head: string

    beforeEach(async () => {
      await put(RECORD, JSON.stringify(recordFor()))
      head = commit('port')
      await put('library/ui/figma/a.ts', 'export const a = 4\n')
      commit('later change without a record')
    })

    it('should judge the given head rather than the checkout', () => {
      expect(main(['--base', base, '--head', head], repo)).toBe(0)
      expect(main(['--base', base], repo)).toBe(1)
    })
  })

  describe('and the base is not a commit', () => {
    beforeEach(() => {
      commit('port')
    })

    it('should fail, saying it is not a commit', () => {
      expect(run('f'.repeat(40))).toBe(1)
      expect(out).toContain('not a commit')
    })
  })

  describe('and the committed record reports a failing check', () => {
    let record: string
    let lock: string

    beforeEach(async () => {
      record = JSON.stringify(recordFor({ checks: { ...passing(), diff: { result: 'fail' } } }))
      await put(RECORD, record)
      commit('failing record')
      lock = await readFile(path.join(repo, LOCK), 'utf8')
    })

    it('should leave the record and the lock untouched', async () => {
      expect(run(base)).toBe(1)
      expect(await readFile(path.join(repo, RECORD), 'utf8')).toBe(record)
      expect(await readFile(path.join(repo, LOCK), 'utf8')).toBe(lock)
    })
  })
})

describe('when reading the files of a commit', () => {
  describe('and the commit holds a Git LFS pointer', () => {
    let head: string

    beforeEach(async () => {
      await put(
        'library/ui/figma/font.woff2',
        'version https://git-lfs.github.com/spec/v1\noid sha256:x\n'
      )
      head = commit('pointer')
    })

    it('should refuse the pointer', () => {
      expect(() => trackedFilesAt(repo, head)).toThrow('Git LFS pointer')
    })
  })

  describe('and the commit holds a submodule', () => {
    beforeEach(() => {
      git(
        'update-index',
        '--add',
        '--cacheinfo',
        `160000,${'e'.repeat(40)},library/ui/figma/vendor`
      )
      git('commit', '--quiet', '--no-gpg-sign', '-m', 'submodule')
    })

    it('should refuse the submodule', () => {
      expect(() => trackedFilesAt(repo, git('rev-parse', 'HEAD'))).toThrow('has mode 160000')
    })
  })

  describe('and the commit holds a symlink', () => {
    beforeEach(() => {
      const target = git('hash-object', '-w', '--stdin')
      git('update-index', '--add', '--cacheinfo', `120000,${target},library/ui/figma/link.mjs`)
      git('commit', '--quiet', '--no-gpg-sign', '-m', 'symlink')
    })

    it('should refuse the symlink, whose target could change without the link changing', () => {
      expect(() => trackedFilesAt(repo, git('rev-parse', 'HEAD'))).toThrow('has mode 120000')
    })
  })

  // A path cut at its first tab used to be filed under the shorter name and overwrite the real file's digest.
  describe('and a path holds a tab next to an edited port input', () => {
    let head: string

    beforeEach(async () => {
      await put('library/ui/figma/a.ts\t.orig', 'export const a = 1\n')
      await put('library/ui/figma/a.ts', 'export const a = 2\n')
      head = commit('shadowed edit')
    })

    it('should refuse the path, so a sibling cannot shadow the port input', () => {
      expect(() => trackedFilesAt(repo, head)).toThrow('control character')
    })

    it('should fail the check on the control character', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('control character')
    })
  })
})

describe('when hashing a listing of blobs', () => {
  describe('and a path is listed twice', () => {
    let oid: string

    beforeEach(async () => {
      await put('library/ui/figma/b.ts', 'export {}\n')
      oid = git('hash-object', 'library/ui/figma/b.ts')
    })

    it('should refuse the listing', () => {
      expect(() =>
        blobDigests(repo, [
          { file: 'library/ui/figma/b.ts', oid },
          { file: 'library/ui/figma/b.ts', oid }
        ])
      ).toThrow('names a path twice')
    })
  })
})

describe('when deciding which files are port inputs', () => {
  it.each([
    'library/ui/figma/MAINTENANCE.md',
    'library/ui/figma/agent-kit/skills/implement-figma-design/SKILL.md',
    'library/ui/figma/test/workflows.test.ts',
    'library/ui/figma/test/key-step.ts',
    'library/ui/figma/test/fixtures/synthetic-snapshot/pages.json',
    'library/ui/figma/src/contract/resolve.test.ts',
    'library/ui/figma/review/test/helpers.ts',
    'library/ui/figma/review/test/check.test.ts'
  ])('should exempt %s', (file) => {
    expect(isRecordExempt(file)).toBe(true)
  })

  it.each([
    'library/ui/figma/port-verification.mjs',
    'library/ui/figma/src/contract/test-fixtures.ts',
    'library/ui/figma/src/guard/git-fixture.ts',
    'library/ui/figma/figma.config.json',
    'library/ui/figma/agent-kit/kit.mjs',
    'library/ui/figma/README.MD',
    'library/ui/figma/src/notes.md',
    'library/ui/figma/src/types.test.d.ts',
    'library/ui/figma/port-verification.test.ts',
    'library/ui/figma/review/fixtures/runtime.mjs',
    'library/ui/figma/review/scripts/test/helper.mjs',
    'library/ui/figma/src/__tests__/helper.ts',
    'library/ui/figma/agent-kit/fixtures/kit.json',
    'library/ui/figma/doc.md.ts',
    'library/ui/README.md',
    'library/ui/figma-other/test/a.ts',
    undefined
  ])('should bind %s', (file) => {
    expect(isRecordExempt(file)).toBe(false)
  })

  // The validator runs this scan on every head in CI; this case keeps the tree it ships with clean.
  it('should find no port input in this tree that names an exempt file', () => {
    const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
    const blobs = trackedBlobsAt(root, 'HEAD')
    expect(Object.keys(blobs).length).toBeGreaterThan(100)
    expect(scanProblems(blobs)).toEqual([])
  })

  it.each([
    ["import './test/setup.ts'", 'a side-effect import'],
    ["export * from './test'", 'a directory re-export'],
    ["new URL('../test/data.json', import.meta.url)", 'a URL into a test root'],
    ["new URL('helpers.spec.mjs', import.meta.url)", 'a bare spec basename'],
    ["path.join(dir, 'payload.SPEC.json')", 'a bare spec basename in another case'],
    ["path.join(root, 'fixtures', 'x.json')", 'a bare directory segment'],
    ["path.join(root, 'Fixtures/x.json')", 'a path-like directory segment in another case'],
    [
      'new URL(`./test/${name}.json`, import.meta.url)',
      'an interpolated template into a test root'
    ],
    ['readFileSync(`${here}/test/data.json`)', 'an interpolated template with a test segment'],
    ["x.replace(/'/g, ''); read('./test/a.json')", 'a literal after a regex with a quote'],
    ["join(here, 'review/test')", 'a test root with no leading dot or trailing slash'],
    ["'library/ui/figma/test'", 'a repository path to a test root'],
    ["import.meta.glob('./**/*.ts')", 'a code glob that also loads tests'],
    ["@import './test/x.css';", 'a CSS import'],
    ['<script src="../test/main.tsx"></script>', 'an HTML script'],
    ["read(`${'./test/a.json'}`)", 'a literal nested in a template interpolation'],
    ["execSync('node ./test/run.mjs')", 'a command line with a path token'],
    ["read('./test/rule set.json')", 'a path with a space'],
    [
      "const s = 'a=/'; read('./test/x.json'); const t = '/'",
      'a literal after a string that looks like a regex'
    ],
    ["const r = total\n  / count; const p = './test/a.json'", 'a literal after a wrapped division'],
    [
      "f = (s) => /'/.test(s); read('./test/a.json')",
      'a literal after a quote-bearing regex after an arrow'
    ],
    ["// don't do this\nread('./test/a.json')", 'a literal after a comment with an apostrophe'],
    ['readFile(`notes/README.md`)', 'a Markdown name in a template literal'],
    ["import notes from './notes.md?raw'", 'a Markdown import with a query'],
    ["import a from './a.md#top'", 'a Markdown import with a fragment'],
    ["glob('./**/*.{md,tsx}')", 'a glob that matches Markdown'],
    ["globSync('*.md', { cwd: figmaDir })", 'a bare Markdown glob'],
    ["import { h } from './helpers.test'", 'an extensionless import of a test module'],
    ["import s from '../src/a.spec'", 'an extensionless import of a spec module'],
    ["x('helpers.test')", 'a bare extensionless test basename'],
    ["join(here, 'Test', 'a.json')", 'a capitalized directory word in a path call'],
    ['.a { background: url(./test/x.png) }', 'an unquoted CSS url'],
    ['@import url(../test/x.css);', 'an unquoted CSS import'],
    ['<img src=../test/x.png>', 'an unquoted HTML attribute'],
    ["read('.\\/test\\/x.json')", 'a path with escaped slashes'],
    ["globSync('agent-kit/skills/*/*')", 'a one-level glob over a directory'],
    ["glob('*.MD')", 'a bare Markdown glob in another case'],
    ["glob('*.{mdx,md}')", 'a bare brace glob with Markdown'],
    ["await import('./src/a.test.ts')", 'a dynamic import of a test']
  ])('should flag %s as %s', (code) => {
    expect(exemptReferences('library/ui/figma/tool.mjs', code)).not.toEqual([])
  })

  it('should not flag literals that only look like words', () => {
    const code = [
      "const label = 'testing'; const re = /test/; run('./src/a.ts'); log('mdx')",
      "const title = 'Test'; const size = 'spacing.md'; const sizes = 'sm,md,lg'",
      "const hint = 'see docs/ui-standards.md'; const open = '/*'; const close = '*/'",
      "throw new Error('run the test again'); const label = 'Test'"
    ].join('\n')
    expect(exemptReferences('library/ui/figma/tool.mjs', code)).toEqual([])
  })

  it('should flag the basename of a tracked exempt Markdown file, and only that one', () => {
    const blobs = {
      'library/ui/figma/agent-kit/skills/x/SKILL.md': Buffer.from('# x\n'),
      'library/ui/figma/tool.mjs': Buffer.from("read(join(dir, 'SKILL.md')); read('NOTES.md')\n")
    }
    expect(scanProblems(blobs)).toEqual([
      'library/ui/figma/tool.mjs names the exempt path "SKILL.md"'
    ])
  })

  it('should scan the Figma tooling config, ledgers, catalog, styles and markup, not other JSON', () => {
    const blobs = {
      'library/ui/figma/ledger/retirements.json': Buffer.from(
        '{ "source": "test/fixtures/r.json" }'
      ),
      'library/ui/figma/figma.config.json': Buffer.from(
        '{ "snapshotLock": "./test/fixtures/lock.json" }'
      ),
      'library/ui/figma/review/a.css': Buffer.from("@import './../test/x.css';"),
      'library/ui/figma/tsconfig.json': Buffer.from('{ "exclude": ["test/fixtures"] }')
    }
    expect(scanProblems(blobs)).toEqual([
      'library/ui/figma/ledger/retirements.json names the exempt path "test/fixtures/r.json"',
      'library/ui/figma/figma.config.json names the exempt path "./test/fixtures/lock.json"',
      'library/ui/figma/review/a.css names the exempt path "./../test/x.css"'
    ])
  })

  it('should catch the validator importing with a space before the parenthesis or after a semicolon', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const problems = (code: string) => scanProblems({ [self]: Buffer.from(code) })
    expect(problems("const r = await import ('./rule.mjs')\n")).toContain(
      `${self} imports ./rule.mjs`
    )
    expect(problems("const a = 1; import './rule.mjs'\n")).toContain(`${self} imports ./rule.mjs`)
    expect(problems("  import './rule.mjs'\n")).toContain(`${self} imports ./rule.mjs`)
  })

  it('should not count an exempt Markdown basename inside a sentence', () => {
    const blobs = {
      'library/ui/figma/agent-kit/skills/x/SKILL.md': Buffer.from('# x\n'),
      'library/ui/figma/tool.mjs': Buffer.from("log('see SKILL.md for details')\n")
    }
    expect(scanProblems(blobs)).toEqual([])
  })

  it('should allow exempt roots only in the rule declarations of the validator', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const declarations = "const TEST_ROOTS = ['test/', 'review/test/']\n"
    expect(scanProblems({ [self]: Buffer.from(declarations) })).toEqual([])
    expect(scanProblems({ [self]: Buffer.from(declarations + "const extra = 'test/'\n") })).toEqual(
      [`${self} names the exempt path "test/"`]
    )
  })

  describe('and a glob leaves a comment opener unclosed', () => {
    let text: string

    beforeEach(() => {
      text = "const g = 'src/*'\n".repeat(60000)
    })

    it('should strip comments in linear time', () => {
      const started = Date.now()
      exemptReferences('library/ui/figma/tool.mjs', text)
      expect(Date.now() - started).toBeLessThan(2000)
    })
  })

  it('should look for require in the raw validator text, so comment-like strings cannot hide it', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const hidden =
      "const open = '/*'\nconst m = createRequire(import.meta.url)(name)\nconst close = '*/'\n"
    expect(scanProblems({ [self]: Buffer.from(hidden) })).toContain(
      `${self} loads modules with require`
    )
    // A require named only in a comment fails closed too.
    expect(scanProblems({ [self]: Buffer.from('// never require (x) here\n') })).toContain(
      `${self} loads modules with require`
    )
  })

  it('should leave out only the exact rule declarations of the validator', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const spread =
      "const TEST_ROOTS = ['test/', ...String(readFileSync('library/ui/figma/test/roots.txt')).split(',')]\n"
    expect(scanProblems({ [self]: Buffer.from(spread) })).toContain(
      `${self} names the exempt path "library/ui/figma/test/roots.txt"`
    )
  })

  it('should flag a tracked exempt Markdown name with punctuation around it, not inside a longer name', () => {
    const names = new Set(['MAINTENANCE.md'])
    const flagged = (code: string) => exemptReferences('library/ui/figma/a.ts', code, names)
    for (const code of [
      "x('--doc=MAINTENANCE.md')",
      "x('(MAINTENANCE.md)')",
      "x('MAINTENANCE.md:12')"
    ])
      expect(flagged(code)).not.toEqual([])
    for (const code of [
      "x('XMAINTENANCE.md')",
      "x('MAINTENANCE.mdx')",
      "x('see MAINTENANCE.md here')"
    ])
      expect(flagged(code)).toEqual([])
  })

  it('should stay linear with many line comments and with many literals on one line', () => {
    let started = Date.now()
    exemptReferences('library/ui/figma/a.ts', '// c\nx\n'.repeat(20000))
    expect(Date.now() - started).toBeLessThan(1000)
    started = Date.now()
    exemptReferences('library/ui/figma/c.json', '[' + Array(40000).fill('"abc"').join(',') + ']')
    expect(Date.now() - started).toBeLessThan(1000)
  })

  it('should scan SVG, both ledgers and the Review catalog', () => {
    const named = (file: string) => ({
      [file]: Buffer.from('<a href="./test/x.svg"/> "./test/x.json"')
    })
    for (const file of [
      'library/ui/figma/review/src/a.svg',
      'library/ui/figma/ledger/orphaned-assets.json',
      'library/ui/figma/ledger/retirements.json',
      'library/ui/figma/review/src/review/catalog.json'
    ])
      expect(scanProblems(named(file))).not.toEqual([])
  })

  it('should refuse a validator that loads code or reads an exempt path some other way', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const problems = (code: string) => scanProblems({ [self]: Buffer.from(code) })
    expect(problems('const r = await import(`./test/rule.mjs`)\n')).toContain(
      `${self} imports ./test/rule.mjs`
    )
    expect(problems('const req = createRequire(import.meta.url)\n')).toContain(
      `${self} loads modules with require`
    )
    expect(problems("readFileSync(new URL('./test/roots.json', import.meta.url))\n")).toContain(
      `${self} names the exempt path "./test/roots.json"`
    )
    expect(problems("const TEST_ROOTS = ['test/', 'review/test/']\n")).toEqual([])
  })

  it('should refuse a validator that imports anything beyond its two bound modules and four built-ins', () => {
    const self = 'library/ui/figma/port-verification.mjs'
    const ok = "import { a } from './lock-digests.mjs'\nimport path from 'node:path'\n"
    expect(scanProblems({ [self]: Buffer.from(ok) })).toEqual([])
    expect(
      scanProblems({ [self]: Buffer.from(ok + "import { roots } from './test/rule.mjs'\n") })
    ).toEqual([
      `${self} imports ./test/rule.mjs`,
      `${self} names the exempt path "./test/rule.mjs"`
    ])
    expect(scanProblems({ [self]: Buffer.from("const r = await import('./rule.mjs')\n") })).toEqual(
      [`${self} imports ./rule.mjs`]
    )
  })

  describe('and a port input names an exempt file', () => {
    beforeEach(async () => {
      await put(
        'library/ui/figma/a.ts',
        "export const notes = new URL('helpers.spec.mjs', import.meta.url)\n"
      )
      commit('reads an exempt file')
    })

    it('should fail the check, naming the exempt path', () => {
      expect(run(base)).toBe(1)
      expect(out).toContain('names the exempt path "helpers.spec.mjs"')
    })
  })
})

describe('when printing the port input hash', () => {
  describe('and the head changes a port input', () => {
    let expected: string

    beforeEach(async () => {
      await put('library/ui/figma/a.ts', 'export const a = 5\n')
      expected = recordFor().portInputHash
      commit('head')
    })

    it('should print the hash a record must name, without needing a base', () => {
      expect(main(['--print-hash'], repo)).toBe(0)
      expect(out.trim()).toBe(expected)
    })
  })

  describe('and a port input names an exempt file', () => {
    let stdout: string
    let stderr: string

    beforeEach(async () => {
      await put(
        'library/ui/figma/a.ts',
        "export const notes = new URL('helpers.spec.mjs', import.meta.url)\n"
      )
      commit('reads an exempt file')
      // Replace the shared capture with one that keeps the two streams apart.
      vi.restoreAllMocks()
      stdout = ''
      stderr = ''
      vi.spyOn(process.stdout, 'write').mockImplementation(
        (chunk) => ((stdout += String(chunk)), true)
      )
      vi.spyOn(process.stderr, 'write').mockImplementation(
        (chunk) => ((stderr += String(chunk)), true)
      )
    })

    it('should keep stdout for the hash and send scan problems to stderr', () => {
      expect(main(['--print-hash'], repo)).toBe(1)
      expect(stdout).toBe('')
      expect(stderr).toContain('names the exempt path "helpers.spec.mjs"')
    })

    // The nearest false positive: without --print-hash the check still reports on stdout, as CI reads it.
    it('should still report scan problems on stdout for the check itself', () => {
      expect(run(base)).toBe(1)
      expect(stdout).toContain('names the exempt path "helpers.spec.mjs"')
    })
  })

  describe('and the head has no lock', () => {
    beforeEach(async () => {
      await rm(path.join(repo, LOCK))
      commit('no lock')
    })

    it('should exit non-zero instead of printing text', () => {
      expect(main(['--print-hash'], repo)).toBe(1)
      expect(out).toContain('there is no hash to print')
    })
  })

  it.each([
    [['--print-hash', 'junk', '--head']],
    [['--base', 'HEAD', 'junk']],
    [['--head']],
    [['--print-hash', '--print-hash']],
    [['--base', 'HEAD', '--base', 'HEAD']]
  ])('should refuse the arguments %j', (argv) => {
    expect(main(argv, repo)).toBe(2)
  })

  it.each([
    [['--base']],
    [['--base', '--print-hash']],
    [['--base', '']],
    [['--print-hash', '--head', '']]
  ])('should refuse the missing or empty value in %j', (argv) => {
    expect(main(argv, repo)).toBe(2)
  })

  it('should refuse --print-hash together with --base', () => {
    expect(main(['--print-hash', '--base', base], repo)).toBe(2)
  })
})

describe('when validating a record', () => {
  let lock: LockValue
  let inputHash: string

  beforeEach(() => {
    lock = lockValue()
    inputHash = portInputHash(lock, { 'library/ui/figma/a.ts': 'b'.repeat(64) })
  })

  const valid = () => ({
    schema: 2,
    portInputHash: inputHash,
    fdsRevision: REVISION,
    snapshotRevision: SNAPSHOT_REVISION,
    checks: passing()
  })
  const problems = (record: unknown) => recordProblems(record, { lock, inputHash })

  it('should accept a complete passing record', () => {
    expect(problems(valid())).toEqual([])
  })

  it('should reject extra keys, so a record cannot carry names or identifiers', () => {
    expect(problems({ ...valid(), reviewer: 'someone' })).toContainEqual(
      expect.stringContaining('exactly the keys')
    )
  })

  it('should reject a missing or unknown check', () => {
    const rest = Object.fromEntries(Object.entries(passing()).filter(([name]) => name !== 'diff'))
    expect(problems({ ...valid(), checks: rest })).toContainEqual('check diff is not reported')
    expect(
      problems({ ...valid(), checks: { ...passing(), census: { result: 'pass' } } })
    ).toContainEqual('check census is not a known check')
  })

  it('should reject a detail that is not short plain text', () => {
    const checks = { ...passing(), diff: { result: 'pass', detail: 'see https://example.com/x?y' } }
    expect(problems({ ...valid(), checks })).toContainEqual('check diff has an unsupported detail')
  })

  it('should reject a record for another capture commit', () => {
    expect(problems({ ...valid(), snapshotRevision: 'd'.repeat(40) })).toContainEqual(
      'the record is for another figma-design-system capture commit'
    )
  })

  it('should reject another FDS commit and a non-object record', () => {
    expect(problems({ ...valid(), fdsRevision: 'b'.repeat(40) })).toContainEqual(
      'the record is for another figma-design-system commit'
    )
    expect(problems([])).toEqual(['the record is not an object'])
  })
})

describe('when hashing the port inputs', () => {
  let files: Record<string, string>

  beforeEach(() => {
    files = { 'library/ui/figma/a.ts': 'b'.repeat(64) }
  })

  describe('and the lock keys are reordered', () => {
    let reordered: LockValue

    beforeEach(() => {
      reordered = JSON.parse(JSON.stringify(lockValue()))
      reordered.sources[0] = Object.fromEntries(
        Object.entries(reordered.sources[0]!).reverse()
      ) as LockValue['sources'][number]
    })

    it('should not depend on key order', () => {
      expect(portInputHash(reordered, files)).toBe(portInputHash(lockValue(), files))
    })
  })

  describe('and the lock has no capture digest table', () => {
    let value: { sources: { snapshot: Record<string, unknown> }[] }

    beforeEach(() => {
      value = lockValue() as { sources: { snapshot: Record<string, unknown> }[] }
      delete value.sources[0]!.snapshot.digests
    })

    it('should fail closed', () => {
      expect(() => portInputHash(value, files)).toThrow('no capture digest table')
    })
  })

  describe('and no bound file is listed', () => {
    it('should fail closed', () => {
      expect(() =>
        portInputHash(lockValue(), { 'library/ui/figma/MAINTENANCE.md': 'b'.repeat(64) })
      ).toThrow('no port input')
    })
  })
})

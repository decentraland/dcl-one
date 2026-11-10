import { execFileSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  REQUIRED_CHECKS,
  isRecordExempt,
  main,
  portInputHash,
  recordProblems,
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
// Every single-, double- or back-quoted literal without escapes or interpolation.
const pathLiterals = (text: string) =>
  [...text.matchAll(/(['"`])((?:(?!\1)[^\\\n$])*)\1/g)].map((match) => match[2]!)
const pointsAtExempt = (file: string, literal: string) => {
  const target = literal.startsWith('.')
    ? path.posix.normalize(path.posix.join(path.posix.dirname(file), literal))
    : undefined
  return Boolean(
    (target && (isRecordExempt(target) || isRecordExempt(`${target}/x.ts`))) ||
    /\.md$/.test(literal) ||
    /^(?:test|fixtures|__tests__)$/.test(literal) ||
    /(?:^|\/)(?:test|fixtures|__tests__)\//.test(literal)
  )
}

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
  it('should pass with no record for a change outside library/ui/figma', async () => {
    await put('library/ui/README.md', 'unrelated\n')
    commit('docs')

    expect(run(base)).toBe(0)
    expect(out).toContain('unchanged')
  })

  it('should pass with no record when only tests, fixtures and Markdown change', async () => {
    await put('library/ui/figma/test/a.test.ts', 'it("a", () => { /* edited */ })\n')
    await put('library/ui/figma/test/fixtures/data.json', '{}\n')
    await put('library/ui/figma/MAINTENANCE.md', 'v2\n')
    await put('library/ui/figma/agent-kit/skills/x/SKILL.md', 'skill\n')
    commit('tests and docs')

    expect(run(base)).toBe(0)
    expect(out).toContain('unchanged')
  })

  it.each([
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
  ])('should pass with no record when only %s changes', async (_, edit) => {
    await put(LOCK, JSON.stringify(lockValue({ edit })))
    commit('provenance')

    expect(run(base)).toBe(0)
    expect(out).toContain('unchanged')
  })
})

describe('when the head has no lock', () => {
  it('should pass for a head branched before the library existed', () => {
    const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
    const head = git('commit-tree', emptyTree, '-m', 'old branch')

    expect(main(['--base', base, '--head', head], repo)).toBe(0)
    expect(out).toContain('no upstream.lock.json at this head')
  })

  it('should pass for a head that keeps library/ui but has no lock', async () => {
    await rm(path.join(repo, LOCK))
    const head = commit('no lock')

    expect(main(['--base', base, '--head', head], repo)).toBe(0)
  })

  it('should fail on a head that is not a commit', () => {
    expect(main(['--base', base, '--head', 'f'.repeat(40)], repo)).toBe(1)
    expect(out).toContain('the head')
    expect(out).toContain('not a commit')
  })
})

describe('when the pull request changes a port input', () => {
  beforeEach(async () => {
    await put('library/ui/figma/a.ts', 'export const a = 2\n')
  })

  it('should fail when no record is committed', () => {
    commit('port without record')

    expect(run(base)).toBe(1)
    expect(out).toContain('not committed')
  })

  it('should pass with a record for exactly these port inputs', async () => {
    await put(RECORD, JSON.stringify(recordFor()))
    commit('port')

    expect(run(base)).toBe(0)
    expect(out).toContain('reports passing checks')
  })

  it('should keep a record valid when only exempt files change after it', async () => {
    await put(RECORD, JSON.stringify(recordFor()))
    await put('library/ui/figma/test/a.test.ts', 'it("b", () => {})\n')
    await put('library/ui/figma/MAINTENANCE.md', 'v3\n')
    commit('port, then a test and a doc edit')

    expect(run(base)).toBe(0)
  })

  it('should fail when a non-exempt file changed after the record was written', async () => {
    await put(RECORD, JSON.stringify(recordFor()))
    await put('library/ui/figma/a.ts', 'export const a = 3\n')
    commit('edited after verification')

    expect(run(base)).toBe(1)
    expect(out).toContain('the record is for other port inputs')
  })

  it('should fail when a new untracked-by-the-lock file appears after the record', async () => {
    await put(RECORD, JSON.stringify(recordFor()))
    await put('library/ui/figma/src/new.ts', 'export {}\n')
    commit('new module')

    expect(run(base)).toBe(1)
    expect(out).toContain('other port inputs')
  })

  it.each([
    ['the capture digest table', { digest: 'f' }],
    ['the public component table', { components: 'f' }]
  ])('should fail when %s changed after the record was written', async (_, options) => {
    await put(RECORD, JSON.stringify(recordFor()))
    await put(LOCK, JSON.stringify(lockValue(options)))
    commit('pin changed after verification')

    expect(run(base)).toBe(1)
    expect(out).toContain('other port inputs')
  })

  it.each([
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
  ])('should require a new record when only %s changes', async (_, edit) => {
    await put('library/ui/figma/a.ts', 'export const a = 1\n')
    await put(LOCK, JSON.stringify(lockValue({ edit })))
    commit('lock-only change')

    expect(run(base)).toBe(1)
    expect(out).toContain('not committed')
  })

  it('should fail with a schema 1 record', async () => {
    const rest: Record<string, unknown> = recordFor()
    delete rest.portInputHash
    await put(RECORD, JSON.stringify({ ...rest, schema: 1, lockHash: 'x', figmaTree: 'y' }))
    commit('old record')

    expect(run(base)).toBe(1)
    expect(out).toContain('unsupported record schema')
  })

  it('should fail with a malformed record', async () => {
    await put(RECORD, '{ not json')
    commit('broken record')

    expect(run(base)).toBe(1)
    expect(out).toContain('not committed valid JSON')
  })

  it('should fail when a required check did not pass', async () => {
    const checks = { ...passing(), diff: { result: 'fail', detail: '3 changed' } }
    await put(RECORD, JSON.stringify(recordFor({ checks })))
    commit('failing check')

    expect(run(base)).toBe(1)
    expect(out).toContain('check diff did not pass')
  })

  it('should treat a base without the library as a change', () => {
    const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
    const empty = git('commit-tree', emptyTree, '-m', 'empty')
    commit('port')

    expect(run(empty)).toBe(1)
  })

  it('should treat a base whose lock cannot be hashed as a change', async () => {
    await put(LOCK, JSON.stringify({ sources: [] }))
    const broken = commit('broken base')
    await put(LOCK, JSON.stringify(lockValue()))
    commit('port')

    expect(run(broken)).toBe(1)
    expect(out).toContain('not committed')
  })

  it('should fail on a head lock that is not valid JSON', async () => {
    await put(LOCK, '{ not json')
    commit('broken lock')

    expect(run(base)).toBe(1)
    expect(out).toContain('is not valid JSON')
  })

  it('should judge the given head rather than the checkout', async () => {
    await put(RECORD, JSON.stringify(recordFor()))
    const head = commit('port')
    await put('library/ui/figma/a.ts', 'export const a = 4\n')
    commit('later change without a record')

    expect(main(['--base', base, '--head', head], repo)).toBe(0)
    expect(main(['--base', base], repo)).toBe(1)
  })

  it('should fail on a base that is not a commit', () => {
    commit('port')

    expect(run('f'.repeat(40))).toBe(1)
    expect(out).toContain('not a commit')
  })

  it('should leave the record and the lock untouched', async () => {
    const record = JSON.stringify(recordFor({ checks: { ...passing(), diff: { result: 'fail' } } }))
    await put(RECORD, record)
    commit('failing record')
    const lock = await readFile(path.join(repo, LOCK), 'utf8')

    expect(run(base)).toBe(1)
    expect(await readFile(path.join(repo, RECORD), 'utf8')).toBe(record)
    expect(await readFile(path.join(repo, LOCK), 'utf8')).toBe(lock)
  })
})

describe('when reading the files of a commit', () => {
  it('should refuse a Git LFS pointer', async () => {
    await put(
      'library/ui/figma/font.woff2',
      'version https://git-lfs.github.com/spec/v1\noid sha256:x\n'
    )
    const head = commit('pointer')

    expect(() => trackedFilesAt(repo, head)).toThrow('Git LFS pointer')
  })

  it('should refuse a submodule', () => {
    git('update-index', '--add', '--cacheinfo', `160000,${'e'.repeat(40)},library/ui/figma/vendor`)
    git('commit', '--quiet', '--no-gpg-sign', '-m', 'submodule')

    expect(() => trackedFilesAt(repo, git('rev-parse', 'HEAD'))).toThrow('is not a file')
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
    'library/ui/figma/doc.md.ts',
    'library/ui/README.md',
    'library/ui/figma-other/test/a.ts',
    undefined
  ])('should bind %s', (file) => {
    expect(isRecordExempt(file)).toBe(false)
  })

  // The exemption holds only while no port input names an exempt file. This scans every string literal of the tracked
  // port inputs for a relative path, a bare directory name or a Markdown name that points at one. It does not see paths
  // built at run time; the agent kit's readdir of its skills is one, and no check against the capture runs the kit.
  it('should find no port input under library/ui/figma whose string literals point at an exempt file', () => {
    const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
    const files = execFileSync('git', ['-C', root, 'ls-files', '-z', '--', 'library/ui/figma'])
      .toString('utf8')
      .split('\0')
      .filter((file) => /\.(?:[cm]?[jt]sx?|mts|cts)$/.test(file) && !isRecordExempt(file))
    expect(files.length).toBeGreaterThan(100)
    const offenders: string[] = []
    for (const file of files) {
      const text = execFileSync('git', ['-C', root, 'show', `:${file}`]).toString('utf8')
      for (const literal of pathLiterals(text))
        if (pointsAtExempt(file, literal)) offenders.push(`${file}: ${JSON.stringify(literal)}`)
    }
    expect(offenders).toEqual([])
  })

  it.each([
    ["import './test/setup.ts'", 'a side-effect import'],
    ["export * from './fixtures'", 'a directory re-export'],
    ["new URL('../test/data.json', import.meta.url)", 'a URL'],
    ["path.join(root, 'fixtures', 'x.json')", 'a bare directory segment'],
    ['readFile(`notes/README.md`)', 'a Markdown name in a template literal'],
    ["await import('./src/a.test.ts')", 'a dynamic import of a test']
  ])('should flag %s as %s', (code) => {
    const file = 'library/ui/figma/tool.mjs'
    expect(pathLiterals(code).some((literal) => pointsAtExempt(file, literal))).toBe(true)
  })

  it('should not flag literals that only look like words', () => {
    const file = 'library/ui/figma/tool.mjs'
    const code = "const label = 'testing'; const re = /test/; run('./src/a.ts'); log('mdx')"
    expect(pathLiterals(code).filter((literal) => pointsAtExempt(file, literal))).toEqual([])
  })
})

describe('when validating a record', () => {
  const lock = lockValue()
  const inputHash = portInputHash(lock, { 'library/ui/figma/a.ts': 'b'.repeat(64) })
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
  const files = { 'library/ui/figma/a.ts': 'b'.repeat(64) }

  it('should not depend on key order', () => {
    const reordered = JSON.parse(JSON.stringify(lockValue()))
    reordered.sources[0] = Object.fromEntries(Object.entries(reordered.sources[0]).reverse())
    expect(portInputHash(reordered, files)).toBe(portInputHash(lockValue(), files))
  })

  it('should fail closed without a capture digest table or any bound file', () => {
    const value = lockValue() as { sources: { snapshot: Record<string, unknown> }[] }
    delete value.sources[0]!.snapshot.digests
    expect(() => portInputHash(value, files)).toThrow('no capture digest table')
    expect(() =>
      portInputHash(lockValue(), { 'library/ui/figma/MAINTENANCE.md': 'b'.repeat(64) })
    ).toThrow('no port input')
  })
})

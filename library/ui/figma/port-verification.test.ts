import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { REQUIRED_CHECKS, main, recordProblems } from './port-verification.mjs'

const LOCK = 'library/ui/upstream.lock.json'
const RECORD = 'library/ui/port-verification.json'
const REVISION = 'a'.repeat(40)
let repo: string
let out: string

const git = (...args: string[]) =>
  execFileSync(
    'git',
    ['-C', repo, '-c', 'user.name=t', '-c', 'user.email=t@example.com', ...args],
    {
      stdio: 'pipe'
    }
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
const SNAPSHOT_REVISION = 'c'.repeat(40)
const lockText = (revision = REVISION, note = 'v1') =>
  JSON.stringify({
    sources: [
      {
        repository: 'https://github.com/decentraland/figma-design-system',
        revision,
        note,
        snapshot: { revision: SNAPSHOT_REVISION }
      }
    ]
  })
const passing = () => Object.fromEntries(REQUIRED_CHECKS.map((name) => [name, { result: 'pass' }]))
// A record that vouches for whatever the working tree will commit next.
const recordFor = (lock: string, overrides: Record<string, unknown> = {}) => {
  git('add', '-A')
  const tree = git('write-tree')
  const figmaTree = git('rev-parse', `${tree}:library/ui/figma`)
  return {
    schema: 1,
    lockHash: createHash('sha256').update(lock).digest('hex'),
    fdsRevision: REVISION,
    snapshotRevision: SNAPSHOT_REVISION,
    figmaTree,
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
  await put(LOCK, lockText())
  base = commit('base')
  out = ''
  vi.spyOn(process.stdout, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
  vi.spyOn(process.stderr, 'write').mockImplementation((chunk) => ((out += String(chunk)), true))
})

afterEach(async () => {
  vi.restoreAllMocks()
  await rm(repo, { recursive: true, force: true })
})

describe('when the pull request does not change the lock', () => {
  it('should pass with no record and no pending status', async () => {
    await put('library/ui/README.md', 'unrelated\n')
    commit('docs')

    expect(run(base)).toBe(0)
    expect(out).toContain('unchanged')
  })
})

describe('when the head has no lock', () => {
  it('should pass for a head branched before the library existed', async () => {
    const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
    const head = git('commit-tree', emptyTree, '-m', 'old branch')

    expect(main(['--base', base, '--head', head], repo)).toBe(0)
    expect(out).toContain('no upstream.lock.json at this head')
  })

  it('should pass when neither the base nor the head has the library', async () => {
    const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
    const empty = git('commit-tree', emptyTree, '-m', 'empty')
    const head = git('commit-tree', emptyTree, '-p', empty, '-m', 'unrelated')

    expect(main(['--base', empty, '--head', head], repo)).toBe(0)
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

describe('when the pull request changes the lock', () => {
  beforeEach(async () => {
    await put('library/ui/figma/a.ts', 'export const a = 2\n')
    await put(LOCK, lockText(REVISION, 'v2'))
  })

  it('should fail when no record is committed', () => {
    commit('port without record')

    expect(run(base)).toBe(1)
    expect(out).toContain('not committed')
  })

  it('should pass with a record for this exact lock, commit and tree', async () => {
    await put(RECORD, JSON.stringify(recordFor(lockText(REVISION, 'v2'))))
    commit('port')

    expect(run(base)).toBe(0)
    expect(out).toContain('vouches')
  })

  it('should fail with a record written for a different lock hash', async () => {
    await put(RECORD, JSON.stringify(recordFor(lockText(REVISION, 'v1'))))
    commit('stale record')

    expect(run(base)).toBe(1)
    expect(out).toContain('another upstream.lock.json')
  })

  it('should fail when files under library/ui/figma changed after the record was written', async () => {
    await put(RECORD, JSON.stringify(recordFor(lockText(REVISION, 'v2'))))
    await put('library/ui/figma/a.ts', 'export const a = 3\n')
    commit('edited after verification')

    expect(run(base)).toBe(1)
    expect(out).toContain('another library/ui/figma tree')
  })

  it('should fail with a malformed record', async () => {
    await put(RECORD, '{ not json')
    commit('broken record')

    expect(run(base)).toBe(1)
    expect(out).toContain('not committed valid JSON')
  })

  it('should fail when a required check did not pass', async () => {
    const checks = { ...passing(), diff: { result: 'fail', detail: '3 changed' } }
    await put(RECORD, JSON.stringify(recordFor(lockText(REVISION, 'v2'), { checks })))
    commit('failing check')

    expect(run(base)).toBe(1)
    expect(out).toContain('check diff did not pass')
  })

  it('should treat a base without the library as a lock change', async () => {
    const emptyTree = execFileSync('git', ['-C', repo, 'mktree'], { input: '' }).toString().trim()
    const empty = git('commit-tree', emptyTree, '-m', 'empty')
    commit('port')

    expect(run(empty)).toBe(1)
  })

  it('should judge the given head rather than the checkout', async () => {
    await put(RECORD, JSON.stringify(recordFor(lockText(REVISION, 'v2'))))
    const head = commit('port')
    await put(LOCK, lockText(REVISION, 'v3'))
    commit('later change without a record')

    expect(main(['--base', base, '--head', head], repo)).toBe(0)
    expect(main(['--base', base], repo)).toBe(1)
  })

  it('should fail on a base that is not a commit', () => {
    commit('port')

    expect(run('f'.repeat(40))).toBe(1)
    expect(out).toContain('not a commit')
  })
})

describe('when validating a record', () => {
  const lockBytes = Buffer.from(lockText())
  const valid = () => ({
    schema: 1,
    lockHash: createHash('sha256').update(lockBytes).digest('hex'),
    fdsRevision: REVISION,
    snapshotRevision: SNAPSHOT_REVISION,
    figmaTree: 'tree',
    checks: passing()
  })
  const problems = (record: unknown) => recordProblems(record, { lockBytes, figmaTree: 'tree' })

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

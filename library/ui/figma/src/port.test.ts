import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { execFile } from 'node:child_process'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { portInputHash, recordProblems, trackedFilesAt } from '../port-verification.mjs'
import {
  PREPARE_ORDER,
  StageError,
  contentSnapshot,
  portPrepare,
  portVerify,
  prepareStages,
  type Stage
} from './port.js'

const REQUIRED = [
  'fetch-source',
  'deviations',
  'diff',
  'check-mappings',
  'review-manifest',
  'navbar-bounds',
  'install'
]
const hash = (text: string) => createHash('sha256').update(text).digest('hex')

// A synthetic checkout: one tracked port input and a lock that pins it; no capture and no Figma data.
describe('when a maintainer runs the port commands', () => {
  let root: string
  let figma: string
  const git = (...args: string[]) => execFileSync('git', ['-C', root, ...args], { stdio: 'pipe' })
  const lock = () =>
    JSON.parse(readFileSync(path.join(root, 'library/ui/upstream.lock.json'), 'utf8'))
  const passing = (overrides: Record<string, () => Stage> = {}) =>
    Object.fromEntries(
      REQUIRED.map((name) => [
        name,
        overrides[name] ?? (() => ({ name, detail: `synthetic ${name} 1` }))
      ])
    )

  beforeEach(() => {
    root = mkdtempSync(path.join(tmpdir(), 'dcl-port-'))
    figma = path.join(root, 'library/ui/figma')
    mkdirSync(figma, { recursive: true })
    const module = 'export const synthetic = true\n'
    writeFileSync(path.join(figma, 'synthetic.mjs'), module)
    writeFileSync(
      path.join(root, 'library/ui/upstream.lock.json'),
      JSON.stringify({
        sources: [
          {
            repository: 'https://github.com/decentraland/figma-design-system',
            revision: 'a'.repeat(40),
            snapshot: { revision: 'b'.repeat(40), digests: { 'pages.json': 'c'.repeat(64) } },
            files: { 'library/ui/figma/synthetic.mjs': { sha256: hash(module), fds: null } }
          }
        ]
      })
    )
    git('init', '--quiet')
    git('add', '.')
    git(
      '-c',
      'user.name=t',
      '-c',
      'user.email=t@example.com',
      '-c',
      'commit.gpgsign=false',
      'commit',
      '-qm',
      'synthetic'
    )
  })
  afterEach(() => {
    rmSync(root, { recursive: true, force: true })
  })

  describe('and every check passes on a clean tree', () => {
    it('should write a record the validator accepts for exactly these port inputs', () => {
      const result = portVerify(figma, { stages: passing() })

      expect(result).toMatchObject({ ok: true, record: 'library/ui/port-verification.json' })
      const record = JSON.parse(
        readFileSync(path.join(root, 'library/ui/port-verification.json'), 'utf8')
      )
      const inputHash = portInputHash(lock(), trackedFilesAt(root, 'HEAD'))
      expect(record.portInputHash).toBe(inputHash)
      expect(recordProblems(record, { lock: lock(), inputHash })).toEqual([])
      expect(Object.keys(record.checks)).toEqual(REQUIRED)
      expect(record.checks.diff).toEqual({ result: 'pass', detail: 'synthetic diff 1' })
    })
  })

  describe('and a check fails', () => {
    it('should stop, write nothing and leave an existing record byte-identical', () => {
      const file = path.join(root, 'library/ui/port-verification.json')
      writeFileSync(file, '{"previous":true}\n')
      git('add', '.')
      git(
        '-c',
        'user.name=t',
        '-c',
        'user.email=t@example.com',
        '-c',
        'commit.gpgsign=false',
        'commit',
        '-qm',
        'record'
      )
      const later = vi.fn(() => ({ name: 'review-manifest', detail: 'never' }))

      const result = portVerify(figma, {
        stages: passing({
          'check-mappings': () => {
            throw new StageError('check-mappings failed: drift')
          },
          'review-manifest': later
        })
      })

      expect(result).toMatchObject({
        ok: false,
        failed: 'check-mappings',
        reason: 'check-mappings failed: drift'
      })
      expect(later).not.toHaveBeenCalled()
      expect(readFileSync(file, 'utf8')).toBe('{"previous":true}\n')
    })
  })

  describe.each([
    ['an uncommitted change', () => writeFileSync(path.join(figma, 'synthetic.mjs'), 'changed\n')],
    ['an untracked file', () => writeFileSync(path.join(figma, 'extra.mjs'), 'new\n')]
  ])('and the tree has %s', (_label, dirty) => {
    it('should refuse before running any check', () => {
      dirty()
      const stage = vi.fn(() => ({ name: 'fetch-source', detail: 'x' }))

      const result = portVerify(figma, { stages: passing({ 'fetch-source': stage }) })

      expect(result).toMatchObject({ ok: false, failed: 'clean-tree' })
      expect(stage).not.toHaveBeenCalled()
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
    })
  })

  describe('and a check changes tracked files', () => {
    it('should refuse to write the record', () => {
      const result = portVerify(figma, {
        stages: passing({
          install: () => {
            writeFileSync(path.join(figma, 'synthetic.mjs'), 'rewritten\n')
            return { name: 'install', detail: 'ok' }
          }
        })
      })

      expect(result).toMatchObject({ ok: false, failed: 'clean-tree' })
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
    })
  })

  describe.each([
    ['a quote', 'said "pass"'],
    ['a newline', 'one\ntwo'],
    ['an over-long report', 'x'.repeat(121)]
  ])('and a check reports %s', (_label, detail) => {
    it('should refuse the detail and write no record', () => {
      const result = portVerify(figma, {
        stages: passing({ diff: () => ({ name: 'diff', detail }) })
      })

      expect(result).toMatchObject({ ok: false, failed: 'diff' })
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
    })
  })

  describe('and git cannot report the tree state', () => {
    it('should fail instead of treating the tree as clean', () => {
      const run = (command: string, args: string[], cwd: string) =>
        args.includes('status')
          ? { code: 128, stdout: '', stderr: 'fatal: broken' }
          : (() => {
              const out = execFileSync(command, args, { cwd, encoding: 'utf8' })
              return { code: 0, stdout: out, stderr: '' }
            })()

      expect(() => portVerify(figma, { run, stages: passing() })).toThrow('git status failed')
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
    })
  })

  describe('and prepare meets a failing stage', () => {
    it('should stop there, name it and run no later stage', () => {
      const ran: string[] = []
      const stages = Object.fromEntries(
        PREPARE_ORDER.map((name) => [
          name,
          () => {
            ran.push(name)
            if (name === 'sync-again') throw new StageError('a second sync changed files')
            return { name, detail: 'ok' }
          }
        ])
      )

      const result = portPrepare(figma, { stages })

      expect(result).toMatchObject({ ok: false, failed: 'sync-again' })
      expect(ran).toEqual(['fetch-source', 'diff', 'sync', 'sync-again'])
    })
  })

  describe('and prepare runs every stage', () => {
    it('should report the stages in order and point at verify', () => {
      const stages = Object.fromEntries(
        PREPARE_ORDER.map((name) => [name, () => ({ name, detail: 'ok' })])
      )

      const result = portPrepare(figma, { stages })

      expect(result.ok).toBe(true)
      expect(result.stages.map((stage) => stage.name)).toEqual(PREPARE_ORDER)
      expect(result.next).toMatch(/port verify/)
    })
  })
})

describe('when the CLI receives a port command it does not know', () => {
  it.each([[['port']], [['port', 'record']], [['port', 'verify', 'extra']]])(
    'should refuse %j',
    async (args) => {
      await expect(
        promisify(execFile)(
          process.execPath,
          ['--import', import.meta.resolve('tsx'), 'src/cli.ts', ...args],
          {
            cwd: path.resolve(import.meta.dirname, '..')
          }
        )
      ).rejects.toMatchObject({
        code: 1,
        stderr: expect.stringContaining('port takes prepare or verify')
      })
    },
    30_000
  )
})

describe('when the second sync of a refresh is compared with the first', () => {
  let repository: string
  const run = (command: string, args: string[], cwd: string) => {
    try {
      return { code: 0, stdout: execFileSync(command, args, { cwd, encoding: 'utf8' }), stderr: '' }
    } catch (error) {
      const failure = error as { status: number | null; stdout?: string; stderr?: string }
      return { code: failure.status, stdout: failure.stdout ?? '', stderr: failure.stderr ?? '' }
    }
  }
  const at = (file: string) => path.join(repository, 'library/ui/figma', file)
  const snapshot = () =>
    contentSnapshot(run, {
      figma: path.join(repository, 'library/ui/figma'),
      ui: path.join(repository, 'library/ui'),
      repository
    })

  beforeEach(() => {
    repository = mkdtempSync(path.join(tmpdir(), 'port-snapshot-'))
    mkdirSync(path.dirname(at('review/X.tsx')), { recursive: true })
    writeFileSync(at('review/X.tsx'), 'v1\n')
    execFileSync('git', ['init', '--quiet'], { cwd: repository })
    execFileSync('git', ['add', '-A'], { cwd: repository })
    execFileSync(
      'git',
      [
        '-c',
        'user.name=t',
        '-c',
        'user.email=t@example.com',
        'commit',
        '--quiet',
        '--no-gpg-sign',
        '-m',
        'base'
      ],
      { cwd: repository }
    )
  })

  afterEach(() => rmSync(repository, { recursive: true, force: true }))

  it('should see an already modified file rewritten with different bytes', () => {
    writeFileSync(at('review/X.tsx'), 'v2\n')
    const first = snapshot()
    writeFileSync(at('review/X.tsx'), 'v3\n')

    expect(snapshot()).not.toBe(first)
  })

  it('should stay equal when the second sync writes the same bytes', () => {
    writeFileSync(at('review/X.tsx'), 'v2\n')
    writeFileSync(at('review/New.tsx'), 'new\n')
    const first = snapshot()
    writeFileSync(at('review/X.tsx'), 'v2\n')
    writeFileSync(at('review/New.tsx'), 'new\n')

    expect(snapshot()).toBe(first)
  })

  it('should see a file the second sync deletes or adds', () => {
    writeFileSync(at('review/X.tsx'), 'v2\n')
    const first = snapshot()
    rmSync(at('review/X.tsx'))
    expect(snapshot()).not.toBe(first)

    writeFileSync(at('review/X.tsx'), 'v2\n')
    writeFileSync(at('review/Extra.tsx'), 'x\n')
    expect(snapshot()).not.toBe(first)
  })

  it('should fail closed when git status fails', () => {
    rmSync(path.join(repository, '.git'), { recursive: true, force: true })
    expect(() => snapshot()).toThrow('git status failed')
  })
})

describe('when prepare reseals the lock digests', () => {
  const p = { figma: '/repo/library/ui/figma', ui: '/repo/library/ui', repository: '/repo' }
  const recorder = (addCode: number) => {
    const calls: string[] = []
    const run = (command: string, args: string[]) => {
      calls.push([command === process.execPath ? 'node' : command, ...args].join(' '))
      const add = command === 'git' && args.includes('add')
      return {
        code: add ? addCode : 0,
        stdout: '',
        stderr: add && addCode ? 'fatal: index.lock exists' : ''
      }
    }
    return { calls, run }
  }

  it('should stage library/ui before lock-digests reads the index', () => {
    const { calls, run } = recorder(0)

    expect(prepareStages(run, p)['lock-digests']!()).toEqual({
      name: 'lock-digests',
      detail: 'staged library/ui and rewrote the lock digests'
    })
    expect(calls).toEqual([
      'git -C /repo add -A -- library/ui',
      'node /repo/library/ui/figma/lock-digests.mjs --write'
    ])
  })

  it('should stop before resealing when staging fails', () => {
    const { calls, run } = recorder(128)

    expect(() => prepareStages(run, p)['lock-digests']!()).toThrow(
      'git add failed: fatal: index.lock exists'
    )
    expect(calls).toEqual(['git -C /repo add -A -- library/ui'])
  })
})

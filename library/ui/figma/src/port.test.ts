import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync
} from 'node:fs'
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

// Git in a temporary repository, never the caller's: a hook or a shell may export GIT_DIR or GIT_INDEX_FILE, which
// would send `git -C <tmp>` to the outer repository's index.
const GIT_ENV = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) =>
      ![
        'GIT_DIR',
        'GIT_INDEX_FILE',
        'GIT_WORK_TREE',
        'GIT_OBJECT_DIRECTORY',
        'GIT_COMMON_DIR'
      ].includes(key)
  )
) as NodeJS.ProcessEnv
const realRun = (command: string, args: string[], cwd: string, env = GIT_ENV) => {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', env })
  if (result.error) throw result.error
  return { code: result.status, stdout: result.stdout, stderr: result.stderr }
}
const COMMIT = ['-c', 'user.name=t', '-c', 'user.email=t@example.com', '-c', 'commit.gpgsign=false']

// A synthetic checkout: one tracked port input and a lock that pins it; no capture and no Figma data.
describe('when a maintainer runs the port commands', () => {
  let root: string
  let figma: string
  const git = (...args: string[]) =>
    execFileSync('git', ['-C', root, ...args], { stdio: 'pipe', env: GIT_ENV })
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
      const result = portVerify(figma, { run: realRun, stages: passing() })

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
        run: realRun,
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

      const result = portVerify(figma, { run: realRun, stages: passing({ 'fetch-source': stage }) })

      expect(result).toMatchObject({ ok: false, failed: 'clean-tree' })
      expect(stage).not.toHaveBeenCalled()
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
    })
  })

  describe('and a check changes tracked files', () => {
    it('should refuse to write the record', () => {
      const result = portVerify(figma, {
        run: realRun,
        stages: passing({
          install: () => {
            writeFileSync(path.join(figma, 'synthetic.mjs'), 'rewritten\n')
            return { name: 'install', detail: 'ok' }
          }
        })
      })

      expect(result).toMatchObject({ ok: false, failed: 'clean-tree', restored: true })
      expect(existsSync(path.join(root, 'library/ui/port-verification.json'))).toBe(false)
      expect(readFileSync(path.join(figma, 'synthetic.mjs'), 'utf8')).toBe(
        'export const synthetic = true\n'
      )
    })
  })

  describe.each([
    ['a quote', 'said "pass"'],
    ['a newline', 'one\ntwo'],
    ['an over-long report', 'x'.repeat(121)]
  ])('and a check reports %s', (_label, detail) => {
    it('should refuse the detail and write no record', () => {
      const result = portVerify(figma, {
        run: realRun,
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
          : realRun(command, args, cwd)

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

      const result = portPrepare(figma, { run: realRun, stages })

      expect(result).toMatchObject({ ok: false, failed: 'sync-again' })
      expect(ran).toEqual(['fetch-source', 'diff', 'sync', 'sync-again'])
    })
  })

  describe('and prepare runs every stage', () => {
    it('should report the stages in order and point at verify', () => {
      const stages = Object.fromEntries(
        PREPARE_ORDER.map((name) => [name, () => ({ name, detail: 'ok' })])
      )

      const result = portPrepare(figma, { run: realRun, stages })

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
  // GIT_CEILING_DIRECTORIES stops git at the temporary directory, so with .git removed it cannot find a repository
  // that happens to contain TMPDIR.
  const run = (command: string, args: string[], cwd: string) =>
    realRun(command, args, cwd, { ...GIT_ENV, GIT_CEILING_DIRECTORIES: path.dirname(repository) })
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
    execFileSync('git', ['init', '--quiet'], { cwd: repository, env: GIT_ENV })
    execFileSync('git', ['add', '-A'], { cwd: repository, env: GIT_ENV })
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
      { cwd: repository, env: GIT_ENV }
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

// The real verify and prepare stages, against a synthetic checkout: git runs for real, and every command a stage
// spawns (this CLI, lock-digests, vitest) answers with canned output.
describe('when the real port stages run against canned command output', () => {
  let root: string
  let ui: string
  let figma: string
  type Reply = { code: number; stdout?: string; stderr?: string; effect?: () => void }
  let replies: Record<string, Reply>
  const spawned: string[][] = []
  const git = (...args: string[]) =>
    execFileSync('git', ['-C', root, ...args], { stdio: 'pipe', env: GIT_ENV })
  const commit = (message: string) => {
    git('add', '-A')
    git(...COMMIT, 'commit', '-qm', message)
  }
  const ok = (value: unknown): Reply => ({ code: 0, stdout: JSON.stringify(value) })
  // The key of a spawned command: the CLI subcommand, or the script it runs.
  const keyOf = (args: string[]) => {
    const script = args.find((arg) => /lock-digests\.mjs$|vitest\.mjs$/.test(arg))
    if (script) return path.basename(script) + (args.includes('--write') ? ' --write' : '')
    const rest = args.slice(process.execArgv.length + 1)
    return rest.filter((arg) => !arg.startsWith('../')).join(' ')
  }
  const run = (command: string, args: string[], cwd: string) => {
    if (command !== process.execPath) return realRun(command, args, cwd)
    spawned.push(args)
    const reply = replies[keyOf(args)]
    if (!reply) throw new Error(`spawn ${keyOf(args)} ENOENT`)
    reply.effect?.()
    return { code: reply.code, stdout: reply.stdout ?? '', stderr: reply.stderr ?? '' }
  }
  const copyTree = (from: string, to: string) => {
    mkdirSync(to, { recursive: true })
    for (const file of ['a.css'])
      writeFileSync(path.join(to, file), readFileSync(path.join(from, file)))
  }

  beforeEach(() => {
    root = mkdtempSync(path.join(tmpdir(), 'dcl-port-stages-'))
    ui = path.join(root, 'library/ui')
    figma = path.join(ui, 'figma')
    mkdirSync(path.join(figma, 'review'), { recursive: true })
    for (const target of ['foundations', 'components']) {
      mkdirSync(path.join(ui, target), { recursive: true })
      writeFileSync(path.join(ui, target, 'a.css'), `/* ${target} */\n`)
    }
    writeFileSync(path.join(figma, 'review/catalog.json'), '{}\n')
    const module = 'export const synthetic = true\n'
    writeFileSync(path.join(figma, 'synthetic.mjs'), module)
    writeFileSync(
      path.join(ui, 'upstream.lock.json'),
      JSON.stringify({
        sources: [
          {
            repository: 'https://github.com/decentraland/figma-design-system',
            revision: 'a'.repeat(40),
            snapshot: { revision: 'b'.repeat(40), digests: { 'pages.json': 'c'.repeat(64) } },
            files: { 'library/ui/figma/synthetic.mjs': { sha256: hash(module), fds: null } }
          }
        ],
        publicFoundations: { files: { 'a.css': {} } },
        publicComponents: { files: { 'a.css': {} } }
      })
    )
    git('init', '--quiet')
    commit('synthetic')
    spawned.length = 0
    const proof = (target: string) => path.join(ui, '.agent-work/figma-install/proof', target)
    replies = {
      'fetch-source': ok({ files: 12 }),
      'lock-digests.mjs': { code: 0, stdout: 'lock-digests: 3 files match upstream.lock.json\n' },
      diff: ok({ diff: { added: [], changed: [], removed: [] } }),
      'check-mappings': ok({ ok: true, checked: 1, captureChecked: true }),
      'review-manifest': ok({ components: 46 }),
      'vitest.mjs': { code: 0, stdout: ' Tests  19 passed (19)\n' },
      'install foundations': ok({}),
      'install components': ok({}),
      'install foundations --out': {
        ...ok({}),
        effect: () => copyTree(path.join(ui, 'foundations'), proof('foundations'))
      },
      'install components --out': {
        ...ok({}),
        effect: () => copyTree(path.join(ui, 'components'), proof('components'))
      }
    }
  })
  afterEach(() => rmSync(root, { recursive: true, force: true }))

  describe('and verify reads every report', () => {
    it('should record the counts each command reported and keep other private install stages', () => {
      const kept = path.join(ui, '.agent-work/figma-install/rejected/keep.txt')
      mkdirSync(path.dirname(kept), { recursive: true })
      writeFileSync(kept, 'mine')

      const result = portVerify(figma, { run })

      expect(result).toMatchObject({ ok: true })
      const record = JSON.parse(readFileSync(path.join(ui, 'port-verification.json'), 'utf8'))
      expect(record.checks['fetch-source'].detail).toBe(
        'Verified 12 cache files against the pinned revision and digest table'
      )
      expect(record.checks.diff.detail).toBe(
        'Fresh diff reports 0 added, 0 changed, 0 removed and no ledger issues'
      )
      expect(record.checks['navbar-bounds'].detail).toBe(
        'Fresh gallery build passes 19 bounds checks'
      )
      expect(record.checks.install.detail).toBe(
        'Private and canonical installs reproduce 1 foundation and 1 component files with no-op repeats'
      )
      expect(readFileSync(kept, 'utf8')).toBe('mine')
      expect(existsSync(path.join(ui, 'port-verification.json.tmp'))).toBe(false)
    })

    it('should run vitest with this Node, not through the POSIX shell shim', () => {
      portVerify(figma, { run })

      const vitest = spawned.find((args) => args[0]!.endsWith('vitest.mjs'))!
      expect(vitest[0]).toBe(path.join(realpathSync(ui), 'node_modules/vitest/vitest.mjs'))
    })
  })

  describe.each([
    [
      'a count that is not a number',
      'fetch-source',
      ok({ files: '12' }),
      'fetch-source reported no count'
    ],
    ['a diff report without its lists', 'diff', ok({}), 'diff added reported no count'],
    [
      'a negative count',
      'review-manifest',
      ok({ components: -1 }),
      'review-manifest reported no count'
    ]
  ])('and a command reports %s', (_label, key, reply, reason) => {
    it('should fail that check instead of recording NaN or undefined', () => {
      replies[key] = reply

      expect(portVerify(figma, { run })).toMatchObject({ ok: false, failed: key, reason })
      expect(existsSync(path.join(ui, 'port-verification.json'))).toBe(false)
    })
  })

  describe('and a reporting command exits 1 with its JSON report on stdout', () => {
    it('should name the first error of the report', () => {
      replies['check-mappings'] = {
        code: 1,
        stdout: JSON.stringify({
          ok: false,
          results: [
            {
              file: 'x',
              diagnostics: [
                { code: 'figma-drift', severity: 'error', message: 'Kind lost Tertiary' }
              ]
            }
          ]
        })
      }

      expect(portVerify(figma, { run })).toMatchObject({
        failed: 'check-mappings',
        reason: 'check-mappings failed: figma-drift: Kind lost Tertiary'
      })
    })
  })

  describe('and lock-digests writes its error to stderr', () => {
    it('should carry that error into the reason', () => {
      replies['lock-digests.mjs'] = { code: 1, stdout: '', stderr: 'CHANGED library/ui/x.ts\n' }

      expect(portVerify(figma, { run })).toMatchObject({
        failed: 'deviations',
        reason: 'lock-digests failed: CHANGED library/ui/x.ts'
      })
    })
  })

  describe('and a stage fails with an error that is not a StageError', () => {
    it('should name the stage and the error instead of crashing', () => {
      delete replies['vitest.mjs']

      expect(portVerify(figma, { run })).toMatchObject({
        ok: false,
        failed: 'navbar-bounds',
        reason: 'spawn vitest.mjs ENOENT'
      })
    })
  })

  describe('and a check fails after regenerating tracked files', () => {
    it('should put the tree back, so the next verify does not refuse it', () => {
      replies['review-manifest'] = {
        code: 1,
        stderr: 'boom\n',
        effect: () => {
          writeFileSync(path.join(figma, 'review/catalog.json'), '{"changed":true}\n')
          writeFileSync(path.join(figma, 'review/new.json'), '{}\n')
        }
      }

      expect(portVerify(figma, { run })).toMatchObject({
        failed: 'review-manifest',
        reason: 'review-manifest failed: boom',
        restored: true
      })
      expect(readFileSync(path.join(figma, 'review/catalog.json'), 'utf8')).toBe('{}\n')
      expect(existsSync(path.join(figma, 'review/new.json'))).toBe(false)
    })
  })

  describe('and the private install differs from the canonical one', () => {
    it('should fail the install check', () => {
      replies['install components --out'] = {
        ...ok({}),
        effect: () => {
          const proof = path.join(ui, '.agent-work/figma-install/proof/components')
          mkdirSync(proof, { recursive: true })
          writeFileSync(path.join(proof, 'a.css'), '/* other bytes */\n')
        }
      }

      expect(portVerify(figma, { run })).toMatchObject({
        failed: 'install',
        reason: 'the private components install differs from the canonical one'
      })
    })
  })

  describe('and HEAD moves while the checks run', () => {
    it('should refuse to write a record for the commit it did not check', () => {
      replies['vitest.mjs'] = {
        code: 0,
        stdout: ' Tests  19 passed (19)\n',
        effect: () => {
          writeFileSync(path.join(root, 'outside.txt'), 'x')
          commit('moved')
        }
      }

      expect(portVerify(figma, { run })).toMatchObject({ ok: false, failed: 'head' })
      expect(existsSync(path.join(ui, 'port-verification.json'))).toBe(false)
    })
  })

  describe('and a port input names an exempt file', () => {
    it('should refuse the record, as CI would', () => {
      writeFileSync(path.join(figma, 'NOTES.md'), '# notes\n')
      writeFileSync(path.join(figma, 'bad.mjs'), "export const notes = './NOTES.md'\n")
      commit('exempt reference')

      expect(portVerify(figma, { run })).toMatchObject({ ok: false, failed: 'scan' })
      expect(existsSync(path.join(ui, 'port-verification.json'))).toBe(false)
    })
  })

  describe('and prepare starts', () => {
    const prepareReplies = () => {
      replies.diff = {
        code: 1,
        stdout: JSON.stringify({ diff: { added: [1], changed: [], removed: [] } })
      }
      replies.sync = ok({})
      replies['lock-digests.mjs --write'] = { code: 0, stdout: 'lock-digests: pinned 3 files\n' }
    }

    it('should refuse a tree with changes other than the lock, before any stage', () => {
      prepareReplies()
      writeFileSync(path.join(figma, 'tmp-probe.ts'), 'probe\n')

      expect(portPrepare(figma, { run })).toMatchObject({
        ok: false,
        failed: 'clean-tree',
        stages: []
      })
      expect(spawned).toEqual([])
    })

    // The nearest false positive: the pin bump that starts a refresh is the one change prepare accepts.
    it('should accept a tree whose only change is the bumped lock', () => {
      prepareReplies()
      writeFileSync(
        path.join(ui, 'upstream.lock.json'),
        readFileSync(path.join(ui, 'upstream.lock.json'), 'utf8') + '\n'
      )

      const result = portPrepare(figma, { run })

      expect(result).toMatchObject({ ok: true })
      expect(result.stages.find((stage) => stage.name === 'diff')?.detail).toBe(
        '1 added, 0 changed, 0 removed'
      )
    })

    it('should report a diff that crashed, with its stderr', () => {
      prepareReplies()
      replies.diff = { code: 2, stdout: '', stderr: 'Error: the capture cache is missing\n' }

      expect(portPrepare(figma, { run })).toMatchObject({
        failed: 'diff',
        reason: 'diff failed: Error: the capture cache is missing'
      })
    })

    it('should stop when the second sync rewrites an already modified file', () => {
      prepareReplies()
      let syncs = 0
      replies.sync = {
        ...ok({}),
        effect: () => {
          syncs += 1
          writeFileSync(path.join(figma, 'review/catalog.json'), `{"sync":${syncs}}\n`)
        }
      }

      expect(portPrepare(figma, { run })).toMatchObject({
        failed: 'sync-again',
        reason: 'a second sync changed files: generation is not stable'
      })
    })

    it('should carry the lock-digests stderr into the reason', () => {
      prepareReplies()
      replies['lock-digests.mjs --write'] = { code: 1, stderr: 'deviation codes are not sorted\n' }

      expect(portPrepare(figma, { run })).toMatchObject({
        failed: 'lock-digests',
        reason: 'lock-digests --write failed: deviation codes are not sorted'
      })
    })
  })
})

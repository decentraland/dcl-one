// `dcl-figma port prepare` and `port verify`: the maintainer path for porting a figma-design-system capture into
// library/ui, as two ordered commands instead of a hand-run sequence. prepare mutates the tree and stops at the first
// failed stage; verify changes nothing until every check has passed, then writes library/ui/port-verification.json
// from what each check printed. The record states what this local run reported. CI checks that it names the head's
// port inputs and that every result is `pass`; it is not proof that the checks ran, an independent review, an
// attestation or evidence of live Figma parity.
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync
} from 'node:fs'
import path from 'node:path'
import { toPosix } from './guard/fs-safe.js'
import {
  RECORD_DETAIL,
  REQUIRED_CHECKS,
  portInputHash,
  recordProblems,
  scanProblems,
  trackedBlobsAt,
  trackedFilesAt
} from '../port-verification.mjs'

type Run = (
  command: string,
  args: string[],
  cwd: string
) => { code: number | null; stdout: string; stderr: string }

type Stage = { name: string; detail: string }
type StageResult = { name: string; ok: boolean; detail: string }

const PROOF = '.agent-work/figma-install/proof'

const spawnRun: Run = (command, args, cwd) => {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', maxBuffer: 1 << 28 })
  if (result.error) throw result.error
  return { code: result.status, stdout: result.stdout, stderr: result.stderr }
}

// The paths a port reads and writes, from the figma/ working directory the CLI runs in.
type Paths = { figma: string; ui: string; repository: string }

function paths(figmaDirectory: string, run: Run): Paths {
  // Real paths, so they compare with what Git reports through a symlinked temporary or home directory.
  const figma = realpathSync(figmaDirectory)
  const ui = path.resolve(figma, '..')
  const top = run('git', ['-C', ui, 'rev-parse', '--show-toplevel'], ui)
  if (top.code !== 0) throw new Error('port needs a Git checkout')
  return { figma, ui, repository: top.stdout.trim() }
}

class StageError extends Error {}

const fail = (message: string): never => {
  throw new StageError(message)
}

const json = (text: string, what: string): Record<string, unknown> => {
  try {
    return JSON.parse(text) as Record<string, unknown>
  } catch {
    return fail(`${what} printed no JSON report`)
  }
}

// This CLI again, in a child process with the same loader, so each stage runs exactly as a maintainer would run it.
const cli = (run: Run, figma: string, args: string[]) =>
  run(process.execPath, [...process.execArgv, process.argv[1]!, ...args], figma)

// A repository-relative path in the form Git prints and accepts: '/' separators on every platform.
const gitPath = (p: Paths, absolute: string) => toPosix(path.relative(p.repository, absolute))

// library/ui without its installed packages and private working files: what a port may change.
const uiPathspec = (p: Paths) => [
  gitPath(p, p.ui),
  `:!${gitPath(p, path.join(p.ui, 'node_modules'))}`,
  `:!${gitPath(p, path.join(p.figma, 'node_modules'))}`,
  `:!${gitPath(p, path.join(p.ui, '.agent-work'))}`
]

// The directories the checks regenerate (review-manifest writes the Review, install writes foundations and
// components). After a failed check only these are restored; anything else stays as the maintainer left it.
const checkOutputs = (p: Paths) => [
  gitPath(p, path.join(p.figma, 'review')) + '/',
  gitPath(p, path.join(p.ui, 'foundations')) + '/',
  gitPath(p, path.join(p.ui, 'components')) + '/'
]

// Uncommitted or untracked files under library/ui. A failing git status is an error, never an empty answer.
function changes(run: Run, p: Paths): string {
  const result = run(
    'git',
    ['-C', p.repository, 'status', '--porcelain', '--untracked-files=all', '--', ...uiPathspec(p)],
    p.ui
  )
  if (result.code !== 0) throw new Error(`git status failed: ${firstLine(result.stderr)}`)
  return result.stdout
}

/**
 * The uncommitted state of library/ui by content: each changed or untracked path with the sha256 of its bytes (or
 * "deleted"). Unlike the `git status` text, it changes when a file that is already modified is rewritten with different
 * bytes, which is what a second, unstable sync does on a refresh.
 */
function contentSnapshot(run: Run, p: Paths): string {
  const result = run(
    'git',
    [
      '-C',
      p.repository,
      'status',
      '--porcelain=v1',
      '-z',
      '--untracked-files=all',
      '--no-renames',
      '--',
      ...uiPathspec(p)
    ],
    p.ui
  )
  if (result.code !== 0) throw new Error(`git status failed: ${firstLine(result.stderr)}`)
  return result.stdout
    .split('\0')
    .filter(Boolean)
    .map((entry) => entry.slice(3))
    .sort()
    .map((file) => {
      const absolute = path.join(p.repository, file)
      const digest = existsSync(absolute)
        ? createHash('sha256').update(readFileSync(absolute)).digest('hex')
        : 'deleted'
      return `${file}\t${digest}`
    })
    .join('\n')
}

// Every file below a directory, by its /-separated relative path, with its bytes.
function treeFiles(directory: string): Map<string, Buffer> {
  const files = new Map<string, Buffer>()
  if (!existsSync(directory)) return files
  for (const entry of readdirSync(directory, { recursive: true, withFileTypes: true }))
    if (entry.isFile()) {
      const full = path.join(entry.parentPath, entry.name)
      files.set(path.relative(directory, full).split(path.sep).join('/'), readFileSync(full))
    }
  return files
}

// The same file names with the same bytes; compared in Node, so no system diff binary is needed.
function sameTree(a: Map<string, Buffer>, b: Map<string, Buffer>): boolean {
  return a.size === b.size && [...a].every(([file, bytes]) => b.get(file)?.equals(bytes) === true)
}

// A count a command reported: a non-negative integer, or the stage fails instead of recording NaN or undefined.
const countOf = (value: unknown, what: string): number =>
  Number.isInteger(value) && (value as number) >= 0
    ? (value as number)
    : fail(`${what} reported no count`)

const firstLine = (text: string) => text.trim().split('\n')[0]?.slice(0, 200) ?? ''

// Why a command failed. The CLI's reporting commands exit 1 with a JSON report on stdout and nothing on stderr, so
// that report is summarized (its first error); otherwise the first line of stderr, then of stdout.
function failureReason(result: { stdout: string; stderr: string }): string {
  let report: Record<string, unknown> | undefined
  try {
    report = JSON.parse(result.stdout) as Record<string, unknown>
  } catch {
    report = undefined
  }
  if (report && typeof report === 'object') {
    type Coded = { code?: unknown; message?: unknown }
    const errors = [
      ...((report.errors as Coded[] | undefined) ?? []),
      ...(
        (report.results as { diagnostics?: (Coded & { severity?: unknown })[] }[] | undefined) ?? []
      )
        .flatMap((entry) => entry.diagnostics ?? [])
        .filter((item) => item.severity === 'error')
    ]
    const first = errors[0]
    if (first) return firstLine(`${String(first.code)}: ${String(first.message)}`)
    const diff = report.diff as Record<string, unknown[]> | undefined
    if (diff)
      return `${diff.added?.length ?? 0} added, ${diff.changed?.length ?? 0} changed, ${diff.removed?.length ?? 0} removed, ${(report.ledgerIssues as unknown[] | undefined)?.length ?? 0} ledger issues`
  }
  return firstLine(result.stderr) || firstLine(result.stdout) || 'no output'
}

function cliStage(run: Run, p: Paths, args: string[], what: string): Record<string, unknown> {
  const result = cli(run, p.figma, args)
  if (result.code !== 0) fail(`${what} failed: ${failureReason(result)}`)
  return json(result.stdout, what)
}

// The library's own vitest, run with this Node instead of the node_modules/.bin shim, which needs a POSIX shell.
const vitest = (p: Paths) => path.join(p.ui, 'node_modules', 'vitest', 'vitest.mjs')

/** The seven checks the record reports, in the order REQUIRED_CHECKS names them. */
function verifyStages(run: Run, p: Paths): Record<string, () => Stage> {
  const lock = () =>
    JSON.parse(readFileSync(path.join(p.ui, 'upstream.lock.json'), 'utf8')) as {
      publicFoundations?: { files: Record<string, unknown> }
      publicComponents?: { files: Record<string, unknown> }
    }
  return {
    'fetch-source': () => {
      const report = cliStage(run, p, ['fetch-source'], 'fetch-source')
      return {
        name: 'fetch-source',
        detail: `Verified ${countOf(report.files, 'fetch-source')} cache files against the pinned revision and digest table`
      }
    },
    deviations: () => {
      const result = run(process.execPath, [path.join(p.figma, 'lock-digests.mjs')], p.ui)
      const pins = /lock-digests: (\d+) files match/.exec(result.stdout)?.[1]
      if (result.code !== 0 || !pins) fail(`lock-digests failed: ${failureReason(result)}`)
      const { publicFoundations, publicComponents } = lock()
      return {
        name: 'deviations',
        detail: `${pins} tooling pins, ${Object.keys(publicFoundations?.files ?? {}).length} foundation origins and ${Object.keys(publicComponents?.files ?? {}).length} component outputs pass the common guard`
      }
    },
    diff: () => {
      const report = cliStage(run, p, ['diff'], 'diff')
      const diff = (report.diff ?? {}) as Record<string, unknown>
      const length = (key: string) =>
        countOf(
          Array.isArray(diff[key]) ? (diff[key] as unknown[]).length : undefined,
          `diff ${key}`
        )
      return {
        name: 'diff',
        detail: `Fresh diff reports ${length('added')} added, ${length('changed')} changed, ${length('removed')} removed and no ledger issues`
      }
    },
    'check-mappings': () => {
      const report = cliStage(run, p, ['check-mappings'], 'check-mappings')
      if (report.captureChecked !== true) fail('check-mappings did not read the capture')
      const checked = countOf(report.checked, 'check-mappings')
      return {
        name: 'check-mappings',
        detail: `Fresh check passes with ${checked} public ${checked === 1 ? 'mapping' : 'mappings'} and no errors`
      }
    },
    'review-manifest': () => {
      const report = cliStage(run, p, ['review-manifest'], 'review-manifest')
      const stale = run(
        'git',
        ['-C', p.figma, 'diff', '--exit-code', '--stat', '--', 'review'],
        p.figma
      )
      if (stale.code !== 0) fail('the regenerated Review catalog differs from the committed one')
      return {
        name: 'review-manifest',
        detail: `Fresh catalog regeneration is byte-identical, ${countOf(report.components, 'review-manifest')} components`
      }
    },
    'navbar-bounds': () => {
      const result = run(
        process.execPath,
        [vitest(p), 'run', '--project', 'consumer', 'navbar-bounds'],
        p.ui
      )
      const passed = /Tests\s+(\d+) passed/.exec(result.stdout)?.[1]
      if (result.code !== 0 || !passed) fail('the NavBar bounds build check failed')
      return { name: 'navbar-bounds', detail: `Fresh gallery build passes ${passed} bounds checks` }
    },
    install: () => {
      for (const target of ['foundations', 'components', 'foundations', 'components'])
        cliStage(run, p, ['install', target], `install ${target}`)
      if (changes(run, p)) fail('a repeated canonical install changed tracked files')
      const proof = path.join(p.ui, PROOF)
      // Only the proof is reset; anything else a maintainer staged under .agent-work/figma-install stays.
      rmSync(proof, { recursive: true, force: true })
      const counts: number[] = []
      for (const target of ['foundations', 'components']) {
        cliStage(
          run,
          p,
          ['install', target, '--out', `../${PROOF}/${target}`],
          `private install ${target}`
        )
        const privateFiles = treeFiles(path.join(proof, target))
        if (!privateFiles.size || !sameTree(privateFiles, treeFiles(path.join(p.ui, target))))
          fail(`the private ${target} install differs from the canonical one`)
        counts.push(privateFiles.size)
      }
      return {
        name: 'install',
        detail: `Private and canonical installs reproduce ${counts[0]} foundation and ${counts[1]} component files with no-op repeats`
      }
    }
  }
}

type Restore = {
  /** Paths put back to HEAD, in Git form. */
  restored: string[]
  /** Where the replaced bytes were copied first, relative to library/ui; absent when nothing was restored. */
  backup?: string
  /** Changed paths outside the check outputs, left untouched. */
  kept: string[]
  /** Why the restore stopped, when it failed; the failed check is still reported. */
  error?: string
}

type VerifyResult =
  | { ok: true; record: string; portInputHash: string; checks: StageResult[] }
  | { ok: false; failed: string; reason: string; checks: StageResult[]; restore?: Restore }

// Runs one stage. Any error, not only a StageError, fails that stage by name, so the report says which check broke.
function runStage(name: string, stage: (() => Stage) | undefined): StageResult {
  try {
    if (!stage) fail(`no runner for ${name}`)
    const { detail } = stage!()
    return { name, ok: true, detail }
  } catch (error) {
    return { name, ok: false, detail: error instanceof Error ? error.message : String(error) }
  }
}

/**
 * After a failed check, puts back what the checks regenerated so the next verify does not refuse a dirty tree. Verify
 * starts from a clean tree, but it runs for minutes, so only paths under the check outputs are touched; each one is
 * copied to a backup directory under .agent-work first, so an edit made there during the run can be recovered. Other
 * changes are reported as kept. A restore that fails reports its error instead of hiding the failed check.
 */
function restoreTree(run: Run, p: Paths): Restore {
  const outcome: Restore = { restored: [], kept: [] }
  try {
    const status = run(
      'git',
      [
        '-C',
        p.repository,
        'status',
        '--porcelain=v1',
        '-z',
        '--untracked-files=all',
        '--no-renames',
        '--',
        ...uiPathspec(p)
      ],
      p.ui
    )
    if (status.code !== 0) throw new Error(`git status failed: ${firstLine(status.stderr)}`)
    const outputs = checkOutputs(p)
    const tracked: string[] = []
    const untracked: string[] = []
    for (const entry of status.stdout.split('\0').filter(Boolean)) {
      const file = entry.slice(3)
      if (!outputs.some((root) => file.startsWith(root))) outcome.kept.push(file)
      else (entry.startsWith('??') ? untracked : tracked).push(file)
    }
    const touched = [...tracked, ...untracked].sort()
    if (touched.length === 0) return outcome
    const backup = path.join(
      p.ui,
      '.agent-work',
      'figma-port',
      `restored-${new Date().toISOString().replace(/[:.]/g, '-')}`
    )
    for (const file of touched) {
      const source = path.join(p.repository, ...file.split('/'))
      if (!existsSync(source)) continue
      const target = path.join(backup, ...file.split('/'))
      mkdirSync(path.dirname(target), { recursive: true })
      copyFileSync(source, target)
    }
    outcome.backup = path.relative(p.ui, backup).split(path.sep).join('/')
    for (const [args, files] of [
      [['checkout', '--'], tracked],
      [['clean', '-fq', '--'], untracked]
    ] as const) {
      if (files.length === 0) continue
      const result = run('git', ['-C', p.repository, ...args, ...files], p.ui)
      if (result.code !== 0) throw new Error(`git ${args[0]} failed: ${firstLine(result.stderr)}`)
    }
    outcome.restored = touched
  } catch (error) {
    outcome.error = error instanceof Error ? error.message : String(error)
  }
  return outcome
}

const headOf = (run: Run, p: Paths): string => {
  const revParse = run('git', ['-C', p.repository, 'rev-parse', 'HEAD'], p.ui)
  if (revParse.code !== 0) throw new Error('port verify needs a committed HEAD')
  return revParse.stdout.trim()
}

/**
 * Runs the seven checks against the verified cache on a clean tree and, only when all pass, writes the record. A
 * failed check writes no record, leaves an existing one byte-identical and puts back what the checks wrote.
 */
function portVerify(
  figma: string,
  { run = spawnRun, stages }: { run?: Run; stages?: Record<string, () => Stage> } = {}
): VerifyResult {
  const p = paths(figma, run)
  const checks: StageResult[] = []
  if (changes(run, p))
    return {
      ok: false,
      failed: 'clean-tree',
      reason: 'library/ui has uncommitted changes: commit the port before verifying it',
      checks
    }
  const head = headOf(run, p)
  const all = stages ?? verifyStages(run, p)
  for (const name of REQUIRED_CHECKS) {
    const result = runStage(name, all[name])
    if (result.ok && !RECORD_DETAIL.test(result.detail)) {
      result.ok = false
      result.detail = `the ${name} report does not fit the record detail format`
    }
    checks.push(result)
    if (!result.ok)
      return {
        ok: false,
        failed: name,
        reason: result.detail,
        checks,
        restore: restoreTree(run, p)
      }
  }
  if (changes(run, p))
    return {
      ok: false,
      failed: 'clean-tree',
      reason: 'the checks changed tracked files',
      checks,
      restore: restoreTree(run, p)
    }
  // The checks take minutes: the record must name the commit they ran on.
  if (headOf(run, p) !== head)
    return { ok: false, failed: 'head', reason: 'HEAD moved while the checks ran', checks }
  const lock = JSON.parse(readFileSync(path.join(p.ui, 'upstream.lock.json'), 'utf8')) as {
    sources: { repository: string; revision: string; snapshot: { revision: string } }[]
  }
  const source = lock.sources.find((entry) => entry.repository.endsWith('/figma-design-system'))
  if (!source)
    return { ok: false, failed: 'record', reason: 'the lock has no capture source', checks }
  // The same exemption scan CI runs: a head it would fail never gets a record.
  const scan = scanProblems(trackedBlobsAt(p.repository, head)) as string[]
  if (scan.length) return { ok: false, failed: 'scan', reason: scan.join('; '), checks }
  const inputHash = portInputHash(lock, trackedFilesAt(p.repository, head)) as string
  const record = {
    schema: 2,
    portInputHash: inputHash,
    fdsRevision: source.revision,
    snapshotRevision: source.snapshot.revision,
    checks: Object.fromEntries(
      checks.map((check) => [check.name, { result: 'pass', detail: check.detail }])
    )
  }
  const problems = recordProblems(record, { lock, inputHash }) as string[]
  if (problems.length) return { ok: false, failed: 'record', reason: problems.join('; '), checks }
  // Written beside the record and renamed over it, so an interrupted write never truncates an existing record.
  const file = path.join(p.ui, 'port-verification.json')
  const temporary = `${file}.tmp`
  writeFileSync(temporary, JSON.stringify(record, null, 2) + '\n')
  renameSync(temporary, file)
  return { ok: true, record: gitPath(p, file), portInputHash: inputHash, checks }
}

/** The ordered, mutating preparation of a port. It stops at the first stage that fails and names it. */
function prepareStages(run: Run, p: Paths): Record<string, () => Stage> {
  return {
    'fetch-source': () => {
      const report = cliStage(run, p, ['fetch-source'], 'fetch-source')
      return {
        name: 'fetch-source',
        detail: `materialized ${countOf(report.files, 'fetch-source')} capture files`
      }
    },
    diff: () => {
      // A refresh is expected to differ (diff then exits 1 with its report); only a crash or ledger issues stop it.
      const result = cli(run, p.figma, ['diff'])
      if (result.code !== 0 && !result.stdout.trim()) fail(`diff failed: ${failureReason(result)}`)
      const report = json(result.stdout, 'diff')
      if ((report.ledgerIssues as unknown[] | undefined)?.length)
        fail('diff reports ledger issues: resolve them in figma-design-system first')
      const diff = (report.diff ?? {}) as Record<string, unknown[]>
      return {
        name: 'diff',
        detail: `${diff.added?.length ?? 0} added, ${diff.changed?.length ?? 0} changed, ${diff.removed?.length ?? 0} removed`
      }
    },
    sync: () => {
      cliStage(run, p, ['sync'], 'sync')
      return { name: 'sync', detail: 'generated the Review previews from the capture' }
    },
    'sync-again': () => {
      const before = contentSnapshot(run, p)
      cliStage(run, p, ['sync'], 'second sync')
      if (contentSnapshot(run, p) !== before)
        fail('a second sync changed files: generation is not stable')
      return { name: 'sync-again', detail: 'a second sync changed nothing' }
    },
    'install-foundations': () => {
      cliStage(run, p, ['install', 'foundations'], 'install foundations')
      return { name: 'install-foundations', detail: 'installed the foundations' }
    },
    'install-components': () => {
      cliStage(run, p, ['install', 'components'], 'install components')
      return { name: 'install-components', detail: 'installed the components' }
    },
    'lock-digests': () => {
      // lock-digests reads the index, so what sync and install just wrote is staged first; otherwise it would pin the
      // previous bytes and leave new generated files unpinned.
      const staged = run('git', ['-C', p.repository, 'add', '-A', '--', gitPath(p, p.ui)], p.ui)
      if (staged.code !== 0) fail(`git add failed: ${firstLine(staged.stderr)}`)
      const result = run(
        process.execPath,
        [path.join(p.figma, 'lock-digests.mjs'), '--write'],
        p.ui
      )
      if (result.code !== 0) fail(`lock-digests --write failed: ${failureReason(result)}`)
      return { name: 'lock-digests', detail: 'staged library/ui and rewrote the lock digests' }
    }
  }
}

const PREPARE_ORDER = [
  'fetch-source',
  'diff',
  'sync',
  'sync-again',
  'install-foundations',
  'install-components',
  'lock-digests'
]

type PrepareResult = {
  ok: boolean
  stages: StageResult[]
  failed?: string
  reason?: string
  next?: string
}

// The one uncommitted file a port may start from: the lock whose capture pin was just bumped (fetch-source --seal).
function unexpectedChanges(run: Run, p: Paths): string[] {
  const lock = gitPath(p, path.join(p.ui, 'upstream.lock.json'))
  return changes(run, p)
    .split('\n')
    .filter(Boolean)
    .map((line) => line.slice(3))
    .filter((file) => file !== lock)
}

/**
 * Runs the stages in order and stops at the first that fails, naming it. It starts only from a tree whose sole change
 * is the lock, so the final `git add -A` stages only what the port wrote.
 */
function portPrepare(
  figma: string,
  { run = spawnRun, stages }: { run?: Run; stages?: Record<string, () => Stage> } = {}
): PrepareResult {
  const p = paths(figma, run)
  const dirty = unexpectedChanges(run, p)
  if (dirty.length)
    return {
      ok: false,
      stages: [],
      failed: 'clean-tree',
      reason: `library/ui has changes other than upstream.lock.json: ${dirty.slice(0, 5).join(', ')}${dirty.length > 5 ? ', ...' : ''}. Commit or remove them first.`
    }
  const all = stages ?? prepareStages(run, p)
  const results: StageResult[] = []
  for (const name of PREPARE_ORDER) {
    const result = runStage(name, all[name])
    results.push(result)
    if (!result.ok) return { ok: false, stages: results, failed: name, reason: result.detail }
  }
  return {
    ok: true,
    stages: results,
    next: 'Record a deviation code for every changed file in upstream.lock.json, stage library/ui again, rerun `node figma/lock-digests.mjs --write` (it reads the index), commit, then run `pnpm figma port verify`.'
  }
}

export { PREPARE_ORDER, StageError, contentSnapshot, portPrepare, portVerify, prepareStages }
export type { PrepareResult, Run, Stage, VerifyResult }

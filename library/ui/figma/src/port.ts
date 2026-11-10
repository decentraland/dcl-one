// `dcl-figma port prepare` and `port verify`: the maintainer path for porting a figma-design-system capture into
// library/ui, as two ordered commands instead of a hand-run sequence. prepare mutates the tree and stops at the first
// failed stage; verify changes nothing until every check has passed, then writes library/ui/port-verification.json
// from what each check printed. The record states what this local run reported. CI checks that it names the head's
// port inputs and that every result is `pass`; it is not proof that the checks ran, an independent review, an
// attestation or evidence of live Figma parity.
import { spawnSync } from 'node:child_process'
import { readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import {
  REQUIRED_CHECKS,
  portInputHash,
  recordProblems,
  trackedFilesAt
} from '../port-verification.mjs'

type Run = (
  command: string,
  args: string[],
  cwd: string
) => { code: number | null; stdout: string; stderr: string }

type Stage = { name: string; detail: string }
type StageResult = { name: string; ok: boolean; detail: string }

const DETAIL = /^[\w ,.:=()/-]{0,120}$/
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

// Uncommitted or untracked files under library/ui. A failing git status is an error, never an empty answer.
function changes(run: Run, p: Paths): string {
  const result = run(
    'git',
    [
      '-C',
      p.repository,
      'status',
      '--porcelain',
      '--untracked-files=all',
      '--',
      path.relative(p.repository, p.ui),
      `:!${path.relative(p.repository, path.join(p.ui, 'node_modules'))}`,
      `:!${path.relative(p.repository, path.join(p.figma, 'node_modules'))}`,
      `:!${path.relative(p.repository, path.join(p.ui, '.agent-work'))}`
    ],
    p.ui
  )
  if (result.code !== 0) throw new Error(`git status failed: ${firstLine(result.stderr)}`)
  return result.stdout
}

const count = (directory: string): number =>
  readdirSync(directory, { recursive: true, withFileTypes: true }).filter((entry) => entry.isFile())
    .length

function cliStage(run: Run, p: Paths, args: string[], what: string): Record<string, unknown> {
  const result = cli(run, p.figma, args)
  if (result.code !== 0) fail(`${what} failed: ${firstLine(result.stderr || result.stdout)}`)
  return json(result.stdout, what)
}

const firstLine = (text: string) => text.trim().split('\n')[0]?.slice(0, 200) ?? ''

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
        detail: `Verified ${Number(report.files)} cache files against the pinned revision and digest table`
      }
    },
    deviations: () => {
      const result = run(process.execPath, [path.join(p.figma, 'lock-digests.mjs')], p.ui)
      const pins = /lock-digests: (\d+) files match/.exec(result.stdout)?.[1]
      if (result.code !== 0 || !pins) fail(`lock-digests failed: ${firstLine(result.stdout)}`)
      const { publicFoundations, publicComponents } = lock()
      return {
        name: 'deviations',
        detail: `${pins} tooling pins, ${Object.keys(publicFoundations?.files ?? {}).length} foundation origins and ${Object.keys(publicComponents?.files ?? {}).length} component outputs pass the common guard`
      }
    },
    diff: () => {
      const report = cliStage(run, p, ['diff'], 'diff')
      const diff = report.diff as Record<string, unknown[]>
      return {
        name: 'diff',
        detail: `Fresh diff reports ${diff.added?.length} added, ${diff.changed?.length} changed, ${diff.removed?.length} removed and no ledger issues`
      }
    },
    'check-mappings': () => {
      const report = cliStage(run, p, ['check-mappings'], 'check-mappings')
      if (report.captureChecked !== true) fail('check-mappings did not read the capture')
      const checked = Number(report.checked)
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
        detail: `Fresh catalog regeneration is byte-identical, ${Number(report.components)} components`
      }
    },
    'navbar-bounds': () => {
      const result = run(
        path.join(p.ui, 'node_modules', '.bin', 'vitest'),
        ['run', '--project', 'consumer', 'navbar-bounds'],
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
      rmSync(path.join(p.ui, '.agent-work/figma-install'), { recursive: true, force: true })
      const counts: number[] = []
      for (const target of ['foundations', 'components']) {
        cliStage(
          run,
          p,
          ['install', target, '--out', `../${PROOF}/${target}`],
          `private install ${target}`
        )
        const same = run('diff', ['-r', path.join(proof, target), path.join(p.ui, target)], p.ui)
        if (same.code !== 0) fail(`the private ${target} install differs from the canonical one`)
        counts.push(count(path.join(proof, target)))
      }
      return {
        name: 'install',
        detail: `Private and canonical installs reproduce ${counts[0]} foundation and ${counts[1]} component files with no-op repeats`
      }
    }
  }
}

type VerifyResult =
  | { ok: true; record: string; portInputHash: string; checks: StageResult[] }
  | { ok: false; failed: string; reason: string; checks: StageResult[] }

/**
 * Runs the seven checks against the verified cache on a clean tree and, only when all pass, writes the record. A
 * failed check writes nothing, so an existing record stays byte-identical.
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
  const revParse = run('git', ['-C', p.repository, 'rev-parse', 'HEAD'], p.ui)
  if (revParse.code !== 0) throw new Error('port verify needs a committed HEAD')
  const head = revParse.stdout.trim()
  const all = stages ?? verifyStages(run, p)
  for (const name of REQUIRED_CHECKS) {
    try {
      const stage = all[name]
      if (!stage) fail(`no runner for ${name}`)
      const { detail } = stage!()
      if (!DETAIL.test(detail)) fail(`the ${name} report does not fit the record detail format`)
      checks.push({ name, ok: true, detail })
    } catch (error) {
      if (!(error instanceof StageError)) throw error
      checks.push({ name, ok: false, detail: error.message })
      return { ok: false, failed: name, reason: error.message, checks }
    }
  }
  if (changes(run, p))
    return { ok: false, failed: 'clean-tree', reason: 'the checks changed tracked files', checks }
  const lock = JSON.parse(readFileSync(path.join(p.ui, 'upstream.lock.json'), 'utf8')) as {
    sources: { repository: string; revision: string; snapshot: { revision: string } }[]
  }
  const source = lock.sources.find((entry) => entry.repository.endsWith('/figma-design-system'))
  if (!source)
    return { ok: false, failed: 'record', reason: 'the lock has no capture source', checks }
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
  const file = path.join(p.ui, 'port-verification.json')
  writeFileSync(file, JSON.stringify(record, null, 2) + '\n')
  return { ok: true, record: path.relative(p.repository, file), portInputHash: inputHash, checks }
}

/** The ordered, mutating preparation of a port. It stops at the first stage that fails and names it. */
function prepareStages(run: Run, p: Paths): Record<string, () => Stage> {
  return {
    'fetch-source': () => {
      const report = cliStage(run, p, ['fetch-source'], 'fetch-source')
      return { name: 'fetch-source', detail: `materialized ${Number(report.files)} capture files` }
    },
    diff: () => {
      // A refresh is expected to differ; only a crash or ledger issues stop the port.
      const result = cli(run, p.figma, ['diff'])
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
      const before = changes(run, p)
      cliStage(run, p, ['sync'], 'second sync')
      if (changes(run, p) !== before) fail('a second sync changed files: generation is not stable')
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
      const result = run(
        process.execPath,
        [path.join(p.figma, 'lock-digests.mjs'), '--write'],
        p.ui
      )
      if (result.code !== 0) fail(`lock-digests --write failed: ${firstLine(result.stdout)}`)
      return { name: 'lock-digests', detail: 'rewrote the lock digests' }
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

type PrepareResult = { ok: boolean; stages: StageResult[]; failed?: string; next?: string }

function portPrepare(
  figma: string,
  { run = spawnRun, stages }: { run?: Run; stages?: Record<string, () => Stage> } = {}
): PrepareResult {
  const all = stages ?? prepareStages(run, paths(figma, run))
  const results: StageResult[] = []
  for (const name of PREPARE_ORDER) {
    try {
      const { detail } = all[name]!()
      results.push({ name, ok: true, detail })
    } catch (error) {
      if (!(error instanceof StageError)) throw error
      results.push({ name, ok: false, detail: error.message })
      return { ok: false, stages: results, failed: name }
    }
  }
  return {
    ok: true,
    stages: results,
    next: 'Record the deviations of changed files in upstream.lock.json, commit, then run `pnpm figma port verify`.'
  }
}

export { PREPARE_ORDER, StageError, portPrepare, portVerify }
export type { PrepareResult, Run, Stage, VerifyResult }

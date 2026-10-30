// The one command that runs the suite:
//
//   node scripts/e2e.ts [--inworld | --visual] [--filter <name pattern>] [--record] [-- <node --test args>]
//
// Every spec file runs in this one process (`--test-isolation=none`), one at a time, so the
// client boots once. The spec reporter goes to stdout and a JUnit report to
// report/junit-<groups>.xml (`junit-inworld.xml`, `junit-visual.xml`, `junit-inworld-visual.xml`).
import { spawn } from 'node:child_process'
import { mkdir } from 'node:fs/promises'
import path from 'node:path'
import { PACKAGE_ROOT, REPORT_DIR } from '../driver/paths.ts'
import { describeTarget, readTarget } from '../driver/target.ts'

interface Options {
  groups: string[]
  filter?: string
  record: boolean
  extra: string[]
}

export function parseArgs(argv: string[]): Options {
  const options: Options = { groups: [], record: false, extra: [] }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!
    if (arg === '--') {
      options.extra = argv.slice(i + 1)
      break
    } else if (arg === '--inworld' || arg === '--visual') options.groups.push(arg.slice(2))
    else if (arg === '--record') options.record = true
    else if (arg === '--filter') {
      options.filter = argv[++i]
      if (options.filter === undefined) throw new Error('--filter needs a pattern')
    } else throw new Error(`unknown argument ${arg}`)
  }
  if (options.groups.length === 0) options.groups = ['inworld', 'visual']
  return options
}

export function specPatterns(groups: string[]): string[] {
  const patterns: string[] = []
  if (groups.includes('inworld')) patterns.push('specs/*.test.ts')
  if (groups.includes('visual')) patterns.push('specs/visual/*.test.ts')
  return patterns
}

export function nodeArgs(options: Options, junit: string): string[] {
  const args = [
    '--test',
    '--test-isolation=none',
    '--test-concurrency=1',
    '--test-reporter=spec',
    '--test-reporter-destination=stdout',
    '--test-reporter=junit',
    `--test-reporter-destination=${junit}`
  ]
  if (options.filter) args.push('--test-name-pattern', options.filter)
  args.push(...options.extra, ...specPatterns(options.groups))
  return args
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  const env = { ...process.env, ...(options.record ? { DCL_E2E_RECORD: '1' } : {}) }
  const target = readTarget(env)
  await mkdir(REPORT_DIR, { recursive: true })
  const junit = path.join(REPORT_DIR, `junit-${options.groups.join('-')}.xml`)
  console.error(`[e2e] ${describeTarget(target)}`)
  console.error(`[e2e] specs: ${specPatterns(options.groups).join(' ')}; report: ${REPORT_DIR}`)
  const child = spawn(process.execPath, nodeArgs(options, junit), { cwd: PACKAGE_ROOT, env, stdio: 'inherit' })
  const code = await new Promise<number>((resolve) => child.on('exit', (exitCode) => resolve(exitCode ?? 1)))
  process.exitCode = code
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(PACKAGE_ROOT, 'scripts', 'e2e.ts')) {
  await main()
}

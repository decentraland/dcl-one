// Drives one client by hand or by agent: serves the web app and a loopback realm holding the
// given scenes, boots a guest there, then runs each console line read from stdin.
//
//   node scripts/drive.ts --scene <dir> [--scene <dir> ...] [--position x,y] [--report <dir>] [--web-app <dir>] < commands.txt
//
// Paths are read from the folder the command was started in (`INIT_CWD` under a package manager).
// Every line runs in turn; blank lines and `#` lines are skipped. stdout carries one JSON line
// per command, `{line, ok, reply, ms}`, with a `/login_identity` or `/hud_login` payload redacted
// in both; stderr carries the boot and a short log. When stdin closes, the frame is kept as
// `final.png` and the replies as `replies.jsonl` in the report folder (`report/drive` by default),
// and the client and the servers are closed. An interrupt (SIGINT, SIGTERM) or a closed stdout
// closes them too, a boot in progress included, and keeps no report.
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { ConsoleError, redact, redactLine } from '../driver/console.ts'
import type { Explorer } from '../driver/explorer.ts'
import { PACKAGE_ROOT, REPORT_DIR } from '../driver/paths.ts'
import { buildScene, type BuiltScene, serveRealm } from '../driver/realm.ts'
import { closeSessions, openSession, parsePosition } from '../driver/session.ts'
import { readTarget } from '../driver/target.ts'

export interface Options {
  scenes: string[]
  position?: string
  report: string
  webApp?: string
}

export interface Result {
  line: string
  ok: boolean
  reply: string
  ms: number
}

/** The folder the command was started in: a package manager runs the script from the package. */
export function startFolder(env: NodeJS.ProcessEnv = process.env): string {
  return env.INIT_CWD || process.cwd()
}

export function parseArgs(argv: string[], cwd: string = startFolder()): Options {
  const options: Options = { scenes: [], report: path.join(REPORT_DIR, 'drive') }
  const value = (i: number, name: string): string => {
    const next = argv[i]
    if (next === undefined || next.startsWith('--')) throw new Error(`${name} needs a value`)
    return next
  }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!
    // `pnpm run drive -- --scene ...` hands the separator on
    if (arg === '--') continue
    if (arg === '--scene') options.scenes.push(path.resolve(cwd, value(++i, arg)))
    else if (arg === '--position') {
      options.position = value(++i, arg)
      parsePosition(options.position)
    } else if (arg === '--report') options.report = path.resolve(cwd, value(++i, arg))
    else if (arg === '--web-app') options.webApp = path.resolve(cwd, value(++i, arg))
    else throw new Error(`unknown argument ${arg}`)
  }
  if (options.scenes.length === 0) throw new Error('--scene <dir> is needed at least once')
  return options
}

/** The console line a stdin line carries, or nothing for a blank or `#` line. */
export function commandLine(text: string): string | undefined {
  const line = text.trim()
  return line === '' || line.startsWith('#') ? undefined : line
}

/** Run each command line in turn, handing every result to `out`; a failed command is a result
 *  with `ok:false` and the run goes on. A result's line and reply have the line's credential
 *  redacted, as the suite's reports do. */
export async function runLines(lines: AsyncIterable<string>, explorer: Pick<Explorer, 'run'>, out: (result: Result) => void): Promise<Result[]> {
  const results: Result[] = []
  for await (const text of lines) {
    const line = commandLine(text)
    if (line === undefined) continue
    const startedAt = Date.now()
    let result: Result
    try {
      result = { line: redactLine(line), ok: true, reply: redact(line, await explorer.run(line)), ms: 0 }
    } catch (error) {
      const reply = error instanceof ConsoleError ? error.output : error instanceof Error ? error.message : String(error)
      result = { line: redactLine(line), ok: false, reply: redact(line, reply), ms: 0 }
    }
    result.ms = Date.now() - startedAt
    results.push(result)
    out(result)
  }
  return results
}

/** Where a run arrives when no `--position` is given: the scene's base parcel, else its first. */
export function baseParcel(scene: Pick<BuiltScene, 'entity'>): string {
  const metadata = scene.entity.metadata as { scene?: { base?: unknown } } | undefined
  const base = metadata?.scene?.base
  return typeof base === 'string' ? base : scene.entity.pointers[0]!
}

/** The exit code of a run ended by a signal, as a shell reports it. */
export const SIGNAL_EXIT = { SIGINT: 130, SIGTERM: 143 } as const

export interface Ending {
  /** Whether the run was told to end: nothing more is reported, and `end` owns the exit. */
  readonly ended: boolean
  /** End the run: close once, then exit with `code`. A second call while closing exits at once. */
  end(code: number, why: string): void
  /** Stop listening for signals; call it once the run has closed by itself. */
  release(): void
}

/** End the run when the process is interrupted: `close` runs once, then the process exits with the
 *  signal's code. */
export function closeOnSignal(close: () => Promise<void>, exit: (code: number) => void = (code) => process.exit(code)): Ending {
  let ended = false
  const end = (code: number, why: string): void => {
    if (ended) return exit(code)
    ended = true
    console.error(`[drive] ${why}: closing the client and the servers`)
    void close()
      .catch((error: unknown) => console.error(`[drive] close failed: ${error instanceof Error ? error.message : String(error)}`))
      .then(() => exit(code))
  }
  const handlers = (Object.keys(SIGNAL_EXIT) as (keyof typeof SIGNAL_EXIT)[]).map((signal) => {
    const handler = (): void => end(SIGNAL_EXIT[signal], signal)
    process.on(signal, handler)
    return [signal, handler] as const
  })
  return {
    get ended() {
      return ended
    },
    end,
    release: () => {
      for (const [signal, handler] of handlers) process.off(signal, handler)
    }
  }
}

/** `buildScene`, with how to build the folder when its bundle is missing. */
async function build(dir: string): Promise<BuiltScene> {
  try {
    return await buildScene(dir)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (!message.includes('build the scene first')) throw error
    throw new Error(`${message}: \`npx sdk-commands build\` in that folder, or \`pnpm run build-scenes\` here for the suite's own scenes`, { cause: error })
  }
}

function short(text: string): string {
  const flat = text.replace(/\s+/g, ' ')
  return flat.length > 160 ? `${flat.slice(0, 160)}... (${text.length} chars)` : flat
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  const read = readTarget()
  if (options.webApp && read.webUrl) throw new Error('--web-app serves a folder, and DCL_E2E_WEB_URL opens a served app instead: give one of them')
  const target = { ...read, webApp: options.webApp ?? read.webApp }
  const scenes = await Promise.all(options.scenes.map(build))
  const position = options.position ?? baseParcel(scenes[0]!)
  const realm = await serveRealm({ scenes, port: target.ports.realm })
  const boot = new AbortController()
  let closed: Promise<void> | undefined
  // one close for every way out; it gives a boot in progress up first, so it never waits on one
  const close = (): Promise<void> =>
    (closed ??= (async () => {
      boot.abort()
      await closeSessions()
      await realm.close()
    })())
  const ending = closeOnSignal(close)
  // a reader that went away (`drive ... | head -1`)
  process.stdout.on('error', () => ending.end(1, 'stdout closed'))
  console.error(`[drive] realm ${realm.realm}: ${scenes.map((scene) => scene.entity.pointers[0]).join(' ')}`)
  try {
    const { explorer } = await openSession({ realm: realm.realm, position, signIn: 'guest', signal: boot.signal }, target)
    console.error(`[drive] in world at ${position}; reading commands from stdin`)
    const results = await runLines(createInterface({ input: process.stdin, crlfDelay: Infinity }), explorer, (result) => {
      if (ending.ended) return
      process.stdout.write(`${JSON.stringify(result)}\n`)
      console.error(`[drive] ${result.ok ? 'ok' : 'FAILED'} ${result.ms}ms ${result.line}: ${short(result.reply)}`)
    })
    if (ending.ended) return
    await mkdir(options.report, { recursive: true })
    await writeFile(path.join(options.report, 'replies.jsonl'), results.map((result) => JSON.stringify(result)).join('\n') + '\n')
    await writeFile(path.join(options.report, 'final.png'), await explorer.screenshot())
    console.error(`[drive] ${results.length} commands, ${results.filter((result) => !result.ok).length} failed; report in ${options.report}`)
  } catch (error) {
    // an ended run fails whatever was in flight; its exit is `end`'s
    if (!ending.ended) throw error
  } finally {
    await close()
    ending.release()
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(PACKAGE_ROOT, 'scripts', 'drive.ts')) {
  await main()
}

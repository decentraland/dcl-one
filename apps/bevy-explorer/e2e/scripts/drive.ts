// Drives one client by hand or by agent: serves the web app and a loopback realm holding the
// given scenes, boots a guest there, then runs each console line read from stdin.
//
//   node scripts/drive.ts --scene <dir> [--scene <dir> ...] [--position x,y] [--report <dir>] [--web-app <dir>] < commands.txt
//
// Every line runs in turn; blank lines and `#` lines are skipped. stdout carries one JSON line
// per command, `{line, ok, reply, ms}`; stderr carries the boot and a short log. When stdin
// closes, the frame is kept as `final.png` and the replies as `replies.jsonl` in the report
// folder (`report/drive` by default), and the client and the servers are closed. An interrupt
// (SIGINT, SIGTERM) closes them too, keeping no report.
import { mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { ConsoleError } from '../driver/console.ts'
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

export function parseArgs(argv: string[], cwd: string = process.cwd()): Options {
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
 *  with `ok:false` and the run goes on. */
export async function runLines(lines: AsyncIterable<string>, explorer: Pick<Explorer, 'run'>, out: (result: Result) => void): Promise<Result[]> {
  const results: Result[] = []
  for await (const text of lines) {
    const line = commandLine(text)
    if (line === undefined) continue
    const startedAt = Date.now()
    let result: Result
    try {
      result = { line, ok: true, reply: await explorer.run(line), ms: 0 }
    } catch (error) {
      const reply = error instanceof ConsoleError ? error.output : error instanceof Error ? error.message : String(error)
      result = { line, ok: false, reply, ms: 0 }
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

/** Close the run once when the process is interrupted, then exit with the signal's code; a second
 *  signal while closing exits at once. Returns the call that removes the handlers. */
export function closeOnSignal(close: () => Promise<void>, exit: (code: number) => void = (code) => process.exit(code)): () => void {
  let closing = false
  const handlers = (Object.keys(SIGNAL_EXIT) as (keyof typeof SIGNAL_EXIT)[]).map((signal) => {
    const handler = (): void => {
      if (closing) return exit(SIGNAL_EXIT[signal])
      closing = true
      console.error(`[drive] ${signal}: closing the client and the servers`)
      void close()
        .catch((error: unknown) => console.error(`[drive] close failed: ${error instanceof Error ? error.message : String(error)}`))
        .then(() => exit(SIGNAL_EXIT[signal]))
    }
    process.on(signal, handler)
    return [signal, handler] as const
  })
  return () => {
    for (const [signal, handler] of handlers) process.off(signal, handler)
  }
}

function short(text: string): string {
  const flat = text.replace(/\s+/g, ' ')
  return flat.length > 160 ? `${flat.slice(0, 160)}... (${text.length} chars)` : flat
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  const read = readTarget()
  const target = { ...read, webApp: options.webApp ?? read.webApp }
  const scenes = await Promise.all(options.scenes.map((dir) => buildScene(dir)))
  const position = options.position ?? baseParcel(scenes[0]!)
  const realm = await serveRealm({ scenes, port: target.ports.realm })
  const close = async (): Promise<void> => {
    await closeSessions()
    await realm.close()
  }
  const release = closeOnSignal(close)
  console.error(`[drive] realm ${realm.realm}: ${scenes.map((scene) => scene.entity.pointers[0]).join(' ')}`)
  try {
    const { explorer } = await openSession({ realm: realm.realm, position, signIn: 'guest' }, target)
    console.error(`[drive] in world at ${position}; reading commands from stdin`)
    const results = await runLines(createInterface({ input: process.stdin, crlfDelay: Infinity }), explorer, (result) => {
      process.stdout.write(`${JSON.stringify(result)}\n`)
      console.error(`[drive] ${result.ok ? 'ok' : 'FAILED'} ${result.ms}ms ${result.line}: ${short(result.reply)}`)
    })
    await mkdir(options.report, { recursive: true })
    await writeFile(path.join(options.report, 'replies.jsonl'), results.map((result) => JSON.stringify(result)).join('\n') + '\n')
    await writeFile(path.join(options.report, 'final.png'), await explorer.screenshot())
    console.error(`[drive] ${results.length} commands, ${results.filter((result) => !result.ok).length} failed; report in ${options.report}`)
  } finally {
    release()
    await close()
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(PACKAGE_ROOT, 'scripts', 'drive.ts')) {
  await main()
}

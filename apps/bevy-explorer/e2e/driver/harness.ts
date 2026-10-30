// `scenario`: one end-to-end test. It shares the process's client, starts with every panel closed
// (Escape, as explorer-automation's SetUp does), records every console command and keeps the
// final frame, whatever the outcome. A scenario the client cannot run is a `todo` with its
// reason: the report lists it, and it is never a pass.
import { test, type TestContext } from 'node:test'
import path from 'node:path'
import type { Explorer } from './explorer.ts'
import { ScenarioReport } from './report.ts'
import { BOOT_ALLOWANCE_MS, openExplorer, type OpenOptions } from './session.ts'
import { TimeoutError } from './wait.ts'

export interface ScenarioContext {
  explorer: Explorer
  t: TestContext
  report: ScenarioReport
  /** Register how to put back account state the scenario changes (the account is shared with
   *  every later scenario and run). The undos run after the body, whatever its outcome, once the
   *  final frame is kept, last registered first; register one before the change it undoes. */
  restore(undo: () => Promise<void>): void
}

export interface ScenarioOptions {
  /** The reason the client cannot run this scenario yet; the body does not run. */
  todo?: string
  /** A reason to skip; prefer `todo`, which the report counts apart. */
  skip?: string
  /** The body's budget, and each undo's; the first scenario's boot has its own (`BOOT_ALLOWANCE_MS`). */
  timeoutMs?: number
  /** Which client: the default session unless given. */
  session?: OpenOptions
  /** Keep the HUD as the scenario left it (no Escape first). */
  noEscape?: boolean
}

export const DEFAULT_SCENARIO_TIMEOUT_MS = 180_000

/** The spec file's name for the report, from its `import.meta.url`. */
export function specName(importMetaUrl: string): string {
  return path.basename(new URL(importMetaUrl).pathname).replace(/\.test\.ts$/, '')
}

/** `work`, or a `TimeoutError` once `timeoutMs` have passed. The work is not cancelled: a
 *  scenario's final frame is taken while its body may still be running. */
export function withinBudget<T>(work: Promise<T>, timeoutMs: number, what: string): Promise<T> {
  let timer: NodeJS.Timeout | undefined
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new TimeoutError(what, timeoutMs, undefined)), timeoutMs)
  })
  return Promise.race([work, deadline]).finally(() => clearTimeout(timer))
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function scenario(file: string, name: string, body: (ctx: ScenarioContext) => Promise<void>, options: ScenarioOptions = {}): void {
  if (options.todo) {
    test(name, { todo: options.todo }, () => {})
    return
  }
  if (options.skip) {
    test(name, { skip: options.skip }, () => {})
    return
  }
  const timeoutMs = options.timeoutMs ?? DEFAULT_SCENARIO_TIMEOUT_MS
  // The first scenario of a session boots the client, whose waits bound it; the scenario's own
  // budget starts once it is up. Node's timeout is the sum, and the undos' backstop.
  test(name, { timeout: BOOT_ALLOWANCE_MS + 2 * timeoutMs }, async (t) => {
    const explorer = await openExplorer(options.session)
    const report = new ScenarioReport(file, name)
    const stop = explorer.onCommand((record) => report.record(record))
    const files: Record<string, Uint8Array | string> = {}
    const undos: (() => Promise<void>)[] = []
    const failures: unknown[] = []
    try {
      if (!options.noEscape) await explorer.escape()
      await withinBudget(body({ explorer, t, report, restore: (undo) => undos.push(undo) }), timeoutMs, `the scenario "${name}" to finish`)
    } catch (error) {
      failures.push(error)
    }
    try {
      files['final.png'] = await explorer.screenshot()
    } catch (error) {
      files['final.txt'] = `no final screenshot: ${message(error)}`
    }
    for (const undo of undos.reverse()) {
      try {
        await withinBudget(undo(), timeoutMs, `the restore after "${name}" to finish`)
      } catch (error) {
        failures.push(error)
      }
    }
    stop()
    await report.write(files)
    t.diagnostic(`${report.commandCount} console commands; report in ${report.dir}`)
    if (failures.length === 1) throw failures[0]
    if (failures.length > 1) throw new AggregateError(failures, failures.map(message).join('; and the restore failed: '))
  })
}

/** A `scenario` bound to one spec file. */
export function spec(importMetaUrl: string): (name: string, body: (ctx: ScenarioContext) => Promise<void>, options?: ScenarioOptions) => void {
  const file = specName(importMetaUrl)
  return (name, body, options) => scenario(file, name, body, options)
}

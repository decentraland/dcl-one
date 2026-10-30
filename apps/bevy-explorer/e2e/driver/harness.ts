// `scenario`: one end-to-end test. It shares the process's client, starts with every panel closed
// (Escape, as explorer-automation's SetUp does), records every console command and keeps the
// final frame, whatever the outcome. A scenario the client cannot run is a `todo` with its
// reason: the report lists it, and it is never a pass.
import { test, type TestContext } from 'node:test'
import path from 'node:path'
import type { Explorer } from './explorer.ts'
import { ScenarioReport } from './report.ts'
import { openExplorer, type OpenOptions } from './session.ts'

export interface ScenarioContext {
  explorer: Explorer
  t: TestContext
  report: ScenarioReport
}

export interface ScenarioOptions {
  /** The reason the client cannot run this scenario yet; the body does not run. */
  todo?: string
  /** A reason to skip; prefer `todo`, which the report counts apart. */
  skip?: string
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

export function scenario(file: string, name: string, body: (ctx: ScenarioContext) => Promise<void>, options: ScenarioOptions = {}): void {
  if (options.todo) {
    test(name, { todo: options.todo }, () => {})
    return
  }
  if (options.skip) {
    test(name, { skip: options.skip }, () => {})
    return
  }
  test(name, { timeout: options.timeoutMs ?? DEFAULT_SCENARIO_TIMEOUT_MS }, async (t) => {
    const explorer = await openExplorer(options.session)
    const report = new ScenarioReport(file, name)
    const stop = explorer.onCommand((record) => report.record(record))
    const files: Record<string, Uint8Array | string> = {}
    try {
      if (!options.noEscape) await explorer.escape()
      await body({ explorer, t, report })
    } finally {
      try {
        files['final.png'] = await explorer.screenshot()
      } catch (error) {
        files['final.txt'] = `no final screenshot: ${error instanceof Error ? error.message : String(error)}`
      }
      stop()
      await report.write(files)
      t.diagnostic(`${report.commandCount} console commands; report in ${report.dir}`)
    }
  })
}

/** A `scenario` bound to one spec file. */
export function spec(importMetaUrl: string): (name: string, body: (ctx: ScenarioContext) => Promise<void>, options?: ScenarioOptions) => void {
  const file = specName(importMetaUrl)
  return (name, body, options) => scenario(file, name, body, options)
}

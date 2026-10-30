// The web transport: the #194 web driver runs Chrome over the DevTools pipe and runs each line as
// `window.engine_console_command(line)` in the app frame, with real input for the HUD commands.
import { pathToFileURL } from 'node:url'
import { type Console, ConsoleError, normalizeLine } from './console.ts'
import { WEB_DRIVER } from './paths.ts'

export interface WebLaunchOptions {
  url: string
  chrome?: string
  headless?: boolean
  args?: string[]
  windowSize?: [number, number]
  /** How long `ready()` may take: the engine worker's start plus the page's first load. */
  readyTimeoutMs?: number
  /** How long one command may take. */
  commandTimeoutMs?: number
}

/** What the suite uses of `local-harness/web-driver/driver.mjs`. */
interface WebDriver {
  ready(options?: { timeout?: number }): Promise<void>
  console(line: string, options?: { timeout?: number }): Promise<string>
  frameTargets(): unknown[]
  close(): Promise<void> | void
  readonly product?: string
}

interface WebDriverModule {
  launch(options: { url: string; chrome?: string; headless?: boolean; args?: string[]; windowSize?: [number, number] }): Promise<WebDriver>
  findChrome(env?: NodeJS.ProcessEnv): string | undefined
}

let driverModule: Promise<WebDriverModule> | undefined

export function webDriverModule(): Promise<WebDriverModule> {
  driverModule ??= import(pathToFileURL(WEB_DRIVER).href) as Promise<WebDriverModule>
  return driverModule
}

/** The Chrome the web driver would launch, or undefined when none is installed. */
export async function findChrome(): Promise<string | undefined> {
  try {
    return (await webDriverModule()).findChrome()
  } catch {
    return undefined
  }
}

export class WebConsole implements Console {
  readonly kind = 'web' as const
  readonly #driver: WebDriver
  readonly #commandTimeoutMs: number

  private constructor(driver: WebDriver, commandTimeoutMs: number) {
    this.#driver = driver
    this.#commandTimeoutMs = commandTimeoutMs
  }

  /** Start Chrome on `url` and wait until the app frame answers console commands. */
  static async launch(options: WebLaunchOptions): Promise<WebConsole> {
    const { launch } = await webDriverModule()
    const driver = await launch({
      url: options.url,
      chrome: options.chrome,
      headless: options.headless ?? false,
      args: options.args ?? [],
      windowSize: options.windowSize ?? [1600, 900]
    })
    try {
      await driver.ready({ timeout: options.readyTimeoutMs ?? 300_000 })
    } catch (error) {
      await driver.close()
      throw error
    }
    return new WebConsole(driver, options.commandTimeoutMs ?? 120_000)
  }

  async run(line: string): Promise<string> {
    const normalized = normalizeLine(line)
    try {
      return await this.#driver.console(normalized, { timeout: this.#commandTimeoutMs })
    } catch (error) {
      throw new ConsoleError(normalized, error instanceof Error ? error.message : String(error))
    }
  }

  async close(): Promise<void> {
    await this.#driver.close()
  }
}

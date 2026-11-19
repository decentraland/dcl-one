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
  /** Gives the wait for the app up when aborted: the browser is closed and the launch fails with
   *  `BOOT_ABANDONED`. */
  signal?: AbortSignal
}

/** What the suite uses of `local-harness/web-driver/driver.mjs`. */
interface WebDriver {
  ready(options?: { timeout?: number }): Promise<void>
  console(line: string, options?: { timeout?: number }): Promise<string>
  frameTargets(): unknown[]
  consoleMessages(): { at: number; target: string; kind: string; text: string }[]
  screenshot(): Promise<Buffer>
  close(): Promise<void> | void
  readonly product?: string
}

/** A boot that did not become ready, with what the page said and how it looked: a timeout's
 *  message says only that the console never came. */
export class BootError extends Error {
  readonly console: string[]
  readonly screenshot: Buffer | undefined

  constructor(message: string, console: string[], screenshot: Buffer | undefined) {
    super(message)
    this.name = 'BootError'
    this.console = console
    this.screenshot = screenshot
  }
}

/** A console message as one line. */
export function consoleLine(message: { at: number; target: string; kind: string; text: string }): string {
  return `${new Date(message.at).toISOString().slice(11, 19)} ${message.target} ${message.kind}: ${message.text}`
}

interface WebDriverModule {
  /** What `console` rejects with when the command's document is replaced or closed first. */
  readonly APP_GONE: string
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

/** A command whose document was replaced or closed before it answered, as by a click that loads
 *  the page again; the next command runs in the new document. */
export class DocumentGoneError extends ConsoleError {
  constructor(line: string, output: string) {
    super(line, output)
    this.name = 'DocumentGoneError'
  }
}

/** What a boot fails with once its `signal` asked for it to be given up. */
export const BOOT_ABANDONED = 'the boot was abandoned'

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
    let driver: WebDriver
    try {
      driver = await launch({
        url: options.url,
        chrome: options.chrome,
        headless: options.headless ?? false,
        args: options.args ?? [],
        windowSize: options.windowSize ?? [1600, 900]
      })
    } catch (error) {
      // the driver closed the browser with the error; what the page said travels on the error
      const said = (error as { console?: unknown }).console
      if (Array.isArray(said)) throw new BootError(error instanceof Error ? error.message : String(error), said.map(consoleLine), undefined)
      throw error
    }
    const { signal } = options
    const abandon = (): void => void Promise.resolve(driver.close()).catch(() => {})
    signal?.addEventListener('abort', abandon, { once: true })
    try {
      if (signal?.aborted) throw new Error(BOOT_ABANDONED)
      await driver.ready({ timeout: options.readyTimeoutMs ?? 300_000 })
    } catch (error) {
      if (signal?.aborted) {
        await driver.close()
        throw new Error(BOOT_ABANDONED, { cause: error })
      }
      const said = driver.consoleMessages().map(consoleLine)
      const shot = await driver.screenshot().catch(() => undefined)
      await driver.close()
      throw new BootError(error instanceof Error ? error.message : String(error), said, shot)
    } finally {
      signal?.removeEventListener('abort', abandon)
    }
    return new WebConsole(driver, options.commandTimeoutMs ?? 120_000)
  }

  async run(line: string): Promise<string> {
    const normalized = normalizeLine(line)
    try {
      return await this.#driver.console(normalized, { timeout: this.#commandTimeoutMs })
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (message === (await webDriverModule()).APP_GONE) throw new DocumentGoneError(normalized, message)
      throw new ConsoleError(normalized, message)
    }
  }

  /** What the page said and how it looks now: for a boot that opened but never arrived. */
  async evidence(): Promise<{ console: string[]; screenshot: Buffer | undefined }> {
    return {
      console: this.#driver.consoleMessages().map(consoleLine),
      screenshot: await this.#driver.screenshot().catch(() => undefined)
    }
  }

  async close(): Promise<void> {
    await this.#driver.close()
  }
}

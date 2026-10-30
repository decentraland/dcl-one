// The console is the one interface tests have to the client: a line in, the command's reply out.
// A transport carries it (the web driver in a browser, the Bevy Remote Protocol on the standalone)
// and reports a failed command through its own channel, since the reply text carries no marker.

export interface Console {
  readonly kind: 'web' | 'native' | 'fake'
  /** Run one console line and resolve with its `Ok` text; reject with a `ConsoleError` on `Err`. */
  run(line: string): Promise<string>
  close(): Promise<void>
}

export class ConsoleError extends Error {
  readonly line: string
  readonly output: string

  constructor(line: string, output: string) {
    super(`\`${line}\` failed: ${output}`)
    this.name = 'ConsoleError'
    this.line = line
    this.output = output
  }
}

/** A line as the transports send it: one leading `/`, single spaces between arguments. */
export function normalizeLine(line: string): string {
  const trimmed = line.trim().replace(/\s+/g, ' ')
  if (trimmed === '') throw new Error('empty console command')
  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`
}

/** The command name of a line, without the `/`. */
export function commandOf(line: string): string {
  return normalizeLine(line).slice(1).split(' ', 1)[0] ?? ''
}

export type Reply = string | ((line: string) => string | Promise<string>)

/** A console that answers from a table, for the driver's own tests: `replies` maps a command name
 *  (without the `/`) to its reply, or to a function of the whole line. A reply function may throw
 *  a `ConsoleError` to fail the command. Every line sent is kept in `lines`. */
export class FakeConsole implements Console {
  readonly kind = 'fake' as const
  readonly lines: string[] = []
  closed = false
  readonly #replies: Record<string, Reply>

  constructor(replies: Record<string, Reply> = {}) {
    this.#replies = replies
  }

  async run(line: string): Promise<string> {
    const normalized = normalizeLine(line)
    this.lines.push(normalized)
    const reply = this.#replies[commandOf(normalized)]
    if (reply === undefined) {
      const known = Object.keys(this.#replies)
        .sort()
        .map((name) => `"/${name}"`)
        .join(', ')
      throw new ConsoleError(normalized, `Command not recognized: \`${normalized.split(' ', 1)[0]}\`. Recognized commands: [${known}]`)
    }
    return typeof reply === 'string' ? reply : reply(normalized)
  }

  async close(): Promise<void> {
    this.closed = true
  }

  /** The lines sent for one command name. */
  sent(command: string): string[] {
    return this.lines.filter((line) => commandOf(line) === command)
  }
}

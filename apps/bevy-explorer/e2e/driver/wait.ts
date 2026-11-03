// Polling waits. Every wait in a spec is bounded, names what it waits for, and reports the last
// value it saw, as explorer-automation's waits do.

export interface UntilOptions {
  timeoutMs: number
  intervalMs?: number
  /** What is being waited for, for the timeout's message. */
  what: string
}

export class TimeoutError extends Error {
  readonly last: unknown

  constructor(what: string, timeoutMs: number, last: unknown) {
    super(`timed out after ${timeoutMs} ms waiting for ${what}${describeLast(last)}`)
    this.name = 'TimeoutError'
    this.last = last
  }
}

function describeLast(last: unknown): string {
  if (last === undefined) return ''
  const text = typeof last === 'string' ? last : JSON.stringify(last)
  return text === undefined ? '' : `; last: ${text.length > 300 ? `${text.slice(0, 300)}...` : text}`
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/** Poll `read` until `done(value)` is true; resolve with that value. The poll runs at once, then
 *  every `intervalMs` (500 by default); a throwing `read` is retried until the deadline, when its
 *  last error is rethrown. */
export async function until<T>(read: () => Promise<T> | T, done: (value: T) => boolean, options: UntilOptions): Promise<T> {
  const { timeoutMs, intervalMs = 500, what } = options
  const deadline = Date.now() + timeoutMs
  let last: unknown
  let lastError: unknown
  for (;;) {
    try {
      const value = await read()
      last = value
      lastError = undefined
      if (done(value)) return value
    } catch (error) {
      lastError = error
    }
    const left = deadline - Date.now()
    if (left <= 0) {
      if (lastError !== undefined) throw lastError
      throw new TimeoutError(what, timeoutMs, last)
    }
    await sleep(Math.min(intervalMs, left))
  }
}

/** Read until two consecutive reads agree (the catalogue's "settled read"), or the deadline
 *  passes, in which case the last read is returned. */
export async function settled<T>(read: () => Promise<T>, options: UntilOptions & { same?: (a: T, b: T) => boolean }): Promise<T> {
  const { timeoutMs, intervalMs = 1000, same = (a, b) => JSON.stringify(a) === JSON.stringify(b) } = options
  const deadline = Date.now() + timeoutMs
  let previous = await read()
  while (Date.now() < deadline) {
    await sleep(intervalMs)
    const next = await read()
    if (same(previous, next)) return next
    previous = next
  }
  return previous
}

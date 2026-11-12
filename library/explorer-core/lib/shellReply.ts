// How the shell answers the app's requests, on both sides of the bridge: `bevy-shell:request` in, one
// `bevy-shell:response` out, with what the request came to or how it failed. A failure crosses as its
// message, for logs and people, and a code when the shell gave one, for the app to act on: the app matches
// the code, never the message, which may be reworded. Plain data either way, as postMessage's structured
// clone needs (an Error crossing as itself would lose its class and anything set on it).

/** An error the shell answers with a code the app can act on: thrown on the shell's side, rejected with on the app's. */
export class ShellError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.name = 'ShellError'
    this.code = code
  }
}

/** Whether `e` is the shell's answer with `code`. */
export function isShellError(e: unknown, code: string): boolean {
  return e instanceof ShellError && e.code === code
}

/** The `bevy-shell:response` the shell posts back to request `id`: what `run` came to, or how it failed. */
export async function shellResponse(id: unknown, run: () => Promise<unknown>): Promise<Record<string, unknown>> {
  try {
    return { type: 'bevy-shell:response', id, result: await run() }
  } catch (err) {
    const error = err instanceof Error ? err.message : String(err)
    return { type: 'bevy-shell:response', id, error, ...(err instanceof ShellError ? { code: err.code } : {}) }
  }
}

/** What a failed `bevy-shell:response` rejects with on the app's side: its code kept, when it had one. */
export function responseError(response: { error: string; code?: unknown }): Error {
  return typeof response.code === 'string' ? new ShellError(response.code, response.error) : new Error(response.error)
}

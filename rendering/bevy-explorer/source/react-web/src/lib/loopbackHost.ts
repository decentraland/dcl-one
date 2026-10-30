// Where the benchmark harness serves the web app (docs/guidelines/benchmark.md). A leaf module: the
// shell page asks it too (shell/login.ts), and it imports nothing.

/** A loopback host name, as `location.hostname` spells it (an IPv6 address keeps its brackets). */
export function isLoopbackHost(host: string): boolean {
  return host === 'localhost' || host.endsWith('.localhost') || host === '127.0.0.1' || host === '[::1]' || host === '::1'
}

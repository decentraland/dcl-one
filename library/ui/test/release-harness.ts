import { rm } from 'node:fs/promises'
import type { Server } from 'node:http'

type Harness = {
  session?: { close(): Promise<unknown> }
  server?: Server
  scratch?: string
}

// Releases whatever a browser test's setup managed to start (browser session, then a listening server, then the
// scratch folder), attempting every release even when an earlier one fails, and rejects with the first release
// error. It only sees release errors: a setup failure is reported by the test runner on its own.
async function releaseHarness({ session, server, scratch }: Harness): Promise<void> {
  const releases: (() => Promise<unknown>)[] = [
    async () => session?.close(),
    async () => {
      if (server?.listening)
        await new Promise<void>((resolve, reject) =>
          server.close((error) => (error ? reject(error) : resolve()))
        )
    },
    async () => {
      if (scratch) await rm(scratch, { recursive: true, force: true })
    }
  ]
  let first: { error: unknown } | undefined
  for (const release of releases) {
    try {
      await release()
    } catch (error) {
      first ??= { error }
    }
  }
  if (first) throw first.error
}

export { releaseHarness }

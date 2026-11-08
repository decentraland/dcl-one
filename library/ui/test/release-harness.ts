import { rm } from 'node:fs/promises'
import type { Server } from 'node:http'

type Harness = {
  session?: { close(): Promise<unknown> }
  server?: Server
  scratch?: string
}

// Releases whatever a browser test's setup managed to start, in reverse order, even when setup failed part way
// or an earlier release throws. The first release error is rethrown after every resource has been released.
async function releaseHarness({ session, server, scratch }: Harness): Promise<void> {
  try {
    await session?.close()
  } finally {
    try {
      if (server?.listening)
        await new Promise<void>((resolve, reject) =>
          server.close((error) => (error ? reject(error) : resolve()))
        )
    } finally {
      if (scratch) await rm(scratch, { recursive: true, force: true })
    }
  }
}

export { releaseHarness }

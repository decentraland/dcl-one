import { access, mkdtemp } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { releaseHarness } from './release-harness'

const exists = (file: string) =>
  access(file).then(
    () => true,
    () => false
  )

describe('when releasing a browser test harness', () => {
  it('should settle when setup failed before any resource started', async () => {
    await expect(releaseHarness({})).resolves.toBeUndefined()
  })

  it('should remove the scratch folder when setup failed before the server and browser started', async () => {
    const scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
    const server = createServer()

    await expect(releaseHarness({ server, scratch })).resolves.toBeUndefined()
    expect(await exists(scratch)).toBe(false)
  })

  it('should still close the server and remove the scratch folder when the browser fails to close', async () => {
    const scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
    const server = createServer()
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
    const session = { close: () => Promise.reject(new Error('browser already gone')) }

    await expect(releaseHarness({ session, server, scratch })).rejects.toThrow(
      'browser already gone'
    )
    expect(server.listening).toBe(false)
    expect(await exists(scratch)).toBe(false)
  })

  it('should reject with the first release error when several releases fail, and still remove the scratch folder', async () => {
    const scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
    const session = { close: () => Promise.reject(new Error('first: browser close')) }
    let closed = false
    const server = {
      listening: true,
      close: (callback: (error?: Error) => void) => {
        closed = true
        callback(new Error('second: server close'))
      }
    } as unknown as Server

    await expect(releaseHarness({ session, server, scratch })).rejects.toThrow(
      'first: browser close'
    )
    expect(closed).toBe(true)
    expect(await exists(scratch)).toBe(false)
  })
})

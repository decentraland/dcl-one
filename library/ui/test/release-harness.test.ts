import { access, mkdtemp } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { beforeEach, describe, expect, it } from 'vitest'
import { releaseHarness } from './release-harness'

const exists = (file: string) =>
  access(file).then(
    () => true,
    () => false
  )

describe('when releasing a browser test harness', () => {
  describe('and setup failed before any resource started', () => {
    it('should settle', async () => {
      await expect(releaseHarness({})).resolves.toBeUndefined()
    })
  })

  describe('and setup failed before the server and browser started', () => {
    let scratch: string
    let server: Server

    beforeEach(async () => {
      scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
      server = createServer()
    })

    it('should settle and remove the scratch folder', async () => {
      await expect(releaseHarness({ server, scratch })).resolves.toBeUndefined()
      expect(await exists(scratch)).toBe(false)
    })
  })

  describe('and the browser fails to close', () => {
    let scratch: string
    let server: Server
    let session: { close: () => Promise<void> }

    beforeEach(async () => {
      scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
      server = createServer()
      await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
      session = { close: () => Promise.reject(new Error('browser already gone')) }
    })

    it('should reject with the browser error, still close the server and remove the scratch folder', async () => {
      await expect(releaseHarness({ session, server, scratch })).rejects.toThrow(
        'browser already gone'
      )
      expect(server.listening).toBe(false)
      expect(await exists(scratch)).toBe(false)
    })
  })

  describe('and several releases fail', () => {
    let scratch: string
    let session: { close: () => Promise<void> }
    let closed: boolean
    let server: Server

    beforeEach(async () => {
      scratch = await mkdtemp(path.join(tmpdir(), 'release-harness-'))
      session = { close: () => Promise.reject(new Error('first: browser close')) }
      closed = false
      server = {
        listening: true,
        close: (callback: (error?: Error) => void) => {
          closed = true
          callback(new Error('second: server close'))
        }
      } as unknown as Server
    })

    it('should reject with the first release error, still close the server and remove the scratch folder', async () => {
      await expect(releaseHarness({ session, server, scratch })).rejects.toThrow(
        'first: browser close'
      )
      expect(closed).toBe(true)
      expect(await exists(scratch)).toBe(false)
    })
  })
})

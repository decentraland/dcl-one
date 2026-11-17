import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createApp } from './app.js'
import { digestTableHash, FdsCacheError, MARKER } from './sources/fds-source.js'
import { createTools } from './tools/index.js'

describe('when MCP reads the configured FDS cache through createApp', () => {
  let root: string
  let cache: string
  let configFile: string
  let lockFile: string
  let config: { outDir: string; snapshotDir: string; snapshotLock?: string; pages: object }
  let revision: string
  let digests: Record<string, string>
  let pagesBytes: string
  let readPages: () => Promise<unknown>

  beforeEach(async () => {
    root = await mkdtemp(path.join(tmpdir(), 'dcl-app-cache-boundary-'))
    cache = path.join(root, 'cache')
    configFile = path.join(root, 'figma.config.json')
    lockFile = path.join(root, 'upstream.lock.json')
    config = {
      outDir: 'output',
      snapshotDir: 'cache',
      snapshotLock: 'upstream.lock.json',
      pages: {}
    }
    revision = 'a'.repeat(40)
    pagesBytes = JSON.stringify({ pages: [{ id: 'synthetic:page', name: 'Synthetic page' }] })
    digests = { 'pages.json': createHash('sha256').update(pagesBytes).digest('hex') }
    await mkdir(cache)
    await writeFile(path.join(cache, 'pages.json'), pagesBytes)
    await writeFile(configFile, JSON.stringify(config))
    await writeFile(
      lockFile,
      JSON.stringify({
        sources: [
          {
            repository: 'https://github.com/decentraland/figma-design-system',
            revision,
            snapshot: { revision, digests }
          }
        ]
      })
    )
    await writeFile(
      path.join(cache, MARKER),
      JSON.stringify({ revision, digests: digestTableHash(digests) })
    )
    readPages = async () => {
      const app = await createApp(root, configFile)
      const tool = createTools(app).find((entry) => entry.name === 'figma_list_pages')
      if (!tool) throw new Error('figma_list_pages is not registered')
      return tool.handler({ source: 'snapshot' })
    }
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  describe('and the cache matches its pinned revision and digests', () => {
    it('should return the verified page through the MCP handler', async () => {
      await expect(readPages()).resolves.toEqual({
        pages: [{ id: 'synthetic:page', name: 'Synthetic page', configured: null }]
      })
    })
  })

  describe.each([
    'absent lock setting',
    'empty lock setting',
    'missing lock file',
    'missing marker',
    'stale revision marker',
    'stale digest marker',
    'malformed marker',
    'altered cache'
  ])('and the cache has %s', (problem) => {
    beforeEach(async () => {
      switch (problem) {
        case 'absent lock setting':
          delete config.snapshotLock
          await writeFile(configFile, JSON.stringify(config))
          break
        case 'empty lock setting':
          config.snapshotLock = ''
          await writeFile(configFile, JSON.stringify(config))
          break
        case 'missing lock file':
          await rm(lockFile)
          break
        case 'missing marker':
          await rm(path.join(cache, MARKER))
          break
        case 'stale revision marker':
          await writeFile(
            path.join(cache, MARKER),
            JSON.stringify({ revision: 'b'.repeat(40), digests: digestTableHash(digests) })
          )
          break
        case 'stale digest marker':
          await writeFile(
            path.join(cache, MARKER),
            JSON.stringify({ revision, digests: '0'.repeat(64) })
          )
          break
        case 'malformed marker':
          await writeFile(path.join(cache, MARKER), '{')
          break
        case 'altered cache':
          await writeFile(
            path.join(cache, 'pages.json'),
            JSON.stringify({ pages: [{ id: 'synthetic:page', name: 'Unpinned page' }] })
          )
          break
      }
    })

    it('should reject the read as an unusable FDS cache', async () => {
      await expect(readPages()).rejects.toBeInstanceOf(FdsCacheError)
    })
  })
})

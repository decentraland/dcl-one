import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { copySnapshot } from '../test/snapshot.js'
import { createApp } from './app.js'
import { digestTableHash, FdsCacheError, MARKER } from './sources/fds-source.js'

// The production boundary: createApp without the explicit test option reads only a cache that matches the
// digest table upstream.lock.json pins, and refuses one it cannot verify before reading anything.
const REVISION = 'c'.repeat(40)

async function listFiles(dir: string, prefix = ''): Promise<string[]> {
  const out: string[] = []
  for (const entry of await readdir(path.join(dir, prefix), { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name
    if (entry.isDirectory()) out.push(...(await listFiles(dir, relative)))
    else out.push(relative)
  }
  return out
}

describe('when a command opens the capture', () => {
  let root: string
  let figma: string
  let cache: string
  let app: Awaited<ReturnType<typeof createApp>>

  async function writeConfig(extra: Record<string, unknown>) {
    await writeFile(
      path.join(figma, 'figma.config.json'),
      JSON.stringify({ outDir: 'output', snapshotDir: 'figma-snapshot', pages: {}, ...extra })
    )
  }

  async function openApp(options?: Parameters<typeof createApp>[2]) {
    return createApp(figma, path.join(figma, 'figma.config.json'), options)
  }

  beforeEach(async () => {
    root = await mkdtemp(path.join(tmpdir(), 'app-cache-'))
    figma = path.join(root, 'figma')
    cache = path.join(figma, 'figma-snapshot')
    await mkdir(figma, { recursive: true })
    await copySnapshot(cache)
    const digests: Record<string, string> = {}
    for (const file of (await listFiles(cache)).filter((name) => name !== MARKER))
      digests[file] = createHash('sha256')
        .update(await readFile(path.join(cache, file)))
        .digest('hex')
    await writeFile(
      path.join(root, 'upstream.lock.json'),
      JSON.stringify({
        sources: [
          {
            repository: 'https://github.com/decentraland/figma-design-system',
            revision: REVISION,
            snapshot: { revision: REVISION, digests }
          }
        ]
      })
    )
    await writeFile(
      path.join(cache, MARKER),
      JSON.stringify({ revision: REVISION, digests: digestTableHash(digests) })
    )
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  describe('and the config names the pinned lock', () => {
    beforeEach(async () => {
      await writeConfig({ snapshotLock: '../upstream.lock.json' })
    })

    describe('and the cache matches the pinned digests', () => {
      beforeEach(async () => {
        app = await openApp()
      })

      it('should read the cache', async () => {
        expect((await app.createSource().listPages()).length).toBeGreaterThan(0)
      })
    })

    describe('and the cache has no fetch-source marker', () => {
      beforeEach(async () => {
        await rm(path.join(cache, MARKER))
        app = await openApp()
      })

      it('should refuse the cache as not materialized', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(/not materialized/)
      })
    })

    describe('and the marker was built for another capture commit', () => {
      beforeEach(async () => {
        const marker = JSON.parse(await readFile(path.join(cache, MARKER), 'utf8'))
        await writeFile(
          path.join(cache, MARKER),
          JSON.stringify({ ...marker, revision: 'd'.repeat(40) })
        )
        app = await openApp()
      })

      it('should refuse the stale marker as a cache error', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(FdsCacheError)
      })

      it('should report that the marker was built for another pin', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(/another pin/)
      })
    })

    describe.each([
      ['null', /marker is malformed/],
      ['42', /marker is malformed/],
      ['[]', /marker is malformed/],
      ['{ not json', /marker is unreadable/]
    ])('and the marker holds %s', (content, message) => {
      beforeEach(async () => {
        await writeFile(path.join(cache, MARKER), content)
        app = await openApp()
      })

      it('should refuse the marker as a cache error', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(FdsCacheError)
      })

      it('should report why the marker is unusable', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(message)
      })
    })

    describe('and a cache file was edited', () => {
      beforeEach(async () => {
        await writeFile(path.join(cache, 'pages.json'), '{"pages":[]}\n')
        app = await openApp()
      })

      it('should refuse the edited cache as a cache error', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(FdsCacheError)
      })

      it('should report the file that differs from the pinned digest', async () => {
        await expect(app.createSource().listPages()).rejects.toThrow(
          /differing from the pinned digest/
        )
      })
    })
  })

  describe('and the config names a missing lock file', () => {
    beforeEach(async () => {
      await writeConfig({ snapshotLock: '../missing.lock.json' })
      app = await openApp()
    })

    it('should refuse the capture as an unusable pin', async () => {
      await expect(app.createSource().listPages()).rejects.toThrow(/pin is unusable/)
    })
  })

  describe('and the config names no lock', () => {
    beforeEach(async () => {
      await writeConfig({})
    })

    describe('and the test does not ask for an unverified snapshot', () => {
      beforeEach(async () => {
        app = await openApp()
      })

      it('should refuse the config as a cache error before reading the directory', () => {
        expect(() => app.createSource()).toThrow(FdsCacheError)
      })

      it('should report that the config names no snapshotLock', () => {
        expect(() => app.createSource()).toThrow(/names no snapshotLock/)
      })
    })

    describe('and the test explicitly asks for an unverified snapshot', () => {
      beforeEach(async () => {
        app = await openApp({ unverifiedSnapshot: true })
      })

      it('should read the unverified directory', async () => {
        expect((await app.createSource().listPages()).length).toBeGreaterThan(0)
      })
    })
  })
})

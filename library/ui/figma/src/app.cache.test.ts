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
let root: string
let figma: string
let cache: string

async function listFiles(dir: string, prefix = ''): Promise<string[]> {
  const out: string[] = []
  for (const entry of await readdir(path.join(dir, prefix), { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name
    if (entry.isDirectory()) out.push(...(await listFiles(dir, relative)))
    else out.push(relative)
  }
  return out
}

async function writeConfig(extra: Record<string, unknown>) {
  await writeFile(
    path.join(figma, 'figma.config.json'),
    JSON.stringify({ outDir: 'output', snapshotDir: 'figma-snapshot', pages: {}, ...extra })
  )
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

describe('when a command opens the capture', () => {
  it('should read a cache that matches the pinned digests', async () => {
    await writeConfig({ snapshotLock: '../upstream.lock.json' })
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    expect((await app.createSource().listPages()).length).toBeGreaterThan(0)
  })

  it('should refuse a config that names no lock, before reading the directory', async () => {
    await writeConfig({})
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    expect(() => app.createSource()).toThrow(FdsCacheError)
    expect(() => app.createSource()).toThrow(/names no snapshotLock/)
  })

  it('should refuse a cache without the fetch-source marker', async () => {
    await writeConfig({ snapshotLock: '../upstream.lock.json' })
    await rm(path.join(cache, MARKER))
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    await expect(app.createSource().listPages()).rejects.toThrow(/not materialized/)
  })

  it('should refuse a stale marker built for another capture commit', async () => {
    await writeConfig({ snapshotLock: '../upstream.lock.json' })
    const marker = JSON.parse(await readFile(path.join(cache, MARKER), 'utf8'))
    await writeFile(
      path.join(cache, MARKER),
      JSON.stringify({ ...marker, revision: 'd'.repeat(40) })
    )
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    await expect(app.createSource().listPages()).rejects.toThrow(FdsCacheError)
    await expect(app.createSource().listPages()).rejects.toThrow(/another pin/)
  })

  it('should refuse an edited cache file', async () => {
    await writeConfig({ snapshotLock: '../upstream.lock.json' })
    await writeFile(path.join(cache, 'pages.json'), '{"pages":[]}\n')
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    await expect(app.createSource().listPages()).rejects.toThrow(FdsCacheError)
  })

  it('should refuse a missing lock file', async () => {
    await writeConfig({ snapshotLock: '../missing.lock.json' })
    const app = await createApp(figma, path.join(figma, 'figma.config.json'))

    await expect(app.createSource().listPages()).rejects.toThrow(/pin is unusable/)
  })

  it('should read an unverified directory only when a test asks for it explicitly', async () => {
    await writeConfig({})
    const app = await createApp(figma, path.join(figma, 'figma.config.json'), {
      unverifiedSnapshot: true
    })

    expect((await app.createSource().listPages()).length).toBeGreaterThan(0)
  })
})

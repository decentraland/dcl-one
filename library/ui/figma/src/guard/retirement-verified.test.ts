import { execFile } from 'node:child_process'
import { readdir, readFile, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createApp, runSync } from '../app.js'
import { FdsCacheError } from '../sources/fds-source.js'
import { pinTestCapture } from '../../test/snapshot.js'
import { fixtureBytes, lifecycleFixture, put, SNAP, LIB } from './lifecycle-fixture.js'

// The retirement cleanup reads a staged copy of the capture without re-verifying it, because applying the
// retirement edits that copy. These cases prove it only ever sees a copy of a cache that was verified first:
// the production App (no test opt-out) refuses an edited cache before anything is staged or written.
const execute = promisify(execFile)
let root: string
let records: unknown

beforeEach(async () => {
  ;({ root, records } = await lifecycleFixture())
  await put(root, `figma/ledger/retirements.json`, records)
  await pinTestCapture(path.join(root, 'figma'))
})

afterEach(async () => {
  await rm(root, { recursive: true, force: true })
})

const cacheBytes = async () => {
  const cache = path.join(root, SNAP)
  const files = (await readdir(cache, { recursive: true, withFileTypes: true }))
    .filter((entry) => entry.isFile())
    .map((entry) => path.relative(cache, path.join(entry.parentPath, entry.name)))
    .sort()
  return Promise.all(
    files.map(async (file) => [file, await readFile(path.join(cache, file))] as const)
  )
}

describe('when a developer applies retirements through the production App', () => {
  it('should stage and publish from a verified cache', async () => {
    const app = await createApp(path.join(root, 'figma'))

    const report = await runSync(app, {
      source: 'snapshot',
      developer: true,
      applyRetirements: true
    })

    expect(report.retired?.length).toBeGreaterThan(0)
    await expect(readFile(path.join(root, `${LIB}/page/First.figma.ts`))).rejects.toMatchObject({
      code: 'ENOENT'
    })
  })

  it('should leave the pinned cache byte-identical, so the next command still verifies it', async () => {
    const before = await cacheBytes()

    const report = await runSync(await createApp(path.join(root, 'figma')), {
      source: 'snapshot',
      developer: true,
      applyRetirements: true
    })

    expect(report.retired?.length).toBeGreaterThan(0)
    expect(report.retired?.filter((file) => file.startsWith(`${SNAP}/`))).toEqual([])
    expect(await cacheBytes()).toEqual(before)
    await expect(
      (await createApp(path.join(root, 'figma'))).createSource().listPages()
    ).resolves.not.toHaveLength(0)
  })

  it('should refuse an edited cache before staging or writing anything', async () => {
    await writeFile(path.join(root, `${SNAP}/pages.json`), '{"pages":[]}\n')
    const before = await fixtureBytes(root)
    const app = await createApp(path.join(root, 'figma'))

    await expect(
      runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
    ).rejects.toBeInstanceOf(FdsCacheError)
    expect(await fixtureBytes(root)).toEqual(before)
  })
})

describe('when retire is given a config that names another capture', () => {
  it('should refuse instead of verifying one cache and reading another', async () => {
    const config = JSON.parse(await readFile(path.join(root, 'figma/figma.config.json'), 'utf8'))
    await writeFile(
      path.join(root, 'figma/other.config.json'),
      JSON.stringify({ ...config, snapshotDir: 'other-snapshot' })
    )

    await expect(
      execute(
        process.execPath,
        [
          '--import',
          import.meta.resolve('tsx'),
          path.resolve(import.meta.dirname, '../cli.ts'),
          'retire',
          '--page-id',
          '1:1',
          '--reason',
          'x',
          '--config',
          path.join(root, 'figma/other.config.json')
        ],
        { cwd: path.join(root, 'figma') }
      )
    ).rejects.toMatchObject({ stderr: expect.stringContaining('names another snapshotDir') })
  })
})

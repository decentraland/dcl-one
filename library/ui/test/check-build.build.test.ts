import { cp, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { checkBuild } from '../figma/review/scripts/check-build.mjs'

// The gallery's functional proof, run against a real relative-base build served under a nested prefix, and
// against broken copies of it: each must fail the check rather than pass vacuously.
const review = path.resolve(import.meta.dirname, '../figma/review')
let scratch: string
let dist: string

beforeAll(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'check-build-'))
  dist = path.join(scratch, 'dist')
  // The runner sets NODE_ENV=test, which would bundle React's development build; publish what CI publishes.
  const environment = process.env.NODE_ENV
  process.env.NODE_ENV = 'production'
  try {
    await build({
      configFile: path.join(review, 'review.vite.config.ts'),
      mode: 'production',
      base: './',
      logLevel: 'silent',
      build: { outDir: dist, emptyOutDir: true }
    })
  } finally {
    process.env.NODE_ENV = environment
  }
}, 300_000)

afterAll(() => rm(scratch, { recursive: true, force: true }))

async function brokenCopy(name: string, breakIt: (copy: string) => Promise<void>) {
  const copy = path.join(scratch, name)
  await cp(dist, copy, { recursive: true })
  await breakIt(copy)
  return copy
}

describe('when checking a built gallery', () => {
  it('should navigate the catalog, switch variants and draw an audit render under a nested prefix', async () => {
    const summary = await checkBuild({ dist, prefix: '/nested/prefix/' })

    expect(summary.entry).toBe('/nested/prefix/index.html')
    expect(summary.components).toBeGreaterThan(0)
    expect(summary.variant).toBeGreaterThan(0)
    expect(summary.audit.width).toBeGreaterThan(0)
    expect(summary.audit.height).toBeGreaterThan(0)
  }, 120_000)

  it('should also work at the root', async () => {
    await expect(checkBuild({ dist })).resolves.toMatchObject({ entry: '/index.html' })
  }, 120_000)

  it('should fail when a script the entry loads is missing', async () => {
    const copy = await brokenCopy('missing-script', async (target) => {
      const assets = path.join(target, 'dcl-ui-review/assets')
      const script = (await readdir(assets)).find((file) => file.endsWith('.js'))!
      await rm(path.join(assets, script))
    })

    await expect(checkBuild({ dist: copy, prefix: '/nested/prefix/' })).rejects.toThrow()
  }, 120_000)

  it('should fail when the root entry is missing', async () => {
    const copy = await brokenCopy('missing-entry', (target) => rm(path.join(target, 'index.html')))

    await expect(checkBuild({ dist: copy })).rejects.toThrow(/Timed out|requests failed/)
  }, 120_000)

  it('should fail on an uncaught page error', async () => {
    const copy = await brokenCopy('page-error', async (target) => {
      const assets = path.join(target, 'dcl-ui-review/assets')
      await writeFile(path.join(assets, 'boom.js'), 'throw new Error("boom")\n')
      const entry = path.join(target, 'index.html')
      const html = await readFile(entry, 'utf8')
      await writeFile(
        entry,
        html.replace('</head>', '<script src="./dcl-ui-review/assets/boom.js"></script></head>')
      )
    })

    await expect(checkBuild({ dist: copy })).rejects.toThrow(/page error|boom/)
  }, 120_000)

  it('should reject a prefix that is not an absolute path', async () => {
    await expect(checkBuild({ dist, prefix: 'relative/' })).rejects.toThrow('absolute URL path')
  })
})

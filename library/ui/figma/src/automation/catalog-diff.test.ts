import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { catalogDiffFile, saveCatalogDiff } from './catalog-diff.js'

describe('when retaining a failed capture catalog diff', () => {
  let root: string
  let reportFile: string
  let stagedFile: string
  let before: string | undefined
  let after: string
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'catalog-diff-'))
    reportFile = path.join(root, 'reports', 'capture report.json')
    stagedFile = path.join(root, 'staged-catalog.json')
    before = JSON.stringify({ stroke: 1, frozen: true }, null, 2) + '\n'
    after = JSON.stringify({ stroke: 3, frozen: true }, null, 2) + '\n'
    await writeFile(stagedFile, after)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })
  it('should retain exact changed lines with stable filenames and leave the staged input untouched', async () => {
    expect(await saveCatalogDiff({ before, stagedFile, reportFile })).toBe(
      'capture report.json.catalog.diff'
    )
    const diff = await readFile(catalogDiffFile(reportFile), 'utf8')
    expect(diff).toContain('-  "stroke": 1,')
    expect(diff).toContain('+  "stroke": 3,')
    expect(diff).not.toContain(root)
    expect(await readFile(stagedFile, 'utf8')).toBe(after)
  })
  describe('and the staged catalog is unchanged', () => {
    beforeEach(async () => {
      await writeFile(stagedFile, before!)
    })
    it('should omit an empty diagnostic artifact', async () => {
      expect(await saveCatalogDiff({ before, stagedFile, reportFile })).toBeUndefined()
      await expect(readFile(catalogDiffFile(reportFile))).rejects.toMatchObject({ code: 'ENOENT' })
    })
  })
  describe('and no catalog was staged before the failure', () => {
    beforeEach(async () => {
      await rm(stagedFile)
    })
    it('should omit evidence for an unavailable stage', async () => {
      expect(await saveCatalogDiff({ before, stagedFile, reportFile })).toBeUndefined()
    })
  })
  describe('and no previous catalog exists', () => {
    beforeEach(() => {
      before = undefined
    })
    it('should retain the staged contents as additions', async () => {
      expect(await saveCatalogDiff({ before, stagedFile, reportFile })).toBeDefined()
      expect(await readFile(catalogDiffFile(reportFile), 'utf8')).toContain('+  "stroke": 3,')
    })
  })
  describe('and reading the staged catalog fails', () => {
    beforeEach(async () => {
      await rm(stagedFile)
      await mkdir(stagedFile)
    })
    it('should propagate the read failure instead of treating it as missing', async () => {
      await expect(saveCatalogDiff({ before, stagedFile, reportFile })).rejects.toThrow()
    })
  })
  describe('and the artifact directory cannot be created', () => {
    beforeEach(async () => {
      await writeFile(path.dirname(reportFile), 'blocked directory')
    })
    it('should propagate the write error without changing the source catalog', async () => {
      await expect(saveCatalogDiff({ before, stagedFile, reportFile })).rejects.toThrow()
      expect(await readFile(stagedFile, 'utf8')).toBe(after)
    })
  })
})
